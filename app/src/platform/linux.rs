//! The app on Linux.
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use winit::window::WindowAttributes;

/// What the primary shortcut modifier is called in a menu.
pub const PRIMARY_MODIFIER: &str = "Ctrl";

/// Whether the primary modifier is the Command key (a Mac) rather than Control.
pub const PRIMARY_IS_CMD: bool = false;

/// The system menu bar's app menu: none here, the app's menu bar is in its Window.
pub struct AppMenu;

/// Put the app menu in the system menu bar (a Mac has one): nothing to do here.
pub fn install_app_menu() -> Option<AppMenu> {
    None
}

/// The folder the app remembers its settings in: `$XDG_CONFIG_HOME/scopecreep`, else `~/.config/scopecreep`.
/// None with neither set.
pub fn settings_folder() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("scopecreep"))
}

/// The Python to run the Resolve helper under: Resolve's bundled one, which can import its scripting module
/// with no setup (Resolve 21.1+).
pub fn resolve_python() -> PathBuf {
    PathBuf::from("/opt/resolve/bin/ResolvePython")
}

/// Start the Resolve helper's process so it dies with the app, even when the app is killed, and holds nothing
/// the app opened.
pub fn spawn_tied_to_app(command: &mut Command) -> std::io::Result<Child> {
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        command.pre_exec(|| {
            // A helper stuck in a Resolve call must not outlive the app...
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            // ...nor hold anything the app inherited (found in testing: a lock file's descriptor). Marked
            // close-on-exec rather than closed, so the standard library's exec-error pipe still works.
            libc::close_range(3, u32::MAX, libc::CLOSE_RANGE_CLOEXEC as libc::c_int);
            Ok(())
        });
    }
    command.spawn()
}

/// Start the system's opener on `url`.
pub fn open_url(url: &str) -> std::io::Result<Child> {
    Command::new("xdg-open").arg(url).spawn()
}

/// The window's platform attributes: the Wayland app id, so the desktop can match the app launcher's entry.
pub fn window_attributes(attributes: WindowAttributes) -> WindowAttributes {
    use winit::platform::wayland::WindowAttributesExtWayland;
    attributes.with_name("scopecreep", "scopecreep")
}

/// The graphics backends wgpu may choose from: any. `WGPU_BACKEND` narrows them, for trying another.
pub const GRAPHICS_BACKENDS: wgpu::Backends = wgpu::Backends::all();
