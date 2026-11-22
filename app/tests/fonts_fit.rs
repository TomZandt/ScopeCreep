//! The text-fit estimate against the bundled interface font (#238, ADR 0045). The pure layout cannot shape text, so it
//! lays every character out at `SPACE_CHAR_WIDTH`; DM Mono is monospaced at 0.6 em, so the estimate must stay at or
//! above the real width of a character at the label and scale sizes. While it does, every least size (title bar, menu,
//! popover, Page bar, status line, Panel, Window) that was computed from the estimate is still right.

use glyphon::{Attrs, Buffer, Family, Metrics, Shaping};
use scopecreep::components::text_width;
use scopecreep::{fonts, style};

fn shaped_width(text: &str, size: f32, weight: u16) -> f32 {
    let mut fonts = fonts::font_system();
    let mut buffer = Buffer::new(&mut fonts, Metrics::new(size, size * 1.4));
    buffer.set_size(Some(10_000.0), None);
    buffer.set_text(
        text,
        &Attrs::new()
            .family(Family::Name(style::FONT_INTERFACE))
            .weight(glyphon::Weight(weight)),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut fonts, false);
    buffer.layout_runs().map(|r| r.line_w).fold(0.0, f32::max)
}

#[test]
fn the_estimate_is_never_narrower_than_the_interface_font() {
    let sample = "Waveform - 0123456789 % Fit Link Skin Tone 1920 x 1080";
    for size in [style::TYPE_LABEL, style::TYPE_SCALE] {
        for weight in [400, style::FONT_EMPHASIS_WEIGHT] {
            let real = shaped_width(sample, size, weight);
            let estimate = text_width(sample, 1.0);
            assert!(
                real <= estimate,
                "{size} px, weight {weight}: shaped {real} is wider than the estimate {estimate}"
            );
        }
    }
}

#[test]
fn every_icon_is_a_distinct_glyph_the_subset_font_has() {
    use scopecreep::components::Icon;
    let mut seen = std::collections::HashSet::new();
    let mut fonts = fonts::font_system();
    for icon in Icon::ALL {
        assert!(seen.insert(icon.glyph()), "{icon:?} shares a codepoint");
        let mut buffer = Buffer::new(&mut fonts, Metrics::new(16.0, 16.0));
        buffer.set_size(Some(100.0), None);
        buffer.set_text(
            &icon.glyph().to_string(),
            &Attrs::new().family(Family::Name(style::FONT_ICON)),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut fonts, false);
        let glyphs: Vec<_> = buffer.layout_runs().flat_map(|r| r.glyphs.iter()).collect();
        assert_eq!(glyphs.len(), 1, "{icon:?}");
        let id = glyphs[0].font_id;
        let family = &fonts.db().face(id).unwrap().families[0].0;
        assert_eq!(family, style::FONT_ICON, "{icon:?} fell back to {family}");
        assert_ne!(glyphs[0].glyph_id, 0, "{icon:?} is .notdef");
    }
}

#[test]
fn a_digit_is_the_same_width_as_a_letter() {
    let a = shaped_width("0000000000", style::TYPE_LABEL, 400);
    let b = shaped_width("iiiiiiiiii", style::TYPE_LABEL, 400);
    assert!((a - b).abs() < 0.01, "{a} vs {b}");
}
