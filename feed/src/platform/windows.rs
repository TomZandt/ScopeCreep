//! The Feed on Windows: named file mappings backed by the paging file (ADR 0043), in the session's `Local\`
//! namespace.
//!
//! A named mapping cannot be renamed, and its name stays taken while any process holds it, so a new Feed cannot
//! replace the old one under the same name the way Linux renames a file over another. Instead the name itself
//! (`Local\scopecreep-feed`) holds a small locator and each Feed lives at the name plus `.N` (feed/PROTOCOL.md,
//! "Windows"). Publishing a Feed is storing its N in the locator. Every handle a process holds on a Feed also
//! holds the locator, so the locator lives exactly as long as some process uses a Feed.
use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use std::sync::{Arc, OnceLock};

use crate::layout::{
    H_HEADER_BYTES, H_MAGIC, H_SLOT_CAPACITY, H_SLOT_COUNT, H_SLOT_HEADER_BYTES, HEADER_BYTES,
    MAGIC, Mapping, SLOT_HEADER_BYTES,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, SetLastError, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile,
};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

/// Where the Feed lives unless `SCOPECREEP_FEED` says otherwise: the locator's name.
pub const DEFAULT_LOCATION: &str = "Local\\scopecreep-feed";

/// The locator's size: one page. Offset 0 is the current Feed's N (0 = none), offset 8 the last N handed out.
const LOCATOR_BYTES: usize = 4096;
const L_CURRENT: usize = 0;
const L_LAST: usize = 8;

/// The system-wide monotonic clock in nanoseconds: the clock every `_ns` field of the Feed uses
/// (`QueryPerformanceCounter`, the same in every process).
pub fn now_ns() -> u64 {
    static FREQUENCY: OnceLock<u64> = OnceLock::new();
    let frequency = *FREQUENCY.get_or_init(|| {
        let mut f = 0i64;
        // SAFETY: a valid i64 to fill in; it cannot fail on Windows XP or later.
        unsafe { QueryPerformanceFrequency(&mut f) };
        (f as u64).max(1)
    });
    let mut count = 0i64;
    // SAFETY: as above.
    unsafe { QueryPerformanceCounter(&mut count) };
    (count as u128 * 1_000_000_000 / frequency as u128) as u64
}

/// Is process `pid` running? A process we may not query (another user's, or elevated) exists, so it counts.
pub fn process_alive(pid: u64) -> bool {
    if pid == 0 || pid > u32::MAX as u64 {
        return false;
    }
    // SAFETY: plain calls; the handle is closed by `Handle`.
    unsafe {
        let h = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid as u32,
        );
        if h.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let h = Handle(h);
        // A process that has ended is signalled; one still running times out.
        WaitForSingleObject(h.0, 0) == WAIT_TIMEOUT
    }
}

/// A kernel handle, closed when dropped.
struct Handle(HANDLE);

// A handle is a plain number that any thread may use.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: a handle this process opened, closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// `name` as Windows wants it: UTF-16 and nul-terminated.
fn wide(name: &OsStr) -> Vec<u16> {
    name.encode_wide().chain(Some(0)).collect()
}

/// The name of Feed `n` under the locator at `location`.
fn feed_name(location: &Path, n: u64) -> Vec<u16> {
    let mut name = location.as_os_str().to_owned();
    name.push(format!(".{n}"));
    wide(&name)
}

/// Open the existing mapping `name`. `NotFound` when there is none.
fn open_mapping(name: &[u16]) -> io::Result<Handle> {
    // SAFETY: a nul-terminated name.
    let h = unsafe { OpenFileMappingW(FILE_MAP_READ | FILE_MAP_WRITE, 0, name.as_ptr()) };
    if h.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(Handle(h))
}

/// Make the mapping `name` of `bytes` bytes in the paging file, or open it if it is there; true if it was there.
fn create_mapping(name: &[u16], bytes: usize) -> io::Result<(Handle, bool)> {
    let bytes = bytes as u64;
    // SAFETY: a nul-terminated name; default security (this user). The last error is cleared first, since only
    // ERROR_ALREADY_EXISTS tells a mapping that was there from a new one.
    unsafe {
        SetLastError(0);
        let h = CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            std::ptr::null(),
            PAGE_READWRITE,
            (bytes >> 32) as u32,
            bytes as u32,
            name.as_ptr(),
        );
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        let existed = GetLastError() == ERROR_ALREADY_EXISTS;
        Ok((Handle(h), existed))
    }
}

/// Map `len` bytes of `section` (0 = all of it), readable and writable.
fn map_view(section: &Handle, len: usize) -> io::Result<*mut u8> {
    // SAFETY: a mapping handle we hold; failure is checked.
    let view = unsafe { MapViewOfFile(section.0, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, len) };
    if view.Value.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(view.Value as *mut u8)
}

/// # Safety
/// `ptr` is a view from [`map_view`], unmapped once, with nothing using it afterwards.
unsafe fn unmap_view(ptr: *mut u8) {
    // SAFETY: as the caller promises.
    unsafe {
        UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
            Value: ptr as *mut core::ffi::c_void,
        })
    };
}

/// Run `f` on one of the locator's counters.
fn with_locator<T>(
    locator: &Handle,
    offset: usize,
    f: impl FnOnce(&AtomicU64) -> T,
) -> io::Result<T> {
    let ptr = map_view(locator, LOCATOR_BYTES)?;
    // SAFETY: the view is a page, page-aligned, so the 8-byte counter at `offset` is in range and aligned; other
    // processes touch it only atomically.
    let result = f(unsafe { AtomicU64::from_ptr(ptr.add(offset) as *mut u64) });
    // SAFETY: the view just made, no longer used.
    unsafe { unmap_view(ptr) };
    Ok(result)
}

/// The handles a Feed in use keeps open: its locator and its own mapping.
struct Held {
    locator: Handle,
    section: Handle,
}

/// One Feed's shared memory, held open by this process: Feed `n` under its locator.
pub struct Shared {
    held: Arc<Held>,
    n: u64,
    len: u64,
    /// The location a Feed made by [`Shared::create`] was made for, where it may be published.
    staged: Option<PathBuf>,
}

impl Shared {
    /// The Feed at `location` now. `NotFound` when there is none.
    pub fn open(location: &Path) -> io::Result<Shared> {
        let locator = open_mapping(&wide(location.as_os_str()))?;
        let n = with_locator(&locator, L_CURRENT, |c| c.load(Acquire))?;
        if n == 0 {
            return Err(io::ErrorKind::NotFound.into());
        }
        let section = open_mapping(&feed_name(location, n))?;
        let mut shared = Shared {
            held: Arc::new(Held { locator, section }),
            n,
            len: HEADER_BYTES as u64,
            staged: None,
        };
        shared.len = feed_bytes(&shared)?;
        Ok(shared)
    }

    /// A new Feed of `bytes` bytes, not yet at `location`: the next free N under the locator, which is made if
    /// no process holds one.
    pub fn create(location: &Path, bytes: usize) -> io::Result<Shared> {
        let (locator, _) = create_mapping(&wide(location.as_os_str()), LOCATOR_BYTES)?;
        for _ in 0..16 {
            let n = with_locator(&locator, L_LAST, |c| c.fetch_add(1, AcqRel) + 1)?;
            let (section, existed) = create_mapping(&feed_name(location, n), bytes)?;
            if !existed {
                return Ok(Shared {
                    held: Arc::new(Held { locator, section }),
                    n,
                    len: bytes as u64,
                    staged: Some(location.to_path_buf()),
                });
            } // else a Feed left from an earlier locator is still held: take the next N
        }
        Err(io::ErrorKind::AlreadyExists.into())
    }

    /// Put this Feed (made by [`Shared::create`]) at `location`, in place of whatever Feed was there. Its
    /// locator is the one at the location it was made for, so it can be published only there.
    pub fn publish(&self, location: &Path) -> io::Result<()> {
        if self.staged.as_deref() != Some(location) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "only a new Feed can be published, at the location it was made for",
            ));
        }
        with_locator(&self.held.locator, L_CURRENT, |c| c.store(self.n, Release))
    }

    /// Its size in bytes.
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
            ptr: map_view(&self.held.section, len)?,
            _held: self.held.clone(),
        })
    }
}

/// A Feed's size, read from its header. Windows tells a mapping's size only by mapping all of it, which reserves
/// the whole Feed's address space to read one number. A mapping that does not start with a Feed header counts as
/// the one page its header would be; the reader then refuses it as not a Feed.
fn feed_bytes(shared: &Shared) -> io::Result<u64> {
    let header = Mapping::map(shared, HEADER_BYTES)?;
    let is_feed = header.u64(H_MAGIC) == MAGIC
        && header.u32(H_HEADER_BYTES) as usize == HEADER_BYTES
        && header.u32(H_SLOT_HEADER_BYTES) as usize == SLOT_HEADER_BYTES;
    if !is_feed {
        return Ok(HEADER_BYTES as u64);
    }
    let slot = (SLOT_HEADER_BYTES as u64).saturating_add(header.u64(H_SLOT_CAPACITY));
    Ok((header.u32(H_SLOT_COUNT) as u64)
        .saturating_mul(slot)
        .saturating_add(HEADER_BYTES as u64))
}

/// The identity of the Feed at `location` now, if there is one.
pub fn current_identity(location: &Path) -> Option<(u64, u64)> {
    let locator = open_mapping(&wide(location.as_os_str())).ok()?;
    let n = with_locator(&locator, L_CURRENT, |c| c.load(Acquire)).ok()?;
    (n != 0).then_some((0, n))
}

/// A mapping made by [`Shared::map`]; unmapped when dropped. It keeps the Feed's handles open, since a named
/// mapping's name goes with its last handle, not its last view.
pub struct View {
    ptr: *mut u8,
    _held: Arc<Held>,
}

impl View {
    pub fn ptr(&self) -> *mut u8 {
        self.ptr
    }
}

impl Drop for View {
    fn drop(&mut self) {
        // SAFETY: the view made in `Shared::map`, unmapped once.
        unsafe { unmap_view(self.ptr) };
    }
}
