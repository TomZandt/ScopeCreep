//! What Linux and macOS share: the clock and the process check, both plain POSIX.
use std::io;

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
