//! The app on Windows.
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use winit::window::WindowAttributes;

/// A child started from the app must not open a console window (the app has none: `windows_subsystem`).
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The folder the app remembers its settings in: `%APPDATA%\ScopeCreep`. None without `APPDATA`.
pub fn settings_folder() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").filter(|v| !v.is_empty())?;
    Some(PathBuf::from(base).join("ScopeCreep"))
}

/// Resolve's bundled Python, which can import its scripting module with no setup (Resolve 21.1+), where the
/// Resolve installer puts it.
pub const RESOLVE_PYTHON: &str =
    r"C:\Program Files\Blackmagic Design\DaVinci Resolve\ResolvePython.exe";

/// Set up the Resolve helper's process: no console window. It holds nothing the app opened, since Windows
/// passes a child only the handles marked inheritable and the standard library marks only its pipes. The app
/// kills it when it quits.
pub fn tie_helper_to_app(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

/// Start the system's opener on `url` (the default browser for a web link). `rundll32` takes the URL as it is,
/// where `cmd /C start` would split it at every `&`.
pub fn open_url(url: &str) -> std::io::Result<Child> {
    Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
}

/// The window's platform attributes: none needed. winit makes the app per-monitor DPI aware, so the Window's
/// scale factor follows the display it is on.
pub fn window_attributes(attributes: WindowAttributes) -> WindowAttributes {
    attributes
}

/// The graphics backend: DirectX 12. `WGPU_BACKEND` still overrides it, for trying another.
pub const GRAPHICS_BACKENDS: wgpu::Backends = wgpu::Backends::DX12;
