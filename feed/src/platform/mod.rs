//! Everything the Feed asks of the OS, behind one module (ADR 0043): where the Feed lives, how it is made,
//! published and mapped (`Shared`, `View`, `current_identity`), the clock the protocol's `_ns` fields use, and
//! whether a Sender's process is still running.
//!
//! Each platform is one file here with the same items. Adding a platform is adding its file and its two lines
//! below; no call site changes.
#[cfg(unix)]
mod posix;
#[cfg(unix)]
pub use posix::{now_ns, process_alive};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
compile_error!("the Feed has no platform module for this OS yet (feed/src/platform)");

/// What the reader and writer rely on from every platform file, the same on each OS.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{
        H_HEADER_BYTES, H_MAGIC, H_SLOT_CAPACITY, H_SLOT_COUNT, H_SLOT_HEADER_BYTES, HEADER_BYTES,
        MAGIC, Mapping, SLOT_HEADER_BYTES, file_bytes,
    };
    use std::io;
    use std::path::PathBuf;

    /// A location private to one test, removed when dropped (on Windows a mapping goes with its last handle; on macOS a
    /// name holds 31 bytes, `.N` included).
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let pid = std::process::id();
            #[cfg(windows)]
            let p = PathBuf::from(format!("Local\\scfeed-unit-{pid}-{name}"));
            #[cfg(target_os = "linux")]
            let p = PathBuf::from(format!("/dev/shm/scfeed-unit-{pid}-{name}"));
            #[cfg(target_os = "macos")]
            let p = PathBuf::from(format!("/scf-u-{pid}-{name}"));
            Scratch(p)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            remove(&self.0);
        }
    }

    /// Make and publish a Feed of `slots` slots of 4096 bytes, its header written as a writer would.
    fn published(at: &Scratch, slots: usize) -> Shared {
        let made = Shared::create(&at.0, file_bytes(slots, 4096)).expect("create");
        let m = Mapping::map(&made, HEADER_BYTES).expect("map");
        m.set_u32(H_HEADER_BYTES, HEADER_BYTES as u32);
        m.set_u32(H_SLOT_HEADER_BYTES, SLOT_HEADER_BYTES as u32);
        m.set_u32(H_SLOT_COUNT, slots as u32);
        m.set_u64(H_SLOT_CAPACITY, 4096);
        m.set_u64(H_MAGIC, MAGIC);
        made.publish(&at.0).expect("publish");
        made
    }

    #[test]
    fn an_opened_feed_is_as_large_as_the_feed_that_was_published() {
        let at = Scratch::new("size");
        let _made = published(&at, 3);
        let opened = Shared::open(&at.0).expect("open");
        let made_bytes = file_bytes(3, 4096) as u64;
        // macOS rounds a shared-memory object up to a page (16 KiB on Apple Silicon); readers only need "at least".
        if cfg!(target_os = "macos") {
            assert!(opened.size() >= made_bytes && opened.size() < made_bytes + 16384);
        } else {
            assert_eq!(opened.size(), made_bytes);
        }
    }

    #[test]
    fn only_a_feed_made_here_can_be_published() {
        let at = Scratch::new("publish");
        let _made = published(&at, 1);
        let opened = Shared::open(&at.0).expect("open");
        let refused = opened
            .publish(&at.0)
            .expect_err("an opened Feed is already published");
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
    }
}
