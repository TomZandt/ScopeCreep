#![allow(dead_code)]
use std::path::PathBuf;

/// A Feed path private to one test (never the real `/dev/shm/scopecreep-feed`). Removed when dropped.
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
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Overwrite one 8-byte header field of a Feed file, for the few tests that must forge one.
pub fn forge_header_u64(path: &std::path::Path, offset: u64, value: u64) {
    use std::os::unix::fs::FileExt;
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.write_all_at(&value.to_le_bytes(), offset).unwrap();
}

pub fn temp_feed(name: &str) -> TempFeed {
    let p = PathBuf::from(format!(
        "/dev/shm/scfeed-test-{}-{}",
        std::process::id(),
        name
    ));
    let _ = std::fs::remove_file(&p);
    TempFeed(p)
}
