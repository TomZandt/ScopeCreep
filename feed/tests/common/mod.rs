#![allow(dead_code)]
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// A Feed location private to one test (never the real Feed), removed when dropped. On Linux a file in
/// `/dev/shm`; on macOS a short shared-memory name (a name there holds 31 bytes, `.N` included); on Windows a
/// named mapping in the session's `Local\` namespace, which goes with its last handle.
pub struct TempFeed(pub PathBuf);

impl std::ops::Deref for TempFeed {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

impl AsRef<std::path::Path> for TempFeed {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempFeed {
    fn drop(&mut self) {
        scopecreep_feed::remove_feed(&self.0);
    }
}

/// Overwrite one 8-byte header field of a Feed file, for the few tests that must forge one (Linux only: the
/// Feed is a file there).
#[cfg(target_os = "linux")]
pub fn forge_header_u64(path: &std::path::Path, offset: u64, value: u64) {
    use std::os::unix::fs::FileExt;
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.write_all_at(&value.to_le_bytes(), offset).unwrap();
}

#[cfg(target_os = "linux")]
pub fn temp_feed(name: &str) -> TempFeed {
    let p = PathBuf::from(format!(
        "/dev/shm/scfeed-test-{}-{}",
        std::process::id(),
        name
    ));
    let _ = std::fs::remove_file(&p);
    TempFeed(p)
}

#[cfg(target_os = "macos")]
pub fn temp_feed(name: &str) -> TempFeed {
    let p = PathBuf::from(format!("/scf-{}-{}", std::process::id(), name));
    scopecreep_feed::remove_feed(&p);
    TempFeed(p)
}

#[cfg(windows)]
pub fn temp_feed(name: &str) -> TempFeed {
    TempFeed(PathBuf::from(format!(
        "Local\\scfeed-test-{}-{}",
        std::process::id(),
        name
    )))
}

/// A real process that runs for about 30 seconds unless killed.
pub fn long_running_process() -> Child {
    #[cfg(unix)]
    let mut command = Command::new("sleep");
    #[cfg(unix)]
    command.arg("30");
    #[cfg(windows)]
    let mut command = Command::new("ping");
    #[cfg(windows)]
    command.args(["-n", "31", "127.0.0.1"]);
    command.stdout(Stdio::null()).spawn().unwrap()
}

/// A real process that ends at once.
pub fn short_process() -> Child {
    #[cfg(unix)]
    let mut command = Command::new("true");
    #[cfg(windows)]
    let mut command = Command::new("cmd");
    #[cfg(windows)]
    command.args(["/C", "exit 0"]);
    command.spawn().unwrap()
}

/// Wait up to ten seconds for `ok`.
pub fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !ok() {
        assert!(std::time::Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
