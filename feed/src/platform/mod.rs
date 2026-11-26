//! Everything the Feed asks of the OS, behind one module (ADR 0043): where the Feed lives, how it is made and
//! mapped, the clock the protocol's `_ns` fields use, and whether a Sender's process is still running.
//!
//! Each platform is one file here with the same items. Adding a platform is adding its file and its two lines
//! below; no call site changes.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(target_os = "linux"))]
compile_error!("the Feed has no platform module for this OS yet (feed/src/platform)");
