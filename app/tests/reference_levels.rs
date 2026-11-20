//! The waveforms' reference-level lines (#128, spec #192): one Low and High pair per Panel, set in the scale unit,
//! kept as a signal level, off by default, Low never above High. Pure: no GPU.
use scopecreep::colour_setup::Levels;
use scopecreep::measurement::MeasuredSpace::{Rec709, Rec2100Hlg, Rec2100Pq};
use scopecreep::reference_levels::{
    DEFS, HIGH_KEY, LOW_KEY, Line, ReferenceLevels, amount, line_unit, popover_defs,
    slider_settings, stored_value,
};
use scopecreep::settings::{PanelSettings, Settings};
use scopecreep::waveform_scale::{Unit, nits_code};

const UNITS: [Unit; 3] = [Unit::Codes, Unit::Percent, Unit::Nits];

fn settings(pairs: &[(&str, &str)]) -> Settings {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Set `key` to `value` in `unit` on a PQ output with video levels, as a slider does.
fn set(stored: &Settings, key: &str, value: &str, unit: Unit) -> Settings {
    let mut next = stored.clone();
    next.insert(
        key.to_string(),
        stored_value(stored, key, value, unit, Levels::Video, Rec2100Pq).unwrap(),
    );
    next
}

/// What the sliders read for `stored` in `unit`.
fn shown(stored: &Settings, unit: Unit, key: &str) -> f32 {
    slider_settings(stored, unit, Levels::Video, Rec2100Pq)[key]
        .parse()
        .unwrap()
}

#[test]
fn the_lines_are_off_by_default() {
    let none = ReferenceLevels::from_settings(&Settings::new());
    for levels in [Levels::Video, Levels::Full] {
        assert_eq!(none.codes(levels), [None, None]);
    }
    let resolved = PanelSettings::default().resolve("Luma waveform", &DEFS);
    assert_eq!(ReferenceLevels::from_settings(&resolved), none);
    // Each unit's sliders show the off ends.
    for unit in UNITS {
        let shown = slider_settings(&resolved, unit, Levels::Video, Rec2100Pq);
        let (low, high) = (amount(unit, Line::Low), amount(unit, Line::High));
        assert_eq!(shown[LOW_KEY], low.min.to_string(), "{unit:?}");
        assert_eq!(shown[HIGH_KEY], high.max.to_string(), "{unit:?}");
    }
}

#[test]
fn the_ends_of_the_sliders_are_off() {
    for unit in UNITS {
        let (low, high) = (amount(unit, Line::Low), amount(unit, Line::High));
        let on = set(&Settings::new(), LOW_KEY, &low.min.to_string(), unit);
        let on = set(&on, HIGH_KEY, &high.max.to_string(), unit);
        assert_eq!(
            ReferenceLevels::from_settings(&on).codes(Levels::Video),
            [None, None],
            "{unit:?}"
        );
        assert_eq!(low.default, low.min);
        assert_eq!(high.default, high.max);
    }
}

#[test]
fn low_and_high_sit_at_the_codes_their_unit_names() {
    let at = |unit, low: &str, high: &str| {
        let stored = set(
            &set(&Settings::new(), LOW_KEY, low, unit),
            HIGH_KEY,
            high,
            unit,
        );
        ReferenceLevels::from_settings(&stored).codes(Levels::Video)
    };
    assert_eq!(at(Unit::Codes, "100", "900"), [Some(100), Some(900)]);
    // Percent of the Levels' range: black is code 64, white 940.
    assert_eq!(at(Unit::Percent, "10", "90"), [Some(152), Some(852)]);
    // Nits: the same codes the scale's numbers use.
    assert_eq!(
        at(Unit::Nits, "1", "1000"),
        [
            Some(nits_code(1, Levels::Video, Rec2100Pq)),
            Some(nits_code(1000, Levels::Video, Rec2100Pq))
        ]
    );
    // A black floor and a deliberate peak are reachable.
    assert_eq!(at(Unit::Codes, "64", "1022"), [Some(64), Some(1022)]);
    assert_eq!(
        at(Unit::Nits, "0.1", "203")[1],
        Some(nits_code(203, Levels::Video, Rec2100Pq))
    );
}

#[test]
fn a_line_keeps_its_light_when_the_unit_changes() {
    // 1000 nits set in nits: the same line in codes and percent, and 1000 nits again when read back.
    let stored = set(&Settings::new(), HIGH_KEY, "1000", Unit::Nits);
    let code = nits_code(1000, Levels::Video, Rec2100Pq);
    assert_eq!(shown(&stored, Unit::Codes, HIGH_KEY), code as f32);
    assert_eq!(shown(&stored, Unit::Nits, HIGH_KEY), 1000.0);
    let percent = (code as f32 - 64.0) / 876.0 * 100.0;
    assert!((shown(&stored, Unit::Percent, HIGH_KEY) - percent).abs() <= 0.5);
    assert_eq!(
        ReferenceLevels::from_settings(&stored).codes(Levels::Video)[1],
        Some(code)
    );
    // And the other way round: set in codes, read in nits.
    let stored = set(&Settings::new(), HIGH_KEY, &code.to_string(), Unit::Codes);
    // (a code is a few nits wide up there, so the nearest nit is not exactly 1000)
    assert!((shown(&stored, Unit::Nits, HIGH_KEY) - 1000.0).abs() < 10.0);
    // Light does not depend on the Levels: full levels put the same nits at other codes.
    let full = slider_settings(
        &set(&Settings::new(), HIGH_KEY, "1000", Unit::Nits),
        Unit::Nits,
        Levels::Full,
        Rec2100Pq,
    );
    assert_eq!(full[HIGH_KEY], "1000");
}

#[test]
fn a_line_that_is_on_never_reads_as_off() {
    // 1 nit is a real floor: in codes and percent it must not land on the "off" end.
    let stored = set(&Settings::new(), LOW_KEY, "1", Unit::Nits);
    for unit in [Unit::Codes, Unit::Percent] {
        assert!(shown(&stored, unit, LOW_KEY) > amount(unit, Line::Low).min);
    }
    assert!(ReferenceLevels::from_settings(&stored).codes(Levels::Video)[0].is_some());
}

#[test]
fn low_can_never_be_above_high() {
    // A slider stops at the other line.
    let stored = set(&Settings::new(), HIGH_KEY, "500", Unit::Codes);
    let stored = set(&stored, LOW_KEY, "900", Unit::Codes);
    assert_eq!(shown(&stored, Unit::Codes, LOW_KEY), 500.0);
    let stored = set(&stored, HIGH_KEY, "100", Unit::Codes);
    assert_eq!(shown(&stored, Unit::Codes, HIGH_KEY), 500.0);
    // A hand-edited file cannot cross them either.
    let crossed = ReferenceLevels::from_settings(&settings(&[(LOW_KEY, "90"), (HIGH_KEY, "20")]));
    let [l, h] = crossed.codes(Levels::Full);
    assert!(l.unwrap() <= h.unwrap());
}

#[test]
fn the_sliders_reach_a_floor_and_a_peak() {
    let (low, high) = (
        amount(Unit::Nits, Line::Low),
        amount(Unit::Nits, Line::High),
    );
    assert_eq!((low.min, low.max, low.step), (0.0, 100.0, 0.1));
    assert_eq!((high.min, high.max, high.step), (0.0, 10000.0, 1.0));
    for unit in [Unit::Codes, Unit::Percent] {
        let (low, high) = (amount(unit, Line::Low), amount(unit, Line::High));
        assert_eq!((low.min, low.max), (high.min, high.max), "{unit:?}");
    }
    // Low in codes reaches well past half way: a floor can sit anywhere under High.
    assert!(amount(Unit::Codes, Line::Low).max >= 1023.0);
    assert_eq!(amount(Unit::Percent, Line::Low).unit, " %");
}

#[test]
fn where_nits_are_not_offered_the_lines_read_codes_like_the_scale() {
    assert_eq!(line_unit(Unit::Nits, Rec709, false), Unit::Codes);
    assert_eq!(line_unit(Unit::Nits, Rec2100Pq, true), Unit::Codes);
    assert_eq!(line_unit(Unit::Nits, Rec2100Hlg, false), Unit::Nits);
    assert_eq!(line_unit(Unit::Percent, Rec709, false), Unit::Percent);
    // The one pair keeps its light meanwhile: 1000 nits comes back when a PQ output does.
    let stored = set(&Settings::new(), HIGH_KEY, "1000", Unit::Nits);
    let on_sdr = slider_settings(&stored, Unit::Codes, Levels::Video, Rec709);
    assert!(on_sdr[HIGH_KEY].parse::<f32>().is_ok());
    assert_eq!(shown(&stored, Unit::Nits, HIGH_KEY), 1000.0);
}

#[test]
fn the_popover_offers_one_pair_in_the_unit_in_use() {
    for unit in UNITS {
        let offered: Vec<_> = popover_defs(unit)
            .iter()
            .map(|d| (d.key, d.label, d.amount.unwrap().unit))
            .collect();
        let suffix = amount(unit, Line::Low).unit;
        assert_eq!(
            offered,
            [
                (LOW_KEY, "Low reference level", suffix),
                (HIGH_KEY, "High reference level", suffix)
            ]
        );
    }
    assert_eq!(DEFS.len(), 2);
}
