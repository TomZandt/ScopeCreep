//! The Feed on Linux: a file in `/dev/shm` (POSIX shared memory), mapped with `mmap`.
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

/// Where the Feed lives unless `SCOPECREEP_FEED` says otherwise.
pub const DEFAULT_LOCATION: &str = "/dev/shm/scopecreep-feed";

/// The system-wide monotonic clock in nanoseconds: the clock every `_ns` field of the Feed uses.
pub fn now_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid timespec to fill in.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Is process `pid` running? (Signal 0 only checks; EPERM means it exists but belongs to someone else.)
pub fn process_alive(pid: u64) -> bool {
    // SAFETY: signal 0 sends nothing.
    pid != 0
        && pid <= i32::MAX as u64
        && (unsafe { libc::kill(pid as libc::pid_t, 0) } == 0
            || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
}

/// Make a new Feed file at `path`, readable and writable by this user only. Fails if one is there.
pub fn create_feed_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

/// What tells one Feed file from another later put at the same path (device and inode).
pub fn file_identity(meta: &Metadata) -> (u64, u64) {
    (meta.dev(), meta.ino())
}

/// Map the first `len` bytes of `file`, shared and writable.
pub fn map_shared(file: &File, len: usize) -> io::Result<*mut u8> {
    // SAFETY: a fresh shared mapping of a file the caller holds open; failure is checked.
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            file.as_raw_fd(),
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return Err(io::Error::last_os_error());
    }
    Ok(ptr as *mut u8)
}

/// Undo [`map_shared`].
///
/// # Safety
/// `ptr` and `len` are one mapping from [`map_shared`], unmapped once, with nothing using it afterwards.
pub unsafe fn unmap(ptr: *mut u8, len: usize) {
    // SAFETY: as the caller promises.
    unsafe { libc::munmap(ptr as *mut libc::c_void, len) };
}
