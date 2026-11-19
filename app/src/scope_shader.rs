//! The scope shader library: the phosphor response, the soft spread step and the scope colours that the waveforms
//! and the vectorscope share, so that the trace brightness and soft spread settings change all of them in one place
//! (#111). Each scope's draw shader is this library followed by its own code.

const LIBRARY: &str = include_str!("scope_shader.wgsl");

/// The key of the graticule brightness setting, the same on every trace display (#127).
pub const GRATICULE_SETTING: &str = "graticule";

/// The graticule brightness slider, the same name, range and default on every trace display: how bright the grid,
/// rings, targets and scale lines are, in percent of the normal look (the marks' contrast with the plot). Drawing
/// only: never tagged. The floor keeps the graticule faintly on screen, so the scope can always be read against it.
pub const GRATICULE_AMOUNT: crate::settings::Amount =
    crate::settings::Amount { min: 20.0, max: 300.0, step: 5.0, default: 100.0, unit: " %", decimals: 0, legacy: &[] };

/// The trace brightness slider, defined once: the same name, 20 % floor, step, default 100 % and unit everywhere; each
/// scope gives its own top of the range and the list values earlier versions saved (those differ, so a file saved
/// before the sliders still draws as it did).
pub const fn trace_brightness_amount(max: f32, legacy: &'static [(&'static str, f32)]) -> crate::settings::Amount {
    crate::settings::Amount { min: 20.0, max, step: 5.0, default: 100.0, unit: " %", decimals: 0, legacy }
}

/// The soft spread slider, the same on both waveforms and the vectorscope: 0 is no spread, 1 pixel (the default) is
/// the look before the slider. The on and off earlier versions saved map to 1 and 0.
pub const SPREAD_AMOUNT: crate::settings::Amount =
    crate::settings::Amount { min: 0.0, max: 3.0, step: 0.25, default: 1.0, unit: " px", decimals: 2, legacy: &[("on", 1.0), ("off", 0.0)] };

/// The gain the shaders multiply the graticule greys by, from the setting (100 % is 1, today's look).
pub fn graticule_gain(stored: Option<&str>) -> f32 {
    GRATICULE_AMOUNT.read(stored) / 100.0
}

/// `body` with the shared library in front of it.
pub fn with_library(body: &str) -> String {
    format!("{LIBRARY}\n{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_comes_first_and_defines_the_shared_look_once() {
        let shader = with_library("fn own() {}");
        assert!(shader.starts_with("// The scope shader library"));
        assert!(shader.ends_with("fn own() {}"));
        for name in ["fn phosphor(", "fn spread_step(", "fn graticule(", "const PLOT:", "const GRID:", "const TRACE:"] {
            assert_eq!(shader.matches(name).count(), 1, "{name}");
        }
    }
}
