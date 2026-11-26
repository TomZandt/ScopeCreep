//! The app's platform seam: where its settings are remembered. One test per platform, since it changes the
//! environment.
use scopecreep::platform::settings_folder;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
#[test]
fn settings_folder_follows_xdg_then_home() {
    // SAFETY: this test binary has only this test, so nothing reads the environment meanwhile.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", "/xdg");
        std::env::set_var("HOME", "/home/someone");
    }
    assert_eq!(settings_folder(), Some(PathBuf::from("/xdg/scopecreep")));

    unsafe { std::env::set_var("XDG_CONFIG_HOME", "") };
    assert_eq!(
        settings_folder(),
        Some(PathBuf::from("/home/someone/.config/scopecreep")),
        "an empty XDG_CONFIG_HOME falls back to HOME"
    );

    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("HOME");
    }
    assert_eq!(settings_folder(), None);
}

#[cfg(windows)]
#[test]
fn settings_folder_is_scopecreep_in_appdata() {
    // SAFETY: this test binary has only this test, so nothing reads the environment meanwhile.
    unsafe { std::env::set_var("APPDATA", r"C:\Users\someone\AppData\Roaming") };
    assert_eq!(
        settings_folder(),
        Some(PathBuf::from(
            r"C:\Users\someone\AppData\Roaming\ScopeCreep"
        ))
    );

    unsafe { std::env::remove_var("APPDATA") };
    assert_eq!(settings_folder(), None);
}
