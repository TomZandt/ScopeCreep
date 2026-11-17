//! The skin-tone line's angle, shared by every display that draws it (the vectorscopes, #61, and the Skin Tone
//! Panel, #63), so they always agree.

/// The angle of the skin-tone line in BT.709 Cb/Cr: `atan2(Cr, Cb)` in degrees, 0 at +Cb, counter-clockwise.
///
/// Measured from Resolve 21.2's own vectorscope with "Show Skin Tone Indicator" on: the line is drawn at 123.3°
/// (± 0.5°) on screen, the traditional I-line angle, so this is 123° (docs/findings/60-skin-tone-line.md, ADR 0018).
pub const SKIN_TONE_LINE_DEGREES: f64 = 123.0;
