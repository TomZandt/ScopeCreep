//! The reader side of the Feed: what the app uses.
use crate::SentColourSetup;
use crate::layout::*;
use crate::{READER_TIMEOUT_NS, VERSION_MAJOR, VERSION_MINOR, now_ns};
use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};

/// Why a Feed could not be opened. None of these is a crash; the app reports them.
#[derive(Debug)]
pub enum OpenError {
    /// No Feed file: no Sender has been loaded. A normal state.
    NoFeed,
    /// The file is there but is not a Feed.
    NotAFeed,
    /// The Sender speaks another major version of the protocol.
    VersionMismatch { found: (u32, u32), supported: (u32, u32) },
    /// Another app (process `pid`) is attached to this Feed.
    Busy { pid: u64 },
    Io(io::Error),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::NoFeed => write!(f, "no Feed"),
            OpenError::NotAFeed => write!(f, "not a Feed file"),
            OpenError::VersionMismatch { found, supported } => {
                write!(f, "Feed version {}.{} (this app reads version {}.x)", found.0, found.1, supported.0)
            }
            OpenError::Busy { pid } => write!(f, "another app (process {pid}) is using the Feed"),
            OpenError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Who is writing the Feed.
#[derive(Clone, Debug, PartialEq)]
pub struct SenderInfo {
    pub name: String,
    pub version: String,
    pub host: String,
    pub pid: u64,
    pub protocol: (u32, u32),
}

/// The Feed's counters, as the Sender keeps them (feed/PROTOCOL.md).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FeedStats {
    pub slot_count: usize,
    pub slot_capacity: usize,
    pub frames_taken: u64,
    pub frames_published: u64,
    pub drops_overwritten: u64,
    pub drops_busy: u64,
    pub renders_total: u64,
    pub renders_thumbnail: u64,
    pub renders_idle: u64,
    pub renders_unsupported: u64,
    pub render_call_ns_total: u64,
    pub last_render_ns: u64,
}

impl FeedStats {
    /// Frames the Sender had to give up because the reader was not keeping up.
    pub fn dropped(&self) -> u64 {
        self.drops_overwritten + self.drops_busy
    }
}

pub struct FeedReader {
    map: Arc<Mapping>,
    path: PathBuf,
    file_id: (u64, u64),
}

impl FeedReader {
    /// Attach to the Feed at `path`. From this moment the Sender sees a reader and starts copying frames, for as
    /// long as [`FeedReader::heartbeat`] keeps being called.
    pub fn open(path: impl AsRef<Path>) -> Result<FeedReader, OpenError> {
        let path = path.as_ref().to_path_buf();
        let file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(OpenError::NoFeed),
            Err(e) => return Err(OpenError::Io(e)),
        };
        let meta = file.metadata().map_err(OpenError::Io)?;
        if (meta.len() as usize) < HEADER_BYTES {
            return Err(OpenError::NotAFeed);
        }
        let header = Mapping::map(&file, HEADER_BYTES).map_err(OpenError::Io)?;
        if header.u64(H_MAGIC) != MAGIC {
            return Err(OpenError::NotAFeed);
        }
        let found = (header.u32(H_VERSION_MAJOR), header.u32(H_VERSION_MINOR));
        if found.0 != VERSION_MAJOR {
            return Err(OpenError::VersionMismatch { found, supported: (VERSION_MAJOR, VERSION_MINOR) });
        }
        if header.u32(H_HEADER_BYTES) as usize != HEADER_BYTES || header.u32(H_SLOT_HEADER_BYTES) as usize != SLOT_HEADER_BYTES {
            return Err(OpenError::NotAFeed);
        }
        let bytes = file_bytes(header.slot_count(), header.slot_capacity());
        if (meta.len() as usize) < bytes {
            return Err(OpenError::NotAFeed);
        }
        let now = now_ns();
        let beat = header.atomic_u64(H_READER_HEARTBEAT_NS).load(Acquire);
        let other = header.atomic_u64(H_READER_PID).load(Acquire);
        if beat != 0 && now.saturating_sub(beat) < READER_TIMEOUT_NS && other != std::process::id() as u64 {
            return Err(OpenError::Busy { pid: other });
        }
        drop(header);
        let map = Mapping::map(&file, bytes).map_err(OpenError::Io)?;
        for i in 0..map.slot_count() {
            // Slots left READING belong to a reader that died.
            let _ = map.atomic_u32(map.slot(i) + S_STATE).compare_exchange(READING, FREE, AcqRel, Acquire);
        }
        let reader = FeedReader { map: Arc::new(map), path, file_id: (meta.dev(), meta.ino()) };
        reader.heartbeat(now);
        Ok(reader)
    }

    /// Tell the Sender the app is still here. Call at least every [`crate::HEARTBEAT_INTERVAL_NS`].
    pub fn heartbeat(&self, now_ns: u64) {
        self.map.atomic_u64(H_READER_PID).store(std::process::id() as u64, Release);
        self.map.atomic_u64(H_READER_HEARTBEAT_NS).store(now_ns, Release);
    }

    /// Has the Sender marked this Feed file as replaced (it made a ring with a new layout)? If so, open the path
    /// again. Cheap: one atomic load.
    pub fn retired(&self) -> bool {
        self.map.atomic_u32(H_RETIRED).load(Acquire) != 0
    }

    /// Does the path now name a different file (Resolve restarted, so nobody marked this one retired)? If so,
    /// open the path again. Costs a `stat`, so check it a few times a second, not every poll.
    pub fn replaced(&self) -> bool {
        match std::fs::metadata(&self.path) {
            Ok(m) => (m.dev(), m.ino()) != self.file_id,
            Err(_) => true,
        }
    }

    pub fn sender(&self) -> SenderInfo {
        let m = &self.map;
        SenderInfo {
            name: m.text(H_SENDER_NAME, SENDER_NAME_BYTES),
            version: m.text(H_SENDER_VERSION, SENDER_VERSION_BYTES),
            host: m.text(H_HOST_NAME, HOST_NAME_BYTES),
            pid: m.u64(H_SENDER_PID),
            protocol: (m.u32(H_VERSION_MAJOR), m.u32(H_VERSION_MINOR)),
        }
    }

    /// Is the Sender's host process still running?
    pub fn sender_alive(&self) -> bool {
        let pid = self.map.u64(H_SENDER_PID);
        // SAFETY: signal 0 only checks that the process exists.
        pid != 0 && (unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
    }

    /// The Sender's last error, if it reported one.
    pub fn sender_error(&self) -> Option<String> {
        let e = self.map.text(H_SENDER_ERROR, SENDER_ERROR_BYTES);
        (!e.is_empty()).then_some(e)
    }

    pub fn stats(&self) -> FeedStats {
        let m = &self.map;
        let c = |off| m.atomic_u64(off).load(Relaxed);
        FeedStats {
            slot_count: m.slot_count(),
            slot_capacity: m.slot_capacity(),
            frames_taken: c(H_NEXT_SEQUENCE),
            frames_published: c(H_FRAMES_PUBLISHED),
            drops_overwritten: c(H_DROPS_OVERWRITTEN),
            drops_busy: c(H_DROPS_BUSY),
            renders_total: c(H_RENDERS_TOTAL),
            renders_thumbnail: c(H_RENDERS_THUMBNAIL),
            renders_idle: c(H_RENDERS_IDLE),
            renders_unsupported: c(H_RENDERS_UNSUPPORTED),
            render_call_ns_total: c(H_RENDER_CALL_NS_TOTAL),
            last_render_ns: c(H_LAST_RENDER_NS),
        }
    }

    /// Claim the oldest unread frame, if there is one. The frame stays valid (and its slot stays out of the
    /// Sender's reach) until the returned [`Frame`] is dropped, so drop it promptly.
    pub fn next_frame(&mut self) -> Option<Frame> {
        let m = &self.map;
        let n = m.slot_count();
        for _ in 0..4 {
            let oldest = (0..n)
                .filter(|&i| m.atomic_u32(m.slot(i) + S_STATE).load(Acquire) == READY)
                .min_by_key(|&i| m.atomic_u64(m.slot(i) + S_SEQUENCE).load(Acquire))?;
            let s = m.slot(oldest);
            if m.atomic_u32(s + S_STATE).compare_exchange(READY, READING, AcqRel, Acquire).is_err() {
                continue; // the Sender overwrote it between the scan and the swap: look again
            }
            let frame = Frame {
                map: self.map.clone(),
                slot: s,
                sequence: m.atomic_u64(s + S_SEQUENCE).load(Acquire),
                width: m.u32(s + S_WIDTH),
                height: m.u32(s + S_HEIGHT),
                row_bytes: m.u32(s + S_ROW_BYTES),
                bottom_up: m.u32(s + S_ROW_ORDER) == 0,
                data_bytes: m.u64(s + S_DATA_BYTES),
                frame_time: m.f64(s + S_FRAME_TIME),
                frame_rate: m.f64(s + S_FRAME_RATE),
                render_ns: m.u64(s + S_RENDER_NS),
                publish_ns: m.u64(s + S_PUBLISH_NS),
                sender_instance: m.u32(s + S_SENDER_INSTANCE),
                colour_setup: SentColourSetup::load(m, s),
            };
            let sane = m.u32(s + S_PIXEL_FORMAT) == PIXEL_RGBA32F
                && frame.width > 0
                && frame.height > 0
                && frame.row_bytes as u64 >= frame.width as u64 * 16
                && frame.row_bytes.is_multiple_of(4)
                && frame.data_bytes == frame.row_bytes as u64 * frame.height as u64
                && frame.data_bytes <= m.slot_capacity() as u64;
            if sane {
                return Some(frame);
            } // else a frame this reader cannot make sense of: give the slot back (Frame's Drop) and look again
        }
        None
    }
}

impl Drop for FeedReader {
    fn drop(&mut self) {
        // Detach cleanly: the Sender stops copying at once.
        self.map.atomic_u64(H_READER_HEARTBEAT_NS).store(0, Release);
    }
}

/// One frame of the Feed, read in place from shared memory. Dropping it gives the slot back to the Sender.
pub struct Frame {
    map: Arc<Mapping>,
    slot: usize,
    sequence: u64,
    width: u32,
    height: u32,
    row_bytes: u32,
    bottom_up: bool,
    data_bytes: u64,
    frame_time: f64,
    frame_rate: f64,
    render_ns: u64,
    publish_ns: u64,
    sender_instance: u32,
    colour_setup: Option<SentColourSetup>,
}

impl Frame {
    /// The Colour Setup the Sender sent with this frame (its manual override), if any. `None` means the Sender has
    /// no Colour Setup of its own, not that there is none: the app then uses the project's.
    pub fn colour_setup(&self) -> Option<SentColourSetup> {
        self.colour_setup.clone()
    }
    /// 1, 2, 3 ... in the order the Sender took frames. A gap means frames were dropped.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    /// Floats from one row to the next (`width * 4` when rows are tightly packed).
    pub fn row_floats(&self) -> usize {
        self.row_bytes as usize / 4
    }
    /// True when the first row in [`Frame::pixels`] is the bottom row of the picture (as OFX hands frames over).
    pub fn bottom_up(&self) -> bool {
        self.bottom_up
    }
    /// Source frame number on the timeline.
    pub fn frame_time(&self) -> f64 {
        self.frame_time
    }
    /// Frames per second of the source timeline, 0 = unknown.
    pub fn frame_rate(&self) -> f64 {
        self.frame_rate
    }
    /// Monotonic time the Sender's render call began.
    pub fn render_ns(&self) -> u64 {
        self.render_ns
    }
    /// Monotonic time the copy into the Feed completed.
    pub fn publish_ns(&self) -> u64 {
        self.publish_ns
    }
    /// Which Sender instance made the frame.
    pub fn sender_instance(&self) -> u32 {
        self.sender_instance
    }
    /// The pixel values exactly as the Sender received them: RGBA 32-bit float, `row_floats()` per row.
    pub fn pixels(&self) -> &[f32] {
        let count = self.data_bytes as usize / 4;
        // SAFETY: the slot is READING until this Frame is dropped, so the Sender does not touch it; range checked.
        unsafe { std::slice::from_raw_parts(self.map.floats(self.slot + SLOT_HEADER_BYTES, count), count) }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        self.map.atomic_u32(self.slot + S_STATE).store(FREE, Release);
    }
}
