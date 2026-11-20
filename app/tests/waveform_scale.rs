//! The waveforms' scale units (#125, spec #113): each unit labels the same trace positions correctly, nits are
//! offered only for PQ and HLG outputs, and the tag names the unit. Pure: no GPU.
use scopecreep::colour_setup::{ColourSetup, KnownColourSetup, Levels, Provenance, Science};
use scopecreep::display::DisplayContext;
use scopecreep::measurement::MeasuredSpace::{Rec709, Rec2100Hlg, Rec2100Pq};
use scopecreep::settings::Settings;
use scopecreep::waveform_scale::{UNIT_SETTING, Unit, nits_at, nits_code, nits_offered};
use scopecreep::window_text::panel_title;

fn texts(
    unit: Unit,
    levels: Levels,
    space: scopecreep::measurement::MeasuredSpace,
) -> Vec<(u32, String)> {
    unit.marks(levels, space)
        .into_iter()
        .map(|m| (m.code, m.text))
        .collect()
}

#[test]
fn ten_bit_codes_are_the_default_and_unchanged() {
    assert_eq!(Unit::from_setting(None), Unit::Codes);
    assert_eq!(Unit::from_setting(Some("nonsense")), Unit::Codes);
    assert_eq!(
        texts(Unit::Codes, Levels::Video, Rec709),
        [(940, "940".into()), (512, "512".into()), (64, "64".into())]
    );
    assert_eq!(
        texts(Unit::Codes, Levels::Full, Rec709),
        [
            (768, "768".into()),
            (512, "512".into()),
            (256, "256".into())
        ]
    );
    assert_eq!(
        Unit::Codes.lines(Levels::Video, Rec709),
        [0, 256, 512, 768, 1023]
    );
    assert_eq!(Unit::Codes.above_text(Levels::Video, Rec709), "> 1023");
    assert_eq!(Unit::Codes.below_text(Levels::Video), "< 0");
    assert_eq!(Unit::Codes.tag(Rec709), None);
}

#[test]
fn percent_puts_black_at_0_and_white_at_100_of_the_levels_range() {
    // Video levels: black is code 64, white 940; the same codes the 10-bit scale names.
    assert_eq!(
        texts(Unit::Percent, Levels::Video, Rec709),
        [(940, "100".into()), (502, "50".into()), (64, "0".into())]
    );
    assert_eq!(
        Unit::Percent.lines(Levels::Video, Rec709),
        [64, 283, 502, 721, 940]
    );
    // The edge rows say what codes 1023 and 0 are in percent: headroom and footroom of video levels.
    assert_eq!(Unit::Percent.above_text(Levels::Video, Rec709), "> 109.5");
    assert_eq!(Unit::Percent.below_text(Levels::Video), "< -7.3");
    // Full levels: percent is the code over 1023, so the quarter lines are the same positions as the codes'.
    assert_eq!(
        texts(Unit::Percent, Levels::Full, Rec709),
        [(767, "75".into()), (512, "50".into()), (256, "25".into())]
    );
    assert_eq!(Unit::Percent.above_text(Levels::Full, Rec709), "> 100");
    assert_eq!(Unit::Percent.below_text(Levels::Full), "< 0");
    assert_eq!(Unit::Percent.tag(Rec709).as_deref(), Some("scale in %"));
}

#[test]
fn pq_nits_sit_where_the_st2084_eotf_puts_them() {
    // ST 2084: 100 nits is signal 0.5081, 1000 nits 0.7518, 10000 nits 1.0.
    assert_eq!(nits_code(10000, Levels::Full, Rec2100Pq), 1023);
    assert_eq!(nits_code(1000, Levels::Full, Rec2100Pq), 769);
    assert_eq!(nits_code(100, Levels::Full, Rec2100Pq), 520);
    assert_eq!(nits_code(0, Levels::Video, Rec2100Pq), 64);
    assert_eq!(nits_code(10000, Levels::Video, Rec2100Pq), 940);
    assert_eq!(nits_code(1000, Levels::Video, Rec2100Pq), 723);
    // And back: the code says the same nits.
    assert!((nits_at(723, Levels::Video, Rec2100Pq) - 1000.0).abs() < 5.0);
    assert!((nits_at(940, Levels::Video, Rec2100Pq) - 10000.0).abs() < 0.01);
    // Above white is still white's light: nothing past the top of the scale is claimed.
    assert!((nits_at(1023, Levels::Video, Rec2100Pq) - 10000.0).abs() < 0.01);
    assert_eq!(nits_at(0, Levels::Video, Rec2100Pq), 0.0);
    let marks = texts(Unit::Nits, Levels::Video, Rec2100Pq);
    assert_eq!(
        marks,
        [
            (940, "10000".into()),
            (723, "1000".into()),
            (509, "100".into()),
            (327, "10".into()),
            (64, "0".into())
        ]
    );
    assert_eq!(Unit::Nits.above_text(Levels::Video, Rec2100Pq), "> 10000");
    assert_eq!(Unit::Nits.below_text(Levels::Video), "< 0");
    assert_eq!(Unit::Nits.tag(Rec2100Pq).as_deref(), Some("scale in nits"));
}

#[test]
fn hlg_nits_are_display_light_on_a_1000_nit_display() {
    // BT.2100: a signal of 1.0 is 1000 nits on the reference display; 0.5 signal is scene 1/12, 1000 × (1/12)^1.2.
    assert_eq!(nits_code(1000, Levels::Full, Rec2100Hlg), 1023);
    assert!((nits_at(1023, Levels::Full, Rec2100Hlg) - 1000.0).abs() < 0.01);
    let half = nits_at(512, Levels::Full, Rec2100Hlg);
    assert!(
        (half - 1000.0 * (1.0f64 / 12.0).powf(1.2) * (1.0 + 0.001)).abs() < 5.0,
        "{half}"
    );
    assert_eq!(Unit::Nits.above_text(Levels::Video, Rec2100Hlg), "> 1000");
    let tag = Unit::Nits.tag(Rec2100Hlg).unwrap();
    assert!(tag.contains("HLG") && tag.contains("1000 nit"), "{tag}");
    // The lines are in order, bottom to top, so no unit scrambles the plot.
    for unit in [Unit::Codes, Unit::Percent, Unit::Nits] {
        for space in [Rec709, Rec2100Pq, Rec2100Hlg] {
            let lines = unit.lines(Levels::Video, space);
            assert!(
                lines.windows(2).all(|w| w[0] < w[1]),
                "{unit:?} {space:?} {lines:?}"
            );
            assert!(lines.len() <= 8, "the shader holds eight lines");
        }
    }
}

#[test]
fn nits_are_offered_only_for_pq_and_hlg_outputs() {
    assert!(nits_offered(Rec2100Pq, false));
    assert!(nits_offered(Rec2100Hlg, false));
    assert!(!nits_offered(Rec709, false));
    // Assumed means no known Output Colour Space: there is no light to name.
    assert!(!nits_offered(Rec2100Pq, true));
    // A remembered "nits" falls back to 10-bit codes where it is not offered, and is not tagged.
    assert_eq!(Unit::Nits.effective(Rec709, false), Unit::Codes);
    assert_eq!(Unit::Nits.effective(Rec2100Pq, false), Unit::Nits);
    assert_eq!(Unit::Percent.effective(Rec709, false), Unit::Percent);
}

fn context(output: &str, unit: &str) -> DisplayContext {
    let setup = ColourSetup::new(
        Some(Science::Yrgb),
        "DaVinci WG/Intermediate".into(),
        output.into(),
        None,
        None,
        None,
        Some(Levels::Video),
    );
    let mut settings = Settings::new();
    settings.insert(UNIT_SETTING.into(), unit.into());
    DisplayContext {
        colour_setup: Some(KnownColourSetup {
            setup,
            provenance: Provenance::Project,
        }),
        settings,
        ..DisplayContext::default()
    }
}

#[test]
fn the_tag_names_the_unit_unless_it_is_10_bit() {
    for name in [
        scopecreep::waveform::NAME,
        scopecreep::waveform::COLOUR_NAME,
    ] {
        let tags = |output: &str, unit: &str| {
            panel_title(name, &context(output, unit), String::new()).tags
        };
        assert!(
            !tags("Rec.709 Gamma 2.4", "codes")
                .iter()
                .any(|t| t.contains("scale"))
        );
        assert!(tags("Rec.709 Gamma 2.4", "percent").contains(&"scale in %".to_string()));
        assert!(tags("Rec.2100 ST2084", "nits").contains(&"scale in nits".to_string()));
        // Nits remembered, then the output changed to one without light: back to 10-bit, no tag.
        assert!(
            !tags("Rec.709 Gamma 2.4", "nits")
                .iter()
                .any(|t| t.contains("scale"))
        );
    }
}
