//! Everything the app asks of the OS, behind one module: the settings folder, where Resolve's Python is and how
//! the Resolve helper's process is set up, the system's link opener, the window's platform attributes, the
//! graphics backend, the name of the primary shortcut modifier and the system menu bar's app menu. The Feed's own platform calls (its location, mapping, clock and the Sender-alive check) are
//! in the Feed crate.
//!
//! Each platform is one file here with the same items. Adding a platform is adding its file and its two lines
//! below; no call site changes.
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
compile_error!("the app has no platform module for this OS yet (app/src/platform)");
