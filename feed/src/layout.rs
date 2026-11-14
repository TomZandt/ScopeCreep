//! Byte layout of a Feed file (feed/PROTOCOL.md) and the shared mapping both sides work through.
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicU32, AtomicU64};

pub const MAGIC: u64 = 0x0000_4445_4546_4353; // "SCFEED\0\0"
pub const HEADER_BYTES: usize = 4096;
pub const SLOT_HEADER_BYTES: usize = 4096;

// Feed header offsets
pub const H_MAGIC: usize = 0;
pub const H_VERSION_MAJOR: usize = 8;
pub const H_VERSION_MINOR: usize = 12;
pub const H_HEADER_BYTES: usize = 16;
pub const H_SLOT_HEADER_BYTES: usize = 20;
pub const H_SLOT_COUNT: usize = 24;
pub const H_RETIRED: usize = 28;
pub const H_SLOT_CAPACITY: usize = 32;
pub const H_FEED_ID: usize = 40;
pub const H_SENDER_PID: usize = 48;
pub const H_SENDER_NAME: usize = 64;
pub const H_SENDER_VERSION: usize = 128;
pub const H_HOST_NAME: usize = 160;
pub const H_READER_HEARTBEAT_NS: usize = 256;
pub const H_READER_PID: usize = 264;
pub const H_NEXT_SEQUENCE: usize = 320;
pub const H_FRAMES_PUBLISHED: usize = 328;
pub const H_DROPS_OVERWRITTEN: usize = 336;
pub const H_DROPS_BUSY: usize = 344;
pub const H_RENDERS_TOTAL: usize = 352;
pub const H_RENDERS_THUMBNAIL: usize = 360;
pub const H_RENDERS_IDLE: usize = 368;
pub const H_RENDERS_UNSUPPORTED: usize = 376;
pub const H_RENDER_CALL_NS_TOTAL: usize = 384;
pub const H_LAST_RENDER_NS: usize = 392;
pub const H_SENDER_ERROR: usize = 512;
pub const SENDER_NAME_BYTES: usize = 64;
pub const SENDER_VERSION_BYTES: usize = 32;
pub const HOST_NAME_BYTES: usize = 64;
pub const SENDER_ERROR_BYTES: usize = 256;

// Slot header offsets
pub const S_STATE: usize = 0;
pub const S_PIXEL_FORMAT: usize = 4;
pub const S_SEQUENCE: usize = 8;
pub const S_WIDTH: usize = 16;
pub const S_HEIGHT: usize = 20;
pub const S_ROW_BYTES: usize = 24;
pub const S_ROW_ORDER: usize = 28;
pub const S_DATA_BYTES: usize = 32;
pub const S_FRAME_TIME: usize = 40;
pub const S_FRAME_RATE: usize = 48;
pub const S_RENDER_NS: usize = 56;
pub const S_PUBLISH_NS: usize = 64;
pub const S_SENDER_INSTANCE: usize = 72;

pub const FREE: u32 = 0;
pub const WRITING: u32 = 1;
pub const READY: u32 = 2;
pub const READING: u32 = 3;

pub const PIXEL_RGBA32F: u32 = 1;

/// A shared, writable mapping of (part of) a Feed file. Plain fields are read and written unsynchronised under
/// the slot protocol (only the side that owns a slot touches its plain fields); shared fields go through atomics.
pub struct Mapping {
    ptr: *mut u8,
    len: usize,
}

// The mapping is shared memory: all access goes through atomics or the slot ownership protocol.
unsafe impl Send for Mapping {}
unsafe impl Sync for Mapping {}

impl Mapping {
    pub fn map(file: &File, len: usize) -> io::Result<Mapping> {
        // SAFETY: a fresh shared mapping of a file we hold open; failure is checked.
        let ptr = unsafe {
            libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, file.as_raw_fd(), 0)
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Mapping { ptr: ptr as *mut u8, len })
    }

    fn check(&self, off: usize, size: usize) {
        assert!(off.checked_add(size).is_some_and(|end| end <= self.len), "Feed access out of range: {off}+{size} > {}", self.len);
    }

    pub fn u32(&self, off: usize) -> u32 {
        self.check(off, 4);
        // SAFETY: in range (checked), 4-byte aligned by the layout.
        unsafe { (self.ptr.add(off) as *const u32).read_volatile() }
    }
    pub fn u64(&self, off: usize) -> u64 {
        self.check(off, 8);
        // SAFETY: in range (checked), 8-byte aligned by the layout.
        unsafe { (self.ptr.add(off) as *const u64).read_volatile() }
    }
    pub fn f64(&self, off: usize) -> f64 {
        f64::from_bits(self.u64(off))
    }
    pub fn set_u32(&self, off: usize, v: u32) {
        self.check(off, 4);
        // SAFETY: as `u32`.
        unsafe { (self.ptr.add(off) as *mut u32).write_volatile(v) }
    }
    pub fn set_u64(&self, off: usize, v: u64) {
        self.check(off, 8);
        // SAFETY: as `u64`.
        unsafe { (self.ptr.add(off) as *mut u64).write_volatile(v) }
    }
    pub fn set_f64(&self, off: usize, v: f64) {
        self.set_u64(off, v.to_bits());
    }
    pub fn atomic_u32(&self, off: usize) -> &AtomicU32 {
        self.check(off, 4);
        // SAFETY: in range, aligned, and the memory lives as long as `self`.
        unsafe { AtomicU32::from_ptr(self.ptr.add(off) as *mut u32) }
    }
    pub fn atomic_u64(&self, off: usize) -> &AtomicU64 {
        self.check(off, 8);
        // SAFETY: in range, aligned, and the memory lives as long as `self`.
        unsafe { AtomicU64::from_ptr(self.ptr.add(off) as *mut u64) }
    }

    /// NUL-padded UTF-8 text field.
    pub fn text(&self, off: usize, cap: usize) -> String {
        self.check(off, cap);
        // SAFETY: in range; bytes may change under us, which can only garble the text.
        let bytes: Vec<u8> = (0..cap).map(|i| unsafe { self.ptr.add(off + i).read_volatile() }).collect();
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(cap);
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
    pub fn set_text(&self, off: usize, cap: usize, text: &str) {
        self.check(off, cap);
        let src = text.as_bytes();
        let mut n = src.len().min(cap - 1);
        while !text.is_char_boundary(n) {
            n -= 1;
        }
        for i in 0..cap {
            // SAFETY: in range.
            unsafe { self.ptr.add(off + i).write_volatile(if i < n { src[i] } else { 0 }) }
        }
    }

    /// The pixel area of a slot as floats. The caller owns the slot (READING or WRITING).
    pub(crate) fn floats(&self, off: usize, count: usize) -> *mut f32 {
        self.check(off, count * 4);
        // SAFETY: in range.
        unsafe { self.ptr.add(off) as *mut f32 }
    }

    pub fn slot_count(&self) -> usize {
        self.u32(H_SLOT_COUNT) as usize
    }
    pub fn slot_capacity(&self) -> usize {
        self.u64(H_SLOT_CAPACITY) as usize
    }
    /// Offset of slot `i`'s header.
    pub fn slot(&self, i: usize) -> usize {
        HEADER_BYTES + i * (SLOT_HEADER_BYTES + self.slot_capacity())
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: the mapping made in `map`, unmapped once.
        unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.len) };
    }
}

/// Total size of a Feed file with this layout.
pub fn file_bytes(slot_count: usize, slot_capacity: usize) -> usize {
    HEADER_BYTES + slot_count * (SLOT_HEADER_BYTES + slot_capacity)
}
