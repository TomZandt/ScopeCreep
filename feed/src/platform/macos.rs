//! The Feed on macOS: POSIX shared-memory objects (`shm_open`) mapped with `mmap` (ADR 0043).
//!
//! macOS cannot rename a shared-memory object, so a new Feed cannot replace the old one under the same name the
//! way Linux renames a file over another. The name `/scopecreep-feed` therefore holds a small **locator** and
//! each Feed lives at the name plus `.N`, as on Windows (ADR 0049; ADR 0052, feed/PROTOCOL.md "Locator").
//! Publishing a Feed is storing its N in the locator. Unlike a Windows mapping, an object here lasts until it is
//! unlinked, so publishing also unlinks the Feed it replaces (whoever has it mapped keeps it until they let go).
//!
//! Two limits of macOS shared memory shape the code: a name is at most 31 bytes including the leading `/` (so a
//! `SCOPECREEP_FEED` name must be short enough for `.N` to fit), and an object can be sized only once.
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use std::time::{Duration, Instant};

/// Where the Feed lives unless `SCOPECREEP_FEED` says otherwise: the locator's name.
pub const DEFAULT_LOCATION: &str = "/scopecreep-feed";

/// The locator's size: one page. Offset 0 is the current Feed's N (0 = none), offset 8 the last N handed out.
const LOCATOR_BYTES: usize = 4096;
const L_CURRENT: usize = 0;
const L_LAST: usize = 8;

/// How long to wait for another process to size an object it has just made.
const SIZING_WAIT: Duration = Duration::from_millis(500);

/// `name` as `shm_open` wants it.
fn c_name(name: &[u8]) -> io::Result<CString> {
    CString::new(name).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul in a name"))
}

/// The name of the locator at `location`.
fn locator_name(location: &Path) -> io::Result<CString> {
    c_name(location.as_os_str().as_bytes())
}

/// The name of Feed `n` under the locator at `location`.
fn feed_name(location: &Path, n: u64) -> io::Result<CString> {
    let mut name = location.as_os_str().as_bytes().to_vec();
    name.extend_from_slice(format!(".{n}").as_bytes());
    c_name(&name)
}

/// `shm_open` with `flags`, readable and writable, private to this user.
fn shm_open(name: &CString, flags: libc::c_int) -> io::Result<OwnedFd> {
    // SAFETY: a nul-terminated name; the mode matters for a new object only. macOS declares `shm_open` variadic,
    // so the mode goes as the int it is promoted to.
    let fd = unsafe { libc::shm_open(name.as_ptr(), flags | libc::O_RDWR, 0o600 as libc::c_uint) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a descriptor just opened, owned by nobody else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn unlink(name: &CString) {
    // SAFETY: a nul-terminated name.
    unsafe { libc::shm_unlink(name.as_ptr()) };
}

/// The size of the object `fd`, in bytes (macOS rounds it up to a page).
fn size_of_object(fd: &OwnedFd) -> io::Result<u64> {
    // SAFETY: a descriptor we hold and a stat block to fill in.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd.as_raw_fd(), &mut st) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(st.st_size as u64)
}

/// Size a new object once. When another process got there first (an object can be sized only once) this waits for
/// that process's size instead.
fn size_once(fd: &OwnedFd, bytes: usize) -> io::Result<()> {
    // SAFETY: a descriptor we hold.
    if unsafe { libc::ftruncate(fd.as_raw_fd(), bytes as libc::off_t) } == 0 {
        return Ok(());
    }
    let e = io::Error::last_os_error();
    wait_sized(fd).map_err(|_| e)
}

/// Wait until the object has a size: it is made and sized in two steps, and touching a page of an unsized object
/// is a bus error, not an error code.
fn wait_sized(fd: &OwnedFd) -> io::Result<()> {
    let deadline = Instant::now() + SIZING_WAIT;
    while size_of_object(fd)? == 0 {
        if Instant::now() > deadline {
            return Err(io::ErrorKind::NotFound.into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

/// Open the locator at `location`. `NotFound` when there is none.
fn open_locator(location: &Path) -> io::Result<OwnedFd> {
    let fd = shm_open(&locator_name(location)?, 0)?;
    wait_sized(&fd)?;
    Ok(fd)
}

/// Open the locator at `location`, making it if no process has.
fn make_locator(location: &Path) -> io::Result<OwnedFd> {
    let fd = shm_open(&locator_name(location)?, libc::O_CREAT)?;
    if size_of_object(&fd)? == 0 {
        size_once(&fd, LOCATOR_BYTES)?;
    }
    Ok(fd)
}

/// Map `len` bytes of `fd`, readable and writable.
fn map_fd(fd: &OwnedFd, len: usize) -> io::Result<*mut u8> {
    // SAFETY: a fresh shared mapping of an object we hold open; failure is checked.
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return Err(io::Error::last_os_error());
    }
    Ok(ptr as *mut u8)
}

/// Run `f` on one of the locator's counters.
fn with_locator<T>(
    locator: &OwnedFd,
    offset: usize,
    f: impl FnOnce(&AtomicU64) -> T,
) -> io::Result<T> {
    let ptr = map_fd(locator, LOCATOR_BYTES)?;
    // SAFETY: the view is a page, page-aligned, so the 8-byte counter at `offset` is in range and aligned; other
    // processes touch it only atomically.
    let result = f(unsafe { AtomicU64::from_ptr(ptr.add(offset) as *mut u64) });
    // SAFETY: the view just made, no longer used.
    unsafe { libc::munmap(ptr as *mut libc::c_void, LOCATOR_BYTES) };
    Ok(result)
}

/// One Feed's shared memory, held open by this process: Feed `n` under its locator.
pub struct Shared {
    locator: OwnedFd,
    section: OwnedFd,
    n: u64,
    len: u64,
    /// The location a Feed made by [`Shared::create`] was made for, where it may be published.
    staged: Option<PathBuf>,
}

impl Shared {
    /// The Feed at `location` now. `NotFound` when there is none.
    pub fn open(location: &Path) -> io::Result<Shared> {
        let locator = open_locator(location)?;
        let n = with_locator(&locator, L_CURRENT, |c| c.load(Acquire))?;
        if n == 0 {
            return Err(io::ErrorKind::NotFound.into());
        }
        let section = shm_open(&feed_name(location, n)?, 0)?;
        wait_sized(&section)?;
        let len = size_of_object(&section)?;
        Ok(Shared {
            locator,
            section,
            n,
            len,
            staged: None,
        })
    }

    /// A new Feed of `bytes` bytes, not yet at `location`: the next free N under the locator, which is made if
    /// it is not there.
    pub fn create(location: &Path, bytes: usize) -> io::Result<Shared> {
        let locator = make_locator(location)?;
        for _ in 0..16 {
            let n = with_locator(&locator, L_LAST, |c| c.fetch_add(1, AcqRel) + 1)?;
            let name = feed_name(location, n)?;
            match shm_open(&name, libc::O_CREAT | libc::O_EXCL) {
                Ok(section) => {
                    if let Err(e) = size_once(&section, bytes) {
                        unlink(&name);
                        return Err(e);
                    }
                    return Ok(Shared {
                        locator,
                        section,
                        n,
                        len: bytes as u64,
                        staged: Some(location.to_path_buf()),
                    });
                }
                // A Feed left from an earlier locator is still there: take the next N.
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(io::ErrorKind::AlreadyExists.into())
    }

    /// Put this Feed (made by [`Shared::create`]) at `location`, in place of whatever Feed was there, which is
    /// unlinked. Its locator is the one at the location it was made for, so it can be published only there.
    pub fn publish(&self, location: &Path) -> io::Result<()> {
        if self.staged.as_deref() != Some(location) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "only a new Feed can be published, at the location it was made for",
            ));
        }
        let before = with_locator(&self.locator, L_CURRENT, |c| c.swap(self.n, Release))?;
        if before != 0 && before != self.n {
            unlink(&feed_name(location, before)?);
        }
        Ok(())
    }

    /// Its size in bytes (rounded up to a page for a Feed opened rather than made).
    pub fn size(&self) -> u64 {
        self.len
    }

    /// What tells this Feed from another later put at the same location: its N.
    pub fn identity(&self) -> (u64, u64) {
        (0, self.n)
    }

    /// Map its first `len` bytes, shared and writable.
    pub fn map(&self, len: usize) -> io::Result<View> {
        Ok(View {
            ptr: map_fd(&self.section, len)?,
            len,
        })
    }
}

/// The identity of the Feed at `location` now, if there is one.
pub fn current_identity(location: &Path) -> Option<(u64, u64)> {
    let locator = open_locator(location).ok()?;
    let n = with_locator(&locator, L_CURRENT, |c| c.load(Acquire)).ok()?;
    (n != 0).then_some((0, n))
}

/// Remove the Feed at `location`, its locator and every Feed made under it (tests and tools). Shared memory here
/// outlasts its processes, so a test that does not clean up leaves its Feed until the next restart. A Feed in
/// use stays mapped for whoever has it.
pub fn remove(location: &Path) {
    if let Ok(locator) = open_locator(location) {
        let last = with_locator(&locator, L_LAST, |c| c.load(Acquire)).unwrap_or(0);
        for n in 1..=last {
            if let Ok(name) = feed_name(location, n) {
                unlink(&name);
            }
        }
    }
    if let Ok(name) = locator_name(location) {
        unlink(&name);
    }
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
