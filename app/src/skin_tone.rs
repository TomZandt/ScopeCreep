//! The skin-tone line's angle, shared by every display that draws it (the vectorscopes, #61, and the Skin Tone
//! Panel, #63), so they always agree.

/// The angle of the skin-tone line in BT.709 Cb/Cr: `atan2(Cr, Cb)` in degrees, 0 at +Cb, counter-clockwise.
///
/// ASSUMED, not measured: it is the traditional I-line (123° in the composite U/V plane) rescaled to BT.709
/// Cb/Cr (docs/research/colorchecker-reference-values.md). What angle Resolve's own vectorscope draws has not
/// been measured yet; until a person checks it at Resolve, labels and docs must not say this matches Resolve.
// TODO(#60): replace with the angle measured in Resolve, and update the label wording if it now matches.
pub const SKIN_TONE_LINE_DEGREES: f64 = 134.5;
