//! The Feed on Linux: a file in `/dev/shm` (POSIX shared memory), mapped with `mmap`. A new Feed is built under
//! a temporary name and renamed over the location, so a reader only ever opens a whole one.
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

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

/// One Feed's shared memory, held open by this process: the Feed file.
pub struct Shared {
    file: File,
    len: u64,
    identity: (u64, u64),
    /// Where a Feed made by [`Shared::create`] waits until it is published.
    staged: Option<PathBuf>,
}

impl Shared {
    /// The Feed at `location` now. `NotFound` when there is none.
    pub fn open(location: &Path) -> io::Result<Shared> {
        let file = OpenOptions::new().read(true).write(true).open(location)?;
        Self::held(file, None)
    }

    /// A new Feed of `bytes` bytes, readable and writable by this user only, not yet at `location`.
    pub fn create(location: &Path, bytes: usize) -> io::Result<Shared> {
        let tmp = PathBuf::from(format!("{}.{}.tmp", location.display(), std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        file.set_len(bytes as u64)?;
        Self::held(file, Some(tmp))
    }

    fn held(file: File, staged: Option<PathBuf>) -> io::Result<Shared> {
        let meta = file.metadata()?;
        Ok(Shared {
            file,
            len: meta.len(),
            identity: (meta.dev(), meta.ino()),
            staged,
        })
    }

    /// Put this Feed (made by [`Shared::create`]) at `location`, in place of whatever Feed was there.
    pub fn publish(&self, location: &Path) -> io::Result<()> {
        let Some(tmp) = &self.staged else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "only a new Feed can be published",
            ));
        };
        std::fs::rename(tmp, location).inspect_err(|_| {
            let _ = std::fs::remove_file(tmp);
        })
    }

    /// Its size in bytes.
    pub fn size(&self) -> u64 {
        self.len
    }

    /// What tells this Feed from another later put at the same location (device and inode).
    pub fn identity(&self) -> (u64, u64) {
        self.identity
    }

    /// Map its first `len` bytes, shared and writable.
    pub fn map(&self, len: usize) -> io::Result<View> {
        // SAFETY: a fresh shared mapping of a file we hold open; failure is checked.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.file.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(View {
            ptr: ptr as *mut u8,
            len,
        })
    }
}

/// The identity of the Feed at `location` now, if there is one.
pub fn current_identity(location: &Path) -> Option<(u64, u64)> {
    std::fs::metadata(location).ok().map(|m| (m.dev(), m.ino()))
}

/// A mapping made by [`Shared::map`]; unmapped when dropped. It outlives the [`Shared`] it came from.
pub struct View {
    ptr: *mut u8,
    len: usize,
}

impl View {
    pub fn ptr(&self) -> *mut u8 {
        self.ptr
    }
}

impl Drop for View {
    fn drop(&mut self) {
        // SAFETY: the mapping made in `Shared::map`, unmapped once.
        unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.len) };
    }
}
