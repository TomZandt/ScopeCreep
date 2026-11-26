//! Everything the app asks of the OS, behind one module: the settings folder, where Resolve's Python is and how
//! the Resolve helper's process is set up, the system's link opener, the window's platform attributes and the
//! graphics backend. The Feed's own platform calls (its location, mapping, clock and the Sender-alive check) are
//! in the Feed crate.
//!
//! Each platform is one file here with the same items. Adding a platform is adding its file and its two lines
//! below; no call site changes.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("the app has no platform module for this OS yet (app/src/platform)");
