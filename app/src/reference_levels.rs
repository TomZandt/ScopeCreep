//! The waveforms' reference-level lines (#128, spec #192): two quiet lines across the plot, Low and High, that mark
//! a floor and a peak (a delivery's minimum black and maximum light). Each is an amount set with a slider in the
//! Panel's scale unit. Pure: no GPU, no window.
//!
//! The lines are a drawing aid: they never change a count or the trace, and they are not in a tag or label.
//!
//! One pair per Panel, kept as a signal level: percent of the Levels' range. The slider shows it in the scale unit
//! (codes, percent or nits) and converts what it is set to back, so a 1000 nit line stays on 1000 nits whichever
//! unit reads it, moves when the unit changes, and is still there when a PQ or HLG output returns after an
//! output without light (where the Panel reads codes, like the scale, ADR 0033). A line is off at its slider's end:
//! Low at the bottom, High at the top. Low can never be above High (the gamut/legal warning's rule).
use crate::colour_setup::Levels;
use crate::measurement::MeasuredSpace;
use crate::settings::{Amount, SettingDef, Settings};
use crate::waveform_scale::{Unit, nits_of_signal, signal_of_nits};

/// The keys of the stored Low and High (each a percent of the Levels' range).
pub const LOW_KEY: &str = "reference_low";
pub const HIGH_KEY: &str = "reference_high";

const LOW_LABEL: &str = "Low reference level";
const HIGH_LABEL: &str = "High reference level";

/// A stored Low that is off: below the lowest code on the plot (video levels' footroom ends at -7.3 %).
const LOW_OFF: f32 = -10.0;
/// A stored High that is off: above the highest code on the plot (video levels' headroom ends at 109.5 %).
const HIGH_OFF: f32 = 110.0;

/// Which line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Line {
    Low,
    High,
}

const fn slider(
    min: f32,
    max: f32,
    step: f32,
    unit: &'static str,
    decimals: usize,
    line: Line,
) -> Amount {
    Amount {
        min,
        max,
        step,
        default: match line {
            Line::Low => min,
            Line::High => max,
        },
        unit,
        decimals,
        legacy: &[],
    }
}

/// The slider of `line` in `unit`: both reach the whole plot, so Low can mark a real floor (down to a tenth of a
/// nit) and High a deliberate peak (203 or 1000 nits); the end of each is "off". Codes 0 to 1023, percent -10 to
/// 110 (past the footroom and headroom), nits 0 to 100 for Low (steps of 0.1) and 0 to 10000 for High (steps of 1).
pub fn amount(unit: Unit, line: Line) -> Amount {
    match (unit, line) {
        (Unit::Codes, _) => slider(0.0, 1023.0, 1.0, " codes", 0, line),
        (Unit::Percent, _) => slider(LOW_OFF, HIGH_OFF, 1.0, " %", 0, line),
        (Unit::Nits, Line::Low) => slider(0.0, 100.0, 0.1, " nits", 1, line),
        (Unit::Nits, Line::High) => slider(0.0, 10000.0, 1.0, " nits", 0, line),
    }
}

const fn def(key: &'static str, label: &'static str, line: Line) -> SettingDef {
    SettingDef {
        key,
        label,
        amount: Some(slider(LOW_OFF, HIGH_OFF, 0.001, " %", 1, line)),
        choices: &[],
    }
}

/// The pair as a Panel keeps it (a percent of the Levels' range, off at the ends).
pub const DEFS: [SettingDef; 2] = [
    def(LOW_KEY, LOW_LABEL, Line::Low),
    def(HIGH_KEY, HIGH_LABEL, Line::High),
];

/// The Low and High the popover offers for `unit`: the stored pair's keys, with the unit's sliders.
pub fn popover_defs(unit: Unit) -> [SettingDef; 2] {
    let with = |def: SettingDef, line| SettingDef {
        amount: Some(amount(unit, line)),
        ..def
    };
    [with(DEFS[0], Line::Low), with(DEFS[1], Line::High)]
}

/// The unit the lines are read in: the scale's unit, which is codes where nits are not offered (ADR 0033), so the
/// sliders and the scale always agree.
pub fn line_unit(chosen: Unit, space: MeasuredSpace, assumed: bool) -> Unit {
    chosen.effective(space, assumed)
}

fn percent_to_value(unit: Unit, percent: f32, levels: Levels, space: MeasuredSpace) -> f32 {
    let (black, white) = levels.black_and_white();
    match unit {
        Unit::Codes => {
            (black as f64 + percent as f64 / 100.0 * (white - black) as f64).round() as f32
        }
        Unit::Percent => percent,
        Unit::Nits => nits_of_signal(percent as f64 / 100.0, space) as f32,
    }
}

fn value_to_percent(unit: Unit, value: f32, levels: Levels, space: MeasuredSpace) -> f32 {
    let (black, white) = levels.black_and_white();
    match unit {
        Unit::Codes => ((value - black as f32) as f64 / (white - black) as f64 * 100.0) as f32,
        Unit::Percent => value,
        Unit::Nits => (signal_of_nits(value as f64, space) * 100.0) as f32,
    }
}

/// What the slider of `line` in `unit` reads for a stored `percent`: the end for an off line, else the nearest
/// step inside the ends (a line that is on is never read as off).
fn slider_value(unit: Unit, line: Line, percent: f32, levels: Levels, space: MeasuredSpace) -> f32 {
    let range = amount(unit, line);
    let (off, inner) = match line {
        Line::Low => (LOW_OFF, range.min + range.step),
        Line::High => (HIGH_OFF, range.max - range.step),
    };
    if percent == off {
        return if line == Line::Low {
            range.min
        } else {
            range.max
        };
    }
    let value = range.at_fraction(range.fraction(percent_to_value(unit, percent, levels, space)));
    let at_end = match line {
        Line::Low => value <= range.min,
        Line::High => value >= range.max,
    };
    if at_end { inner } else { value }
}

/// The two keys' values as the popover's sliders read them in `unit`; the rest of `stored` as it is.
pub fn slider_settings(
    stored: &Settings,
    unit: Unit,
    levels: Levels,
    space: MeasuredSpace,
) -> Settings {
    let mut shown = stored.clone();
    let lines = ReferenceLevels::from_settings(stored);
    for (key, line, percent) in [
        (LOW_KEY, Line::Low, lines.low),
        (HIGH_KEY, Line::High, lines.high),
    ] {
        shown.insert(
            key.to_string(),
            Amount::store(slider_value(unit, line, percent, levels, space)),
        );
    }
    shown
}

/// What to keep for a slider of `key` set to `value` in `unit` (`None` for any other key): the end is off, any
/// other value is its signal level; Low stops at High and High at Low.
pub fn stored_value(
    stored: &Settings,
    key: &str,
    value: &str,
    unit: Unit,
    levels: Levels,
    space: MeasuredSpace,
) -> Option<String> {
    let line = match key {
        LOW_KEY => Line::Low,
        HIGH_KEY => Line::High,
        _ => return None,
    };
    let range = amount(unit, line);
    let value = range.read(Some(value));
    let lines = ReferenceLevels::from_settings(stored);
    let percent = match line {
        Line::Low if value <= range.min => LOW_OFF,
        Line::High if value >= range.max => HIGH_OFF,
        Line::Low => value_to_percent(unit, value, levels, space).min(lines.high),
        Line::High => value_to_percent(unit, value, levels, space).max(lines.low),
    };
    Some(Amount::store(percent))
}

/// The Low and High a Panel keeps, as signal levels (a percent of the Levels' range).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceLevels {
    low: f32,
    high: f32,
}

impl Default for ReferenceLevels {
    fn default() -> ReferenceLevels {
        ReferenceLevels::from_settings(&Settings::new())
    }
}

impl ReferenceLevels {
    /// Low never above High, whatever a hand-edited file says.
    pub fn from_settings(settings: &Settings) -> ReferenceLevels {
        let read = |def: &SettingDef| {
            def.amount
                .unwrap()
                .read(settings.get(def.key).map(String::as_str))
        };
        let (low, high) = (read(&DEFS[0]), read(&DEFS[1]));
        ReferenceLevels {
            low: low.min(high),
            high,
        }
    }

    /// The 10-bit codes of Low and High, `None` for a line that is off or would land on the plot's edge rows or
    /// off it.
    pub fn codes(&self, levels: Levels) -> [Option<u32>; 2] {
        let (black, white) = levels.black_and_white();
        let code = |percent: f32| {
            (black as f64 + percent as f64 / 100.0 * (white - black) as f64).round() as i64
        };
        let on_plot = |c: i64| (1..1023).contains(&c).then_some(c as u32);
        [
            (self.low > LOW_OFF)
                .then(|| on_plot(code(self.low)))
                .flatten(),
            (self.high < HIGH_OFF)
                .then(|| on_plot(code(self.high)))
                .flatten(),
        ]
    }
}
