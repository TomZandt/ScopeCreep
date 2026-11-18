//! The Window's text on the pure seam (#99, spec #94 stories 23 to 26a): the Colour Setup once in the top bar, each
//! Panel's title bar with only the tags its own measurement needs (and the full label on hover), and the status
//! line with playback and performance only. No GPU.
use scopecreep::colour_setup::{ColourSetup, KnownColourSetup, Provenance};
use scopecreep::display::DisplayContext;
use scopecreep::panel_grid::{PanelGrid, PanelLayout};
use scopecreep::settings::Settings;
use scopecreep::waveform::LumaWeights;
use scopecreep::window_text::{self, PanelTitle, panel_title};
use scopecreep_feed::SentColourSetup;

const ACES_709: &str = "science=aces\ntimeline=ACEScct\noutput=Rec.709 BT.1886\nlevels=video\n";
const DWG_709: &str = "science=rcm\ntimeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\nlevels=video\n";
const UNSUPPORTED: &str = "science=rcm\ntimeline=Arri LogC\noutput=Rec.2100 HLG\nlevels=video\n";
const APPROXIMATED: &str = "science=yrgb\ntimeline=Rec.709 Gamma 2.4\noutput=Rec.709 BT.1886\nlevels=video\n";
const UNREADABLE: &str = "science=yrgb\ntimeline=Arri LogC\noutput=Rec.2100 HLG\nlevels=video\n";
const FULL_LEVELS: &str = "science=aces\ntimeline=ACEScct\noutput=Rec.709 BT.1886\nlevels=full\n";

fn known(setup: &str, provenance: Provenance) -> KnownColourSetup {
    KnownColourSetup { setup: ColourSetup::from_feed(&SentColourSetup::new(provenance, setup)).unwrap(), provenance }
}

fn context(setup: Option<&str>, settings: &[(&str, &str)]) -> DisplayContext {
    let settings: Settings = settings.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    DisplayContext { colour_setup: setup.map(|s| known(s, Provenance::Project)), settings, ..DisplayContext::default() }
}

fn title(name: &str, setup: Option<&str>, settings: &[(&str, &str)]) -> PanelTitle {
    panel_title(name, &context(setup, settings), "the full label".into())
}

const NAMES: [&str; 10] = [
    "Clean picture",
    "Luma waveform",
    "Channel view",
    "ARRI False Colour",
    "Black and white",
    "Vectorscope",
    "Saturation heatmap",
    "Skin tone",
    "Stops False Colour",
    "Colour waveform",
];

// ------------------------------------------------------------------------------------------------ top bar

#[test]
fn the_top_bar_says_the_colour_setup_once_with_levels_source_and_conversion() {
    let bar = window_text::top_bar(Some(&known(ACES_709, Provenance::Project)), "unknown");
    assert_eq!(bar, "ACEScct → Rec.709 BT.1886  ·  Video levels  ·  from project, assumes ACES 2.0  ·  Output Conversion exact for ACES 2.0");
    let bar = window_text::top_bar(Some(&known(DWG_709, Provenance::Manual)), "unknown");
    assert_eq!(bar, "DaVinci WG/Intermediate → Rec.709 Gamma 2.4  ·  Video levels  ·  set manually  ·  Output Conversion exact");
    assert!(window_text::top_bar(Some(&known(FULL_LEVELS, Provenance::Project)), "").contains("Full levels"));
}

#[test]
fn the_top_bar_never_overstates_what_is_not_proven() {
    assert_eq!(window_text::top_bar(None, "Colour Setup unknown"), "Colour Setup unknown  ·  as sent, no Output Conversion");
    let bar = window_text::top_bar(Some(&known(UNSUPPORTED, Provenance::Project)), "");
    assert!(bar.contains("as sent, no Output Conversion"), "{bar}");
    assert!(!bar.contains("exact"), "{bar}");
    let approximated = window_text::top_bar(Some(&known(APPROXIMATED, Provenance::Project)), "");
    assert!(approximated.contains("Output Conversion approximated"), "{approximated}");
}

#[test]
fn levels_not_in_the_colour_setup_are_called_assumed() {
    let bar = window_text::top_bar(Some(&known("science=aces\ntimeline=ACEScct\noutput=Rec.709 BT.1886\n", Provenance::Project)), "");
    assert!(bar.contains("Video levels assumed"), "{bar}");
}

#[test]
fn the_top_bar_drops_its_last_parts_first_and_never_cuts_a_part() {
    let line = window_text::top_bar(Some(&known(ACES_709, Provenance::Project)), "");
    let wide = window_text::fit_parts(&line, 10_000.0, 1.0);
    assert_eq!(wide, line);
    let narrow = window_text::fit_parts(&line, 400.0, 1.0);
    assert!(narrow.starts_with("ACEScct → Rec.709 BT.1886"), "{narrow}");
    assert!(line.starts_with(&narrow) && narrow.len() < line.len());
    assert_eq!(window_text::fit_parts(&line, 1.0, 1.0), "ACEScct → Rec.709 BT.1886", "the route is always kept");
}

// ------------------------------------------------------------------------------------------------ title bars

#[test]
fn on_an_exact_setup_with_defaults_a_title_bar_says_only_its_name_and_its_own_state() {
    // Nothing the top bar already says, and none of the defaults the long labels used to repeat.
    for (name, want_name, want_tags) in [
        ("Clean picture", "Clean picture", vec![]),
        ("Luma waveform", "Luma waveform", vec![]),
        ("Colour waveform", "Colour waveform", vec![]),
        ("Black and white", "Black and white", vec![]),
        ("ARRI False Colour", "ARRI False Colour", vec![]),
        ("Vectorscope", "Vectorscope", vec![]),
        ("Saturation heatmap", "Saturation heatmap", vec![]),
        ("Skin tone", "Skin tone", vec![]),
        ("Channel view", "Channel view", vec!["Blue", "200 %"]),
        ("Stops False Colour", "Stops False Colour", vec![]),
    ] {
        let t = title(name, Some(ACES_709), &[]);
        assert_eq!(t.name, want_name);
        assert_eq!(t.tags, want_tags, "{name}");
        for repeated in ["Cb/Cr", "0.2126", "Y =", "from primaries", "ACEScct", "Rec.709 BT.1886", "from project"] {
            assert!(!t.name.contains(repeated) && !t.tags_text().contains(repeated), "{name} repeats {repeated}");
        }
    }
}

#[test]
fn the_full_label_is_what_hovering_the_title_bar_shows() {
    let t = panel_title("Skin tone", &context(Some(ACES_709), &[]), "Skin tone  ·  yellow within ±5°".into());
    assert_eq!(t.full, "Skin tone  ·  yellow within ±5°");
    let hover = window_text::hover_text(&t, false, 60);
    assert_eq!(hover, "Skin tone\nyellow within ±5°");
    let cut = window_text::hover_text(&title("Vectorscope", None, &[]), true, 60);
    assert!(cut.starts_with("Vectorscope  (unconverted, Rec.709 assumed)"), "{cut}");
}

#[test]
fn a_picture_shown_as_sent_is_called_unconverted_and_its_measured_space_assumed() {
    for name in ["Clean picture", "Skin tone", "Channel view"] {
        assert!(title(name, None, &[]).tags.contains(&"unconverted".to_string()), "{name}");
        assert!(title(name, Some(UNSUPPORTED), &[]).tags.contains(&"unconverted".to_string()), "{name}");
    }
    for name in ["Luma waveform", "Colour waveform", "Black and white", "ARRI False Colour", "Vectorscope", "Saturation heatmap"] {
        let tags = title(name, None, &[]).tags;
        assert!(tags.contains(&"unconverted".to_string()) && tags.contains(&"Rec.709 assumed".to_string()), "{name}: {tags:?}");
    }
}

#[test]
fn an_approximated_output_conversion_is_said_on_every_panel_that_measures_the_converted_picture() {
    let approximated = APPROXIMATED;
    for name in ["Clean picture", "Luma waveform", "Colour waveform", "Black and white", "ARRI False Colour", "Vectorscope", "Saturation heatmap", "Skin tone", "Channel view"] {
        assert!(title(name, Some(approximated), &[]).tags.contains(&"approximated".to_string()), "{name}");
    }
}

#[test]
fn non_default_settings_that_change_what_is_measured_are_tags_and_defaults_are_not() {
    let mut c = context(Some(ACES_709), &[]);
    c.luma = LumaWeights::MatchResolve;
    for name in ["Luma waveform", "Colour waveform", "Black and white", "ARRI False Colour", "Vectorscope"] {
        assert!(panel_title(name, &c, String::new()).tags.contains(&"luma weights: match Resolve".to_string()), "{name}");
    }
    assert!(title("Luma waveform", Some(ACES_709), &[]).tags.is_empty(), "the default is not repeated");
    assert_eq!(title("Saturation heatmap", Some(ACES_709), &[("scale", "150")]).tags, ["hot 150 %"]);
    assert!(title("Saturation heatmap", Some(ACES_709), &[("scale", "75")]).tags.is_empty());
    assert_eq!(title("Skin tone", Some(ACES_709), &[("tolerance", "10")]).tags, ["±10°"]);
    assert!(title("Skin tone", Some(ACES_709), &[("tolerance", "5")]).tags.is_empty());
    assert_eq!(title("Vectorscope", Some(ACES_709), &[("targets", "100")]).tags, ["100 % targets"]);
    assert_eq!(title("Channel view", Some(ACES_709), &[("channel", "red"), ("loupe_zoom", "400")]).tags, ["Red", "400 %"]);
    assert_eq!(title("Stops False Colour", Some(ACES_709), &[("mode", "grey-plus-one")]).tags, ["grey and +1 stop"]);
}

#[test]
fn the_waveforms_say_levels_that_change_how_to_read_their_scale() {
    assert_eq!(title("Luma waveform", Some(FULL_LEVELS), &[]).tags, ["full levels"]);
    let unknown_levels = "science=aces\ntimeline=ACEScct\noutput=Rec.709 BT.1886\n";
    assert_eq!(title("Luma waveform", Some(unknown_levels), &[]).tags, ["video levels assumed"]);
}

#[test]
fn stops_false_colour_says_approximated_assumed_and_when_it_cannot_measure() {
    // Scene light decoded from the Timeline Colour Space: nothing to add.
    assert!(title("Stops False Colour", Some(ACES_709), &[]).tags.is_empty());
    // A display curve inverted: approximated.
    assert_eq!(title("Stops False Colour", Some(DWG_709), &[]).tags, Vec::<String>::new(), "DaVinci Intermediate is a log curve");
    assert!(title("Stops False Colour", Some("science=rcm\ntimeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\nlevels=video\n"), &[]).tags.contains(&"approximated".to_string()));
    // Unknown Colour Setup: the Output picture as sent, through an assumed curve.
    let t = title("Stops False Colour", None, &[]).tags;
    assert_eq!(t, ["approximated", "assumed", "unconverted"]);
    assert_eq!(title("Stops False Colour", Some(UNREADABLE), &[]).tags, ["not measured"]);
}

// A worst case for the narrowest Panels at the default layout at 1280 x 760: the name is never cut, the title
// is one line, and the tags that fit are shown most important first; the hover holds the rest.
#[test]
fn every_title_bar_fits_one_line_at_the_default_layout() {
    let area = [12.0, 12.0, 1256.0, 716.0];
    let cells = PanelGrid::default_for(&NAMES).cells(area, 12.0);
    for scale in [1.0, 2.0] {
        for (i, name) in NAMES.iter().enumerate() {
            let cell = cells[i].map(|v| v * scale);
            let bar = PanelLayout::new(cell, scale).title;
            // Usual states: everything fits beside the name.
            for setup in [Some(ACES_709), Some(DWG_709), Some(FULL_LEVELS)] {
                let t = title(name, setup, &[]);
                let (layout, shown, all) = window_text::lay_out_title(&t, bar, scale);
                assert!(!layout.name_cut, "{name} at {scale}x: the name fits");
                assert!(all && shown == t.tags_text(), "{name} at {scale}x: every tag shows ({shown:?} of {:?})", t.tags);
                let right = layout.tags.map_or(layout.name[0] + layout.name[2], |r| r[0] + r[2]);
                assert!(right <= bar[0] + bar[2] + 0.01 && layout.bar[3] == scale * 22.0, "{name}: one line, inside the bar");
            }
            // Worst case: shown as sent, with the heaviest settings. Name kept, tags shown in priority order.
            let mut c = context(None, &[("targets", "100"), ("scale", "150"), ("tolerance", "10"), ("mode", "grey-plus-one"), ("channel", "red")]);
            c.luma = LumaWeights::MatchResolve;
            let t = panel_title(name, &c, String::new());
            let (layout, shown, _) = window_text::lay_out_title(&t, bar, scale);
            assert!(!layout.name_cut, "{name} at {scale}x worst case: the name fits");
            assert!(t.tags_text().starts_with(&shown), "{name}: the tags shown are the first ones: {shown:?} of {:?}", t.tags);
        }
    }
}

// ------------------------------------------------------------------------------------------------ status line

#[test]
fn the_status_line_keeps_the_rate_and_the_drops_when_the_window_is_narrow() {
    let candidates: Vec<String> = ["50.00 fps  ·  8 dropped (6 app too slow, 2 Sender busy)  ·  Timeline 1", "50.00 fps  ·  8 dropped (6 app too slow, 2 Sender busy)", "50.00 fps  ·  8 dropped"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(window_text::fit_status(&candidates, 5_000.0, 1.0), candidates[0]);
    // Lower-priority parts go first: the timeline, then the reason for the drops.
    let w = |c: &String| window_text::text_width(c, 1.0);
    assert_eq!(window_text::fit_status(&candidates, w(&candidates[1]) + 1.0, 1.0), candidates[1], "the timeline goes first");
    assert_eq!(window_text::fit_status(&candidates, w(&candidates[1]) - 1.0, 1.0), candidates[2], "then the reason for the drops");
    // At 320 px wide (less the margins) the rate and the drops are still there, whole.
    let narrow = window_text::fit_status(&candidates, 320.0 - 24.0, 1.0);
    assert!(narrow.starts_with("50.00 fps  ·  8 dropped"), "{narrow}");
    assert!(candidates.iter().any(|c| c == narrow), "never cut mid-word");
    // Even when nothing fits, the shortest (rate and drops) is what shows.
    assert_eq!(window_text::fit_status(&candidates, 10.0, 1.0), "50.00 fps  ·  8 dropped");
}
