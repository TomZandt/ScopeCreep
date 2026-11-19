//! The scope shader library: the phosphor response, the soft spread step and the scope colours that the waveforms
//! and the vectorscope share, so that the trace brightness and soft spread settings change all of them in one place
//! (#111). Each scope's draw shader is this library followed by its own code.

const LIBRARY: &str = include_str!("scope_shader.wgsl");

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
        for name in ["fn phosphor(", "fn spread_step(", "const PLOT:", "const GRID:", "const TRACE:"] {
            assert_eq!(shader.matches(name).count(), 1, "{name}");
        }
    }
}
