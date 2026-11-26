//! The synthetic frame writer: plays the part of Resolve and the Sender, so everything downstream of the Feed can
//! be tested with no Resolve. It follows the same protocol as the Sender (feed/PROTOCOL.md): the slot protocol,
//! the Sender registry, and the rule that the first running Sender process owns the Feed while Senders in other
//! processes join it as guests.
use crate::SentColourSetup;
use crate::layout::*;
use crate::platform::{create_feed_file, process_alive};
use crate::{
    READER_TIMEOUT_NS, REGISTRY_ENTRIES, SENDER_NAME, VERSION_MAJOR, VERSION_MINOR, now_ns,
};
use std::fs::OpenOptions;
use std::io;
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

/// Which Sender process this writer stands in for.
#[derive(Clone, Debug, PartialEq)]
pub struct SenderIdentity {
    /// Process id of the Sender's host (Resolve).
    pub pid: u64,
    /// Host name and version, as the host reports them.
    pub host: String,
}

impl Default for SenderIdentity {
    fn default() -> Self {
        SenderIdentity {
            pid: std::process::id() as u64,
            host: "synthetic frame writer".into(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum WriteError {
    /// Every slot was being written or read: the frame was skipped and counted as a drop.
    Dropped,
    /// The frame does not fit a slot. Make a new Feed with larger slots.
    TooLarge,
    /// This writer is a guest in another Sender process's Feed: it sends no frames (its render was recorded).
    Guest,
}

pub struct FeedWriter {
    /// The whole Feed file when this writer owns it; only the header when it is a guest.
    map: Mapping,
    path: PathBuf,
    identity: SenderIdentity,
    guest: bool,
    colour_setup: Option<SentColourSetup>,
}

impl FeedWriter {
    /// Publish a new Feed at `path` with `slot_count` slots of `slot_capacity` bytes (rounded up to 4096), as this
    /// process. A Feed already at `path` is replaced and marked retired, whoever owns it.
    pub fn create(
        path: impl AsRef<Path>,
        slot_count: usize,
        slot_capacity: usize,
    ) -> io::Result<FeedWriter> {
        Self::publish(
            path.as_ref(),
            slot_count,
            slot_capacity,
            VERSION_MAJOR,
            VERSION_MINOR,
            SenderIdentity::default(),
            None,
        )
    }

    /// As [`FeedWriter::create`], but claiming another protocol version (to test how readers react).
    pub fn create_with_version(
        path: impl AsRef<Path>,
        slot_count: usize,
        slot_capacity: usize,
        major: u32,
        minor: u32,
    ) -> io::Result<FeedWriter> {
        Self::publish(
            path.as_ref(),
            slot_count,
            slot_capacity,
            major,
            minor,
            SenderIdentity::default(),
            None,
        )
    }

    /// Start as a Sender process does when Resolve loads it: if the Feed at `path` belongs to another Sender
    /// process that is still running, join it as a guest; otherwise publish a Feed with no ring.
    pub fn start(path: impl AsRef<Path>, identity: SenderIdentity) -> io::Result<FeedWriter> {
        let path = path.as_ref();
        if let Some(map) = live_feed_of_another(path, identity.pid) {
            return Ok(FeedWriter {
                map,
                path: path.to_path_buf(),
                identity,
                guest: true,
                colour_setup: None,
            });
        }
        Self::publish(path, 0, 0, VERSION_MAJOR, VERSION_MINOR, identity, None)
    }

    /// Owner only: publish a new Feed file with another ring layout, carrying the counters and the registry over
    /// (as the Sender does when it first builds its ring, or a larger frame arrives).
    pub fn replace(&mut self, slot_count: usize, slot_capacity: usize) -> io::Result<()> {
        assert!(!self.guest, "a guest does not publish Feeds");
        let colour_setup = self.colour_setup.take();
        *self = Self::publish(
            &self.path,
            slot_count,
            slot_capacity,
            VERSION_MAJOR,
            VERSION_MINOR,
            self.identity.clone(),
            Some(&self.map),
        )?;
        self.colour_setup = colour_setup;
        Ok(())
    }

    /// Guest only (the Sender does this about once a second): follow the owner to a new Feed file, or take the
    /// Feed over if the owner's process has gone.
    pub fn refresh(&mut self) {
        if !self.guest {
            return;
        }
        if let Some(map) = live_feed_of_another(&self.path, self.identity.pid) {
            if map.u64(H_FEED_ID) != self.map.u64(H_FEED_ID) {
                let mine = registry_of(&self.map, Some(self.identity.pid));
                self.map = map;
                for e in mine {
                    self.register(e.0, e.1);
                }
            }
            return;
        }
        // The owner has quit: publish a Feed of our own with only our own instances.
        if let Ok(w) = Self::publish(
            &self.path,
            0,
            0,
            VERSION_MAJOR,
            VERSION_MINOR,
            self.identity.clone(),
            None,
        ) {
            let mine = registry_of(&self.map, Some(self.identity.pid));
            *self = w;
            for e in mine {
                self.register(e.0, e.1);
            }
        }
    }

    pub fn is_guest(&self) -> bool {
        self.guest
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Is a reader attached (heartbeat less than a second old at `now_ns`)? The Sender copies nothing otherwise.
    pub fn reader_present(&self, now_ns: u64) -> bool {
        let hb = self.map.atomic_u64(H_READER_HEARTBEAT_NS).load(Acquire);
        hb != 0 && (now_ns < hb || now_ns - hb < READER_TIMEOUT_NS)
    }

    /// The Colour Setup the Sender knows of its own (the manual override), stamped on every frame written from now
    /// on; `None` = none (the app then uses the project's).
    pub fn set_colour_setup(&mut self, setup: Option<SentColourSetup>) {
        self.colour_setup = setup;
    }

    /// Record the Sender's last error for the app to show (empty = none).
    pub fn set_sender_error(&self, text: &str) {
        if !self.guest {
            self.map.set_text(H_SENDER_ERROR, SENDER_ERROR_BYTES, text);
        }
    }

    /// A Sender instance was created: list it in the registry. A full registry is not an error: the instance
    /// just goes unlisted.
    pub fn register(&mut self, instance: u32, frame_rate: f64) {
        let m = &self.map;
        let pid = self.identity.pid;
        if find_entry(m, pid, instance).is_some() {
            return;
        }
        for i in 0..REGISTRY_ENTRIES {
            let e = entry(i);
            if m.atomic_u32(e + R_STATE)
                .compare_exchange(ENTRY_FREE, ENTRY_CLAIMING, AcqRel, Acquire)
                .is_ok()
            {
                m.set_u32(e + R_INSTANCE, instance);
                m.set_u64(e + R_PID, pid);
                m.set_f64(e + R_FRAME_RATE, frame_rate);
                m.set_u64(e + R_REGISTERED_NS, now_ns());
                m.atomic_u64(e + R_LAST_RENDER_NS).store(0, Relaxed);
                m.set_f64(e + R_LAST_FRAME_TIME, 0.0);
                m.atomic_u64(e + R_RENDERS).store(0, Relaxed);
                m.set_text(e + R_HOST_NAME, HOST_NAME_BYTES, &self.identity.host);
                m.atomic_u32(e + R_STATE).store(ENTRY_LIVE, Release);
                return;
            }
        }
    }

    /// A Sender instance was destroyed: take it off the registry.
    pub fn unregister(&mut self, instance: u32) {
        if let Some(i) = find_entry(&self.map, self.identity.pid, instance) {
            self.map
                .atomic_u32(entry(i) + R_STATE)
                .store(ENTRY_FREE, Release);
        }
    }

    /// Record a render call of `instance` in its registry entry (every render, fed or not, as the Sender does).
    pub fn note_render(&self, instance: u32, frame_time: f64, render_ns: u64) {
        if let Some(i) = find_entry(&self.map, self.identity.pid, instance) {
            let e = entry(i);
            self.map.set_f64(e + R_LAST_FRAME_TIME, frame_time);
            self.map.atomic_u64(e + R_RENDERS).fetch_add(1, Relaxed);
            self.map
                .atomic_u64(e + R_LAST_RENDER_NS)
                .store(render_ns, Release);
        }
    }

    /// The Sender could not take a frame without waiting (its staging copy was still busy, or another render
    /// thread held the Feed): the frame is skipped and counted as a busy drop, as the Sender does.
    pub fn skip_frame(&self) {
        if self.guest {
            return;
        }
        self.map.atomic_u64(H_DROPS_BUSY).fetch_add(1, Relaxed);
        self.map.atomic_u64(H_RENDERS_TOTAL).fetch_add(1, Relaxed);
    }

    /// Put one frame in the Feed without waiting, exactly as the Sender does: a free slot, else the oldest unread
    /// one (counted as a drop). `pixels` is RGBA 32-bit float, tightly packed, bottom row first.
    /// Returns the frame's sequence number. A guest only records the render.
    pub fn write_frame(&mut self, meta: &FrameMeta, pixels: &[f32]) -> Result<u64, WriteError> {
        self.note_render(meta.sender_instance, meta.frame_time, meta.render_ns);
        if self.guest {
            return Err(WriteError::Guest);
        }
        let m = &self.map;
        let row_bytes = meta.width as usize * 16;
        let data_bytes = row_bytes * meta.height as usize;
        assert_eq!(
            pixels.len() * 4,
            data_bytes,
            "pixels must be width * height * 4 floats"
        );
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
        SentColourSetup::store(self.colour_setup.as_ref(), m, s);
        // SAFETY: the slot is WRITING, so only this writer touches its pixel area; the range was checked.
        unsafe {
            std::ptr::copy_nonoverlapping(
                pixels.as_ptr(),
                m.floats(s + SLOT_HEADER_BYTES, pixels.len()),
                pixels.len(),
            )
        };
        m.set_u64(s + S_PUBLISH_NS, now_ns());
        m.atomic_u64(H_FRAMES_PUBLISHED).fetch_add(1, Relaxed);
        m.atomic_u64(H_RENDERS_TOTAL).fetch_add(1, Relaxed);
        m.atomic_u64(H_LAST_RENDER_NS)
            .store(meta.render_ns, Relaxed);
        m.atomic_u32(s + S_STATE).store(READY, Release);
        Ok(sequence)
    }

    fn acquire(&self) -> Option<usize> {
        let m = &self.map;
        let n = m.slot_count();
        for _ in 0..2 {
            for i in 0..n {
                if m.atomic_u32(m.slot(i) + S_STATE)
                    .compare_exchange(FREE, WRITING, AcqRel, Acquire)
                    .is_ok()
                {
                    return Some(i);
                }
            }
            let oldest = (0..n)
                .filter(|&i| m.atomic_u32(m.slot(i) + S_STATE).load(Acquire) == READY)
                .min_by_key(|&i| m.atomic_u64(m.slot(i) + S_SEQUENCE).load(Acquire))?;
            if m.atomic_u32(m.slot(oldest) + S_STATE)
                .compare_exchange(READY, WRITING, AcqRel, Acquire)
                .is_ok()
            {
                m.atomic_u64(H_DROPS_OVERWRITTEN).fetch_add(1, Relaxed);
                return Some(oldest);
            } // else the reader took it between the scan and the swap: look once more
        }
        None
    }

    /// Build a Feed file under a temporary name, rename it over `path` and retire the Feed that was there.
    /// `carry`: the previous Feed of the same Sender, whose counters and registry continue in the new file.
    fn publish(
        path: &Path,
        slot_count: usize,
        slot_capacity: usize,
        major: u32,
        minor: u32,
        identity: SenderIdentity,
        carry: Option<&Mapping>,
    ) -> io::Result<FeedWriter> {
        let path = path.to_path_buf();
        let slot_capacity = slot_capacity.div_ceil(4096) * 4096;
        let bytes = file_bytes(slot_count, slot_capacity);
        let tmp = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let file = create_feed_file(&tmp)?;
        file.set_len(bytes as u64)?;
        let map = Mapping::map(&file, bytes)?;
        map.set_u32(H_VERSION_MAJOR, major);
        map.set_u32(H_VERSION_MINOR, minor);
        map.set_u32(H_HEADER_BYTES, HEADER_BYTES as u32);
        map.set_u32(H_SLOT_HEADER_BYTES, SLOT_HEADER_BYTES as u32);
        map.set_u32(H_SLOT_COUNT, slot_count as u32);
        map.set_u64(H_SLOT_CAPACITY, slot_capacity as u64);
        map.set_u64(
            H_FEED_ID,
            (now_ns() << 16 ^ std::process::id() as u64 ^ identity.pid).max(1),
        );
        map.set_u64(H_SENDER_PID, identity.pid);
        map.set_text(H_SENDER_NAME, SENDER_NAME_BYTES, SENDER_NAME);
        map.set_text(
            H_SENDER_VERSION,
            SENDER_VERSION_BYTES,
            concat!(env!("CARGO_PKG_VERSION"), " synthetic"),
        );
        map.set_text(H_HOST_NAME, HOST_NAME_BYTES, &identity.host);
        if let Some(old) = carry {
            for off in [
                H_NEXT_SEQUENCE,
                H_FRAMES_PUBLISHED,
                H_DROPS_OVERWRITTEN,
                H_DROPS_BUSY,
                H_RENDERS_TOTAL,
                H_RENDERS_THUMBNAIL,
                H_RENDERS_IDLE,
                H_RENDERS_UNSUPPORTED,
                H_RENDER_CALL_NS_TOTAL,
                H_LAST_RENDER_NS,
            ] {
                map.set_u64(off, old.atomic_u64(off).load(Relaxed));
            }
            copy_registry(old, &map);
        }
        map.set_u64(H_MAGIC, MAGIC);

        // Find the Feed that is there now, so it can be told it has been replaced.
        let old = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .ok()
            .and_then(|f| {
                (f.metadata().ok()?.len() >= HEADER_BYTES as u64)
                    .then(|| Mapping::map(&f, HEADER_BYTES).ok())
                    .flatten()
            });
        std::fs::rename(&tmp, &path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
        if let Some(old) = old
            && old.u64(H_MAGIC) == MAGIC
        {
            old.atomic_u32(H_RETIRED).store(1, Release);
        }
        Ok(FeedWriter {
            map,
            path,
            identity,
            guest: false,
            colour_setup: None,
        })
    }
}

/// The header of the Feed at `path`, if it is a live Feed of this major version owned by a running process
/// other than `my_pid`: then a Sender starting in `my_pid` must join it as a guest, not replace it.
fn live_feed_of_another(path: &Path, my_pid: u64) -> Option<Mapping> {
    let f = OpenOptions::new().read(true).write(true).open(path).ok()?;
    if f.metadata().ok()?.len() < HEADER_BYTES as u64 {
        return None;
    }
    let m = Mapping::map(&f, HEADER_BYTES).ok()?;
    let owner = m.u64(H_SENDER_PID);
    let live = m.u64(H_MAGIC) == MAGIC
        && m.u32(H_VERSION_MAJOR) == VERSION_MAJOR
        && m.u32(H_VERSION_MINOR) >= 2
        && m.atomic_u32(H_RETIRED).load(Acquire) == 0
        && owner != my_pid
        && process_alive(owner);
    live.then_some(m)
}

fn find_entry(m: &Mapping, pid: u64, instance: u32) -> Option<usize> {
    (0..REGISTRY_ENTRIES).find(|&i| {
        let e = entry(i);
        m.atomic_u32(e + R_STATE).load(Acquire) == ENTRY_LIVE
            && m.u64(e + R_PID) == pid
            && m.u32(e + R_INSTANCE) == instance
    })
}

/// (instance, frame rate) of the live entries, of one process or all.
fn registry_of(m: &Mapping, pid: Option<u64>) -> Vec<(u32, f64)> {
    (0..REGISTRY_ENTRIES)
        .map(entry)
        .filter(|&e| {
            m.atomic_u32(e + R_STATE).load(Acquire) == ENTRY_LIVE
                && pid.is_none_or(|p| m.u64(e + R_PID) == p)
        })
        .map(|e| (m.u32(e + R_INSTANCE), m.f64(e + R_FRAME_RATE)))
        .collect()
}

/// Copy the live registry entries of `from` into the same places in the new header `to`.
fn copy_registry(from: &Mapping, to: &Mapping) {
    for i in 0..REGISTRY_ENTRIES {
        let e = entry(i);
        if from.atomic_u32(e + R_STATE).load(Acquire) != ENTRY_LIVE {
            continue;
        }
        for off in (8..R_ENTRY_BYTES).step_by(8) {
            to.set_u64(e + off, from.u64(e + off));
        }
        to.set_u32(e + R_INSTANCE, from.u32(e + R_INSTANCE));
        to.set_u32(e + R_STATE, ENTRY_LIVE);
    }
}
