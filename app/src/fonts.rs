//! The bundled fonts (#238, ADR 0045): DM Mono for all text, Anton for the wordmark. Both are
//! compiled into the app, so the Window looks the same on a machine that has neither installed. Their licences (SIL
//! Open Font Licence 1.1), sources and checksums are beside the files in `app/assets/fonts/`.

use crate::style;
use glyphon::FontSystem;

pub const DM_MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/DMMono-Regular.ttf");
pub const DM_MONO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/DMMono-Medium.ttf");
pub const LUCIDE_SUBSET: &[u8] = include_bytes!("../assets/fonts/Lucide-subset.ttf");
pub const ANTON_REGULAR: &[u8] = include_bytes!("../assets/fonts/Anton-Regular.ttf");

/// The text renderer's font database: the system's fonts (only for glyph fallback) plus the four bundled files (DM Mono Regular and Medium, Anton, the Lucide icon subset).
pub fn font_system() -> FontSystem {
    let mut fonts = FontSystem::new();
    let db = fonts.db_mut();
    for bytes in [
        DM_MONO_REGULAR,
        DM_MONO_MEDIUM,
        ANTON_REGULAR,
        LUCIDE_SUBSET,
    ] {
        db.load_font_data(bytes.to_vec());
    }
    // The default family is DM Mono, so nothing draws in the system sans. A glyph DM Mono lacks falls back through the
    // system's monospace, which is left as it is.
    db.set_sans_serif_family(style::FONT_INTERFACE);
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_family(fonts: &FontSystem, name: &str) -> bool {
        fonts
            .db()
            .faces()
            .any(|f| f.families.iter().any(|(n, _)| n == name))
    }

    #[test]
    fn both_families_are_in_the_database_whatever_the_machine_has() {
        let fonts = font_system();
        assert!(has_family(&fonts, style::FONT_INTERFACE));
        assert!(has_family(&fonts, style::FONT_WORDMARK));
        assert!(has_family(&fonts, style::FONT_ICON));
    }

    #[test]
    fn the_number_font_has_a_regular_and_a_medium_weight() {
        let fonts = font_system();
        let weights: Vec<u16> = fonts
            .db()
            .faces()
            .filter(|f| f.families.iter().any(|(n, _)| n == style::FONT_INTERFACE))
            .map(|f| f.weight.0)
            .collect();
        assert!(
            weights.contains(&400) && weights.contains(&500),
            "{weights:?}"
        );
    }

    #[test]
    fn the_files_are_real_truetype_fonts() {
        for bytes in [
            DM_MONO_REGULAR,
            DM_MONO_MEDIUM,
            ANTON_REGULAR,
            LUCIDE_SUBSET,
        ] {
            assert_eq!(&bytes[..4], &[0, 1, 0, 0], "not a TrueType file");
        }
    }
}
