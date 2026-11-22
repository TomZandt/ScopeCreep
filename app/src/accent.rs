//! The app accent colour (spec #196, #226, ADR 0044): the one colour in the Window's chrome, small and away from the
//! picture. It marks state only (a chip that is on, the page tab, the focused Panel's title bar line, the slider's
//! filled track), as thin lines and text, never a fill and never on measurement data. The default is #d2ff00; the
//! user picks another in Settings > Appearance, and it is remembered in its own file under `~/.config/scopecreep`.
//!
//! Everything here is pure: parsing and checking the hex, the colours derived from it, the contrast the Window
//! tells the user about, the swatches, and the text the file holds. Reading is unchecked-input safe: a file that
//! is too long, of another version, or whose value is not a hex colour reads as the default.
//!
//! The current accent is held per thread (the Window draws on its one thread; a test that sets an accent cannot
//! disturb another test running beside it). Chrome draws through [`current`], never a literal.
use crate::style::{self, Rgb};
use std::cell::Cell;

/// The default accent, as the user sees it.
pub const DEFAULT_HEX: &str = "#d2ff00";
/// The default accent as red, green and blue.
pub const DEFAULT: [u8; 3] = [0xd2, 0xff, 0x00];

/// A contrast ratio under this is flagged to the user (WCAG's AA for normal text).
pub const MIN_CONTRAST: f32 = 4.5;

/// The first line of the accent file; a file that does not start with it is ignored.
const HEADER: &str = "scopecreep-accent 1";
/// The most the accent file is read from: it holds two short lines.
pub const FILE_LIMIT: u64 = 1024;

/// The swatches Settings > Appearance offers: a name and a hex. The first is the default.
pub const SWATCHES: [(&str, &str); 6] = [
    ("Lime", DEFAULT_HEX),
    ("Amber", "#ffb000"),
    ("Orange", "#ff7a1a"),
    ("Cyan", "#2fd8ff"),
    ("Magenta", "#ff4fd8"),
    ("White", "#f0f0f0"),
];

/// An accent colour: red, green and blue, 0 to 255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent(pub [u8; 3]);

impl Default for Accent {
    fn default() -> Accent {
        Accent(DEFAULT)
    }
}

/// The colour `text` names: `#rrggbb` or `rrggbb`, case-blind, nothing else (no names, no short form, no spaces
/// inside, nothing after).
pub fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let t = text.trim();
    let digits = t.strip_prefix('#').unwrap_or(t);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let part = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some([part(0)?, part(2)?, part(4)?])
}

fn linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance of a colour in 0 to 1 (WCAG).
pub fn luminance(c: Rgb) -> f32 {
    let [r, g, b] = c.map(linear);
    r.mul_add(0.2126, g.mul_add(0.7152, b * 0.0722))
}

/// WCAG contrast ratio of two colours, 1 to 21.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

impl Accent {
    /// As a hex the user can read and type: `#d2ff00`.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2])
    }

    /// For rectangles: red, green and blue in 0 to 1.
    pub fn rgb(self) -> Rgb {
        style::rgb(self.0)
    }

    /// For text: red, green and blue in 0 to 255.
    pub fn text(self) -> [u8; 3] {
        self.0
    }

    /// The accent under a press: a darker one, derived (hover is never the accent, so there is no hover colour).
    pub fn pressed(self) -> Rgb {
        self.rgb().map(|v| v * style::ACCENT_PRESSED_SHARE)
    }

    /// Text on an accent fill (the Window has none by default; kept for a swatch's label): near black on a light
    /// accent, near white on a dark one, whichever contrasts more.
    pub fn on(self) -> [u8; 3] {
        let dark = style::TEXT_ON_ACCENT;
        let light = style::TEXT_HIGH;
        if contrast(self.rgb(), style::rgb(dark)) >= contrast(self.rgb(), style::rgb(light)) {
            dark
        } else {
            light
        }
    }

    /// The contrast of the accent as text on the lightest surface it sits on (an on chip).
    pub fn contrast(self) -> f32 {
        contrast(self.rgb(), style::SURFACE_CONTROL_ON)
    }

    /// Whether the accent reads poorly as text on a chip, which the Appearance panel says.
    pub fn low_contrast(self) -> bool {
        self.contrast() < MIN_CONTRAST
    }
}

/// The accent file's text for `accent`.
pub fn to_text(accent: Accent) -> String {
    format!("{HEADER}\naccent {}\n", accent.hex())
}

/// The accent `text` holds; anything unreadable (another version, a missing or bad value, a value that is not
/// exactly `accent #rrggbb`) is the default.
pub fn from_text(text: &str) -> Accent {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(HEADER) {
        return Accent::default();
    }
    for line in lines {
        let mut parts = line.split_whitespace();
        if let (Some("accent"), Some(value), None) = (parts.next(), parts.next(), parts.next())
            && let Some(rgb) = parse_hex(value)
        {
            return Accent(rgb);
        }
    }
    Accent::default()
}

/// Read the accent from `path`: a missing, oversized or unreadable file is the default.
pub fn load(path: &std::path::Path) -> Accent {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(path) else {
        return Accent::default();
    };
    let mut text = String::new();
    match file.take(FILE_LIMIT + 1).read_to_string(&mut text) {
        Ok(n) if n as u64 <= FILE_LIMIT => from_text(&text),
        _ => Accent::default(),
    }
}

/// Remember `accent` at `path` (written whole or not at all, like the other settings files).
pub fn save(path: &std::path::Path, accent: Accent) -> std::io::Result<()> {
    crate::settings::save(path, &to_text(accent))
}

thread_local! {
    static CURRENT: Cell<Accent> = const { Cell::new(Accent(DEFAULT)) };
}

/// The accent the chrome draws with now (this thread's).
pub fn current() -> Accent {
    CURRENT.with(Cell::get)
}

/// Make `accent` the one the chrome draws with.
pub fn set_current(accent: Accent) {
    CURRENT.with(|c| c.set(accent));
}
