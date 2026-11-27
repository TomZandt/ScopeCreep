//! The app on macOS (Apple Silicon, Metal).
use crate::build_info::Build;
use muda::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};
use std::path::PathBuf;
use std::process::{Child, Command};
use winit::window::WindowAttributes;

/// What the primary shortcut modifier is called in a menu: Cmd, where Linux uses Ctrl.
pub const PRIMARY_MODIFIER: &str = "Cmd";

/// The primary modifier is the Command key (winit's "super"), and Control is left to the system.
pub const PRIMARY_IS_CMD: bool = true;

/// The folder the app remembers its settings in: `~/Library/Application Support/ScopeCreep`. None without `HOME`.
pub fn settings_folder() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|v| !v.is_empty())?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("ScopeCreep"),
    )
}

/// The Python to run the Resolve helper under. Resolve on a Mac has no Python of its own, so this is the system's
/// `python3`; finding Resolve's scripting module on a Mac is the helper's job (#256).
pub fn resolve_python() -> PathBuf {
    PathBuf::from("/usr/bin/python3")
}

/// Start the Resolve helper's process. It is not yet tied to the app's life on a Mac: macOS has no parent-death
/// signal, and the helper watching its parent comes with the Colour Setup work (#256).
pub fn spawn_tied_to_app(command: &mut Command) -> std::io::Result<Child> {
    command.spawn()
}

/// Start the system's opener on `url`.
pub fn open_url(url: &str) -> std::io::Result<Child> {
    Command::new("open").arg(url).spawn()
}

/// The window's platform attributes: none needed. winit gives the Window the screen's backing scale (2 on a
/// Retina display), so the Window's scale factor is 2 there and every size in the app follows it.
pub fn window_attributes(attributes: WindowAttributes) -> WindowAttributes {
    attributes
}

/// The graphics backend: Metal. `WGPU_BACKEND` still overrides it, for trying another.
pub const GRAPHICS_BACKENDS: wgpu::Backends = wgpu::Backends::METAL;

/// The app menu in the system menu bar, kept alive for as long as the app runs.
pub struct AppMenu {
    _menu: Menu,
}

/// Put the standard macOS app menu in the system menu bar: About ScopeCreep, Hide, Hide Others, Show All and
/// Quit ScopeCreep (Cmd+Q). The app's own menu bar stays inside the Window. Call it on the main thread once the
/// event loop has started, so it replaces the default menu winit makes.
pub fn install_app_menu() -> Option<AppMenu> {
    let menu = Menu::new();
    let app = Submenu::new("ScopeCreep", true);
    let about = AboutMetadata {
        name: Some("ScopeCreep".into()),
        version: Some(Build::current().version),
        copyright: Some("Copyright Tom Zandt-Valentine".into()),
        website: Some("https://github.com/TomZandt/ScopeCreep".into()),
        website_label: Some("ScopeCreep on GitHub".into()),
        ..Default::default()
    };
    app.append_items(&[
        &PredefinedMenuItem::about(Some("About ScopeCreep"), Some(about)),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::quit(Some("Quit ScopeCreep")),
    ])
    .ok()?;
    menu.append(&app).ok()?;
    menu.init_for_nsapp();
    Some(AppMenu { _menu: menu })
}
