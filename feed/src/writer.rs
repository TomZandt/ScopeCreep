//! The synthetic frame writer: plays the part of Resolve and the Sender, so everything downstream of the Feed can
//! be tested with no Resolve. It follows the same slot protocol as the Sender (feed/PROTOCOL.md).
use crate::layout::*;
use crate::{READER_TIMEOUT_NS, SENDER_NAME, VERSION_MAJOR, VERSION_MINOR, now_ns};
use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};

/// What the Sender knows about a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameMeta {
    pub width: u32,
    pub height: u32,
    /// Source frame number on the timeline.
    pub frame_time: f64,
    /// Frames per second of the source timeline, 0 = unknown.
    pub frame_rate: f64,
    /// Monotonic time the render call began.
    pub render_ns: u64,
    pub sender_instance: u32,
}

#[derive(Debug, PartialEq)]
pub enum WriteError {
    /// Every slot was being written or read: the frame was skipped and counted as a drop.
    Dropped,
    /// The frame does not fit a slot. Make a new Feed with larger slots.
    TooLarge,
}

pub struct FeedWriter {
    map: Mapping,
    path: PathBuf,
}

impl FeedWriter {
    /// Publish a new Feed at `path` with `slot_count` slots of `slot_capacity` bytes (rounded up to 4096).
    /// A Feed already at `path` is replaced and marked retired, as the protocol requires.
    pub fn create(path: impl AsRef<Path>, slot_count: usize, slot_capacity: usize) -> io::Result<FeedWriter> {
        Self::create_with_version(path, slot_count, slot_capacity, VERSION_MAJOR, VERSION_MINOR)
    }

    /// As [`FeedWriter::create`], but claiming another protocol version (to test how readers react).
    pub fn create_with_version(path: impl AsRef<Path>, slot_count: usize, slot_capacity: usize, major: u32, minor: u32) -> io::Result<FeedWriter> {
        let path = path.as_ref().to_path_buf();
        let slot_capacity = slot_capacity.div_ceil(4096) * 4096;
        let bytes = file_bytes(slot_count, slot_capacity);
        let tmp = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let file = OpenOptions::new().read(true).write(true).create_new(true).mode(0o600).open(&tmp)?;
        file.set_len(bytes as u64)?;
        let map = Mapping::map(&file, bytes)?;
        map.set_u32(H_VERSION_MAJOR, major);
        map.set_u32(H_VERSION_MINOR, minor);
        map.set_u32(H_HEADER_BYTES, HEADER_BYTES as u32);
        map.set_u32(H_SLOT_HEADER_BYTES, SLOT_HEADER_BYTES as u32);
        map.set_u32(H_SLOT_COUNT, slot_count as u32);
        map.set_u64(H_SLOT_CAPACITY, slot_capacity as u64);
        map.set_u64(H_FEED_ID, (now_ns() << 16 ^ std::process::id() as u64).max(1));
        map.set_u64(H_SENDER_PID, std::process::id() as u64);
        map.set_text(H_SENDER_NAME, SENDER_NAME_BYTES, SENDER_NAME);
        map.set_text(H_SENDER_VERSION, SENDER_VERSION_BYTES, concat!(env!("CARGO_PKG_VERSION"), " synthetic"));
        map.set_text(H_HOST_NAME, HOST_NAME_BYTES, "synthetic frame writer");
        map.set_u64(H_MAGIC, MAGIC);

        // Find the Feed that is there now, so it can be told it has been replaced.
        let old = OpenOptions::new().read(true).write(true).open(&path).ok().and_then(|f| {
            (f.metadata().ok()?.len() >= HEADER_BYTES as u64).then(|| Mapping::map(&f, HEADER_BYTES).ok()).flatten()
        });
        std::fs::rename(&tmp, &path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
        if let Some(old) = old
            && old.u64(H_MAGIC) == MAGIC
        {
            old.atomic_u32(H_RETIRED).store(1, Release);
        }
        Ok(FeedWriter { map, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Is a reader attached (heartbeat less than a second old at `now_ns`)? The Sender copies nothing otherwise.
    pub fn reader_present(&self, now_ns: u64) -> bool {
        let hb = self.map.atomic_u64(H_READER_HEARTBEAT_NS).load(Acquire);
        hb != 0 && (now_ns < hb || now_ns - hb < READER_TIMEOUT_NS)
    }

    /// Record the Sender's last error for the app to show (empty = none).
    pub fn set_sender_error(&self, text: &str) {
        self.map.set_text(H_SENDER_ERROR, SENDER_ERROR_BYTES, text);
    }

    /// Put one frame in the Feed without waiting, exactly as the Sender does: a free slot, else the oldest unread
    /// one (counted as a drop). `pixels` is RGBA 32-bit float, tightly packed, bottom row first.
    /// Returns the frame's sequence number.
    pub fn write_frame(&mut self, meta: &FrameMeta, pixels: &[f32]) -> Result<u64, WriteError> {
        let m = &self.map;
        let row_bytes = meta.width as usize * 16;
        let data_bytes = row_bytes * meta.height as usize;
        assert_eq!(pixels.len() * 4, data_bytes, "pixels must be width * height * 4 floats");
        if data_bytes > m.slot_capacity() || m.slot_count() == 0 {
            return Err(WriteError::TooLarge);
        }
        let Some(slot) = self.acquire() else {
            m.atomic_u64(H_DROPS_BUSY).fetch_add(1, Relaxed);
            return Err(WriteError::Dropped);
        };
        let s = m.slot(slot);
        let sequence = m.atomic_u64(H_NEXT_SEQUENCE).fetch_add(1, Relaxed) + 1;
        m.atomic_u64(s + S_SEQUENCE).store(sequence, Release);
        m.set_u32(s + S_PIXEL_FORMAT, PIXEL_RGBA32F);
        m.set_u32(s + S_WIDTH, meta.width);
        m.set_u32(s + S_HEIGHT, meta.height);
        m.set_u32(s + S_ROW_BYTES, row_bytes as u32);
        m.set_u32(s + S_ROW_ORDER, 0);
        m.set_u64(s + S_DATA_BYTES, data_bytes as u64);
        m.set_f64(s + S_FRAME_TIME, meta.frame_time);
        m.set_f64(s + S_FRAME_RATE, meta.frame_rate);
        m.set_u64(s + S_RENDER_NS, meta.render_ns);
        m.set_u32(s + S_SENDER_INSTANCE, meta.sender_instance);
        // SAFETY: the slot is WRITING, so only this writer touches its pixel area; the range was checked.
        unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), m.floats(s + SLOT_HEADER_BYTES, pixels.len()), pixels.len()) };
        m.set_u64(s + S_PUBLISH_NS, now_ns());
        m.atomic_u64(H_FRAMES_PUBLISHED).fetch_add(1, Relaxed);
        m.atomic_u64(H_RENDERS_TOTAL).fetch_add(1, Relaxed);
        m.atomic_u64(H_LAST_RENDER_NS).store(meta.render_ns, Relaxed);
        m.atomic_u32(s + S_STATE).store(READY, Release);
        Ok(sequence)
    }

    fn acquire(&self) -> Option<usize> {
        let m = &self.map;
        let n = m.slot_count();
        for _ in 0..2 {
            for i in 0..n {
                if m.atomic_u32(m.slot(i) + S_STATE).compare_exchange(FREE, WRITING, AcqRel, Acquire).is_ok() {
                    return Some(i);
                }
            }
            let oldest = (0..n)
                .filter(|&i| m.atomic_u32(m.slot(i) + S_STATE).load(Acquire) == READY)
                .min_by_key(|&i| m.atomic_u64(m.slot(i) + S_SEQUENCE).load(Acquire))?;
            if m.atomic_u32(m.slot(oldest) + S_STATE).compare_exchange(READY, WRITING, AcqRel, Acquire).is_ok() {
                m.atomic_u64(H_DROPS_OVERWRITTEN).fetch_add(1, Relaxed);
                return Some(oldest);
            } // else the reader took it between the scan and the swap: look once more
        }
        None
    }
}
