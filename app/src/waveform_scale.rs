//! The unit the waveforms' scale is written in (#125, spec #113): 10-bit codes (the default), percent of the Levels'
//! range, or nits (offered only when the Output Colour Space is PQ or HLG). Pure: no GPU, no window.
//!
//! The unit only renames positions. The counts, the plot and where a trace lands never change; the scale's
//! numbers, its reference lines and the edge rows' words are what follow. Every position is a 10-bit code
//! (0 to 1023, the plot's rows); a unit says what that code is called.
use crate::colour_setup::Levels;
use crate::measurement::MeasuredSpace;
use scopecreep_accuracy::transfer;

/// The setting's key.
pub const UNIT_SETTING: &str = "scale_units";

/// What the scale's numbers are in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Unit {
    /// 10-bit code values, 0 to 1023.
    #[default]
    Codes,
    /// Percent of the Levels' range: black 0 %, white 100 % (full levels: code 0 to 1023).
    Percent,
    /// Light, in nits, from the PQ EOTF or the HLG display light (only for PQ and HLG outputs).
    Nits,
}

/// One number on the scale: the code it sits at and what it says.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub code: u32,
    pub text: String,
    /// Lower matters more when numbers would overlap.
    pub rank: u32,
}

/// Whether nits mean something for what is measured: only the PQ and HLG Output Colour Spaces carry light, and
/// only when the space is known rather than assumed.
pub fn nits_offered(space: MeasuredSpace, assumed: bool) -> bool {
    !assumed && matches!(space, MeasuredSpace::Rec2100Pq | MeasuredSpace::Rec2100Hlg)
}

impl Unit {
    /// The unit a setting value names (anything else is the default).
    pub fn from_setting(value: Option<&str>) -> Unit {
        match value {
            Some("percent") => Unit::Percent,
            Some("nits") => Unit::Nits,
            _ => Unit::Codes,
        }
    }

    /// The unit in use: nits fall back to codes where they are not offered (the choice is kept for when they are).
    pub fn effective(self, space: MeasuredSpace, assumed: bool) -> Unit {
        if self == Unit::Nits && !nits_offered(space, assumed) { Unit::Codes } else { self }
    }

    /// What a Panel's tag says about the scale; nothing for the default 10-bit codes.
    pub fn tag(self, space: MeasuredSpace) -> Option<String> {
        match self {
            Unit::Codes => None,
            Unit::Percent => Some("scale in %".into()),
            Unit::Nits if space == MeasuredSpace::Rec2100Hlg => Some("scale in nits (HLG, 1000 nit display)".into()),
            Unit::Nits => Some("scale in nits".into()),
        }
    }

    /// The reference lines drawn across the plot, as codes.
    pub fn lines(self, levels: Levels, space: MeasuredSpace) -> Vec<u32> {
        match self {
            Unit::Codes => vec![0, 256, 512, 768, 1023],
            Unit::Percent => [0.0, 25.0, 50.0, 75.0, 100.0].iter().map(|p| percent_code(*p, levels)).collect(),
            Unit::Nits => nit_marks(space).iter().map(|n| nits_code(*n, levels, space)).collect(),
        }
    }

    /// The numbers on the scale, top to bottom, besides the edge rows.
    pub fn marks(self, levels: Levels, space: MeasuredSpace) -> Vec<Mark> {
        match self {
            Unit::Codes => levels
                .scale_marks()
                .iter()
                .enumerate()
                .map(|(i, c)| Mark { code: *c, text: c.to_string(), rank: if i == 1 { 2 } else { 0 } })
                .collect(),
            Unit::Percent => {
                let percents: [u32; 3] = if levels == Levels::Video { [100, 50, 0] } else { [75, 50, 25] };
                percents.iter().map(|p| Mark { code: percent_code(*p as f64, levels), text: p.to_string(), rank: if *p == 50 { 2 } else { 0 } }).collect()
            }
            Unit::Nits => {
                let marks = nit_marks(space);
                let last = marks.len() - 1;
                // Black and white keep their numbers longest; the ones between give way.
                marks
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(i, n)| Mark { code: nits_code(*n, levels, space), text: n.to_string(), rank: if i == 0 || i == last { 0 } else { 2 } })
                    .collect()
            }
        }
    }

    /// What the edge row above the plot says: pixels over code 1023.
    pub fn above_text(self, levels: Levels, space: MeasuredSpace) -> String {
        match self {
            Unit::Codes => "> 1023".into(),
            Unit::Percent => format!("> {}", percent_text(percent_of(1023.0, levels))),
            // Anything above code 1023 is above white, which is the top of the light scale.
            Unit::Nits => format!("> {}", nit_marks(space).last().copied().unwrap_or(0)),
        }
    }

    /// What the edge row below the plot says: pixels under code 0.
    pub fn below_text(self, levels: Levels) -> String {
        match self {
            Unit::Codes => "< 0".into(),
            Unit::Percent => format!("< {}", percent_text(percent_of(0.0, levels))),
            Unit::Nits => "< 0".into(),
        }
    }
}

/// The nit values the scale names, from black up to the top of the light scale (white).
fn nit_marks(space: MeasuredSpace) -> &'static [u32] {
    if space == MeasuredSpace::Rec2100Hlg { &[0, 10, 100, 1000] } else { &[0, 10, 100, 1000, 10000] }
}

/// Where a percent of the Levels' range lands on the 10-bit scale.
fn percent_code(percent: f64, levels: Levels) -> u32 {
    let (black, white) = levels.black_and_white();
    (black as f64 + percent / 100.0 * (white - black) as f64).round() as u32
}

/// A code as percent of the Levels' range (black 0, white 100; beyond it for headroom and footroom).
pub fn percent_of(code: f64, levels: Levels) -> f64 {
    let (black, white) = levels.black_and_white();
    (code - black as f64) / (white - black) as f64 * 100.0
}

fn percent_text(percent: f64) -> String {
    let rounded = (percent * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 { format!("{rounded:.0}") } else { format!("{rounded:.1}") }
}

/// The light, in nits, a signal (0 black to 1 white of the Levels) stands for: PQ's EOTF, or HLG's display light
/// for a neutral on a 1000 nit display (BT.2100, system gamma 1.2).
pub fn nits_of_signal(signal: f64, space: MeasuredSpace) -> f64 {
    let signal = signal.clamp(0.0, 1.0);
    if space == MeasuredSpace::Rec2100Hlg { 1000.0 * hlg_scene(signal).powf(1.2) } else { transfer::pq_decode(signal) }
}

/// The signal of `nits`: the inverse of [`nits_of_signal`].
pub fn signal_of_nits(nits: f64, space: MeasuredSpace) -> f64 {
    if space == MeasuredSpace::Rec2100Hlg { hlg_oetf((nits / 1000.0).max(0.0).powf(1.0 / 1.2)) } else { transfer::pq_encode(nits) }
}

/// The 10-bit code a nit value sits at.
pub fn nits_code(nits: u32, levels: Levels, space: MeasuredSpace) -> u32 {
    let (black, white) = levels.black_and_white();
    (black as f64 + signal_of_nits(nits as f64, space) * (white - black) as f64).round().clamp(0.0, 1023.0) as u32
}

/// The nits at a 10-bit code (a code beyond white is clamped to white's light).
pub fn nits_at(code: u32, levels: Levels, space: MeasuredSpace) -> f64 {
    nits_of_signal(percent_of(code as f64, levels) / 100.0, space)
}

const HLG_A: f64 = 0.17883277;
const HLG_B: f64 = 0.28466892;
const HLG_C: f64 = 0.55991073;

/// BT.2100 HLG OETF: scene light (0 to 1) to signal.
fn hlg_oetf(e: f64) -> f64 {
    if e <= 1.0 / 12.0 { (3.0 * e).sqrt() } else { HLG_A * (12.0 * e - HLG_B).ln() + HLG_C }
}

/// The inverse: signal to scene light.
fn hlg_scene(signal: f64) -> f64 {
    if signal <= 0.5 { signal * signal / 3.0 } else { (((signal - HLG_C) / HLG_A).exp() + HLG_B) / 12.0 }
}
