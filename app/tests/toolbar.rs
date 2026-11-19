//! The Panel toolbar on the pure seam (#116, spec #113 stories 13 to 15, 20, 22 to 25): chips on the title bar,
//! the popover for the rest, shrinking on a small Panel, and the tags. No GPU.
use scopecreep::channel_view::{SETTINGS, TOOLBAR};
use scopecreep::components::{self, Tooltip};
use scopecreep::display::DisplayContext;
use scopecreep::panel_grid::PanelLayout;
use scopecreep::settings::{Menu, MenuRow, PanelSettings, Settings};
use scopecreep::toolbar::{Toolbar, popover_defs};
use scopecreep::window_text::{self, panel_title};

const NAME: &str = "Channel view";

fn current(channel: Option<&str>) -> Settings {
    let mut s = PanelSettings::default();
    if let Some(c) = channel {
        s.set(NAME, "channel", c);
    }
    s.resolve(NAME, &SETTINGS)
}

fn on(toolbar: &Toolbar) -> Vec<&str> {
    toolbar.chips.iter().zip(&toolbar.on).filter(|(_, on)| **on).map(|(c, _)| c.text.as_str()).collect()
}

#[test]
fn the_channel_view_has_y_r_g_b_chips_with_blue_on_by_default() {
    let toolbar = Toolbar::new(&TOOLBAR, &current(None));
    assert_eq!(toolbar.chips.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["Y", "R", "G", "B"]);
    assert_eq!(on(&toolbar), ["B"]);
}

#[test]
fn a_click_switches_the_channel_one_at_a_time_and_y_is_luma() {
    let mut settings = PanelSettings::default();
    for (index, (value, chip)) in [("luma", "Y"), ("red", "R"), ("green", "G"), ("blue", "B")].into_iter().enumerate() {
        let toolbar = Toolbar::new(&TOOLBAR, &settings.resolve(NAME, &SETTINGS));
        let (key, picked) = toolbar.click(index).expect("a chip picks a choice");
        assert_eq!((key, picked), ("channel", value));
        settings.set(NAME, key, picked);
        let after = Toolbar::new(&TOOLBAR, &settings.resolve(NAME, &SETTINGS));
        assert_eq!(on(&after), [chip], "exactly one chip is on");
    }
}

#[test]
fn the_choice_is_remembered_with_the_other_settings() {
    let mut settings = PanelSettings::default();
    settings.set(NAME, "channel", "luma");
    let back = PanelSettings::from_text(&settings.to_text());
    assert_eq!(on(&Toolbar::new(&TOOLBAR, &back.resolve(NAME, &SETTINGS))), ["Y"]);
}

#[test]
fn the_chips_are_coloured_when_on_and_grey_when_off_and_hovering_names_them() {
    let toolbar = Toolbar::new(&TOOLBAR, &current(Some("red")));
    let drawn = |i: usize| toolbar.chips[i].draw([0.0, 0.0], toolbar.on[i], components::State::Normal, 1.0);
    let fill = |i: usize| match &drawn(i)[0] {
        components::Prim::Rect { colour, .. } => *colour,
        other => panic!("{other:?}"),
    };
    assert_eq!(fill(1), scopecreep::style::rgba(scopecreep::style::CHANNEL_R), "R is on, in red");
    assert_eq!(fill(2), scopecreep::style::rgba(scopecreep::style::SURFACE_CONTROL), "G is off, grey");
    assert_eq!(toolbar.tooltips, ["Luma (Y)", "Red", "Green", "Blue"]);
    assert_eq!(Tooltip::new(&toolbar.tooltips[0]).lines(), ["Luma (Y)"]);
}

#[test]
fn the_popover_holds_the_settings_the_toolbar_does_not_show() {
    let left = popover_defs(&SETTINGS, &TOOLBAR);
    assert_eq!(left.iter().map(|d| d.key).collect::<Vec<_>>(), ["size"], "the channel is chips now, the Loupe size stays in the popover");
    let menu = Menu::new(0, &left, &current(None)).unwrap();
    assert!(menu.rows.iter().all(|r| !matches!(r, MenuRow::Choice { key: "channel", .. })));
    // A display whose settings are all chips has no popover.
    assert!(Menu::new(0, &popover_defs(&SETTINGS[..1], &TOOLBAR), &current(None)).is_none());
}

#[test]
fn the_popover_opened_from_the_settings_button_stays_inside_the_window() {
    let menu = Menu::new(0, &popover_defs(&SETTINGS, &TOOLBAR), &current(None)).unwrap();
    for scale in [1.0, 2.0] {
        let window = [1280.0 * scale, 760.0 * scale];
        // A Panel at the window's right edge, and at its bottom right.
        for cell in [[900.0 * scale, 20.0 * scale, 360.0 * scale, 300.0 * scale], [900.0 * scale, 440.0 * scale, 360.0 * scale, 300.0 * scale]] {
            let bar = PanelLayout::new(cell, scale).title;
            let title = panel_title(NAME, &DisplayContext::default(), String::new());
            let toolbar = Toolbar::new(&TOOLBAR, &current(None));
            let (layout, _, _) = window_text::lay_out_toolbar_title(&title, &toolbar.chips, true, bar, scale);
            let button = layout.settings.expect("the settings button is on the bar");
            let rect = menu.rect([button[0], button[1] + button[3]], window, scale);
            assert!(rect[0] >= 0.0 && rect[1] >= 0.0 && rect[0] + rect[2] <= window[0] && rect[1] + rect[3] <= window[1], "{rect:?} in {window:?}");
        }
    }
}

#[test]
fn the_toolbar_shrinks_on_a_small_panel_chips_stay_name_truncates_tags_go_to_the_hover() {
    let title = panel_title(NAME, &DisplayContext::default(), "the full label".into());
    assert!(!title.tags.is_empty());
    let toolbar = Toolbar::new(&TOOLBAR, &current(None));
    for scale in [1.0, 2.0] {
        // Wide: everything fits.
        let wide = PanelLayout::new([0.0, 0.0, 600.0 * scale, 300.0 * scale], scale).title;
        let (l, tags, all) = window_text::lay_out_toolbar_title(&title, &toolbar.chips, true, wide, scale);
        assert!(!l.name_cut && all && !l.tags_in_tooltip && l.tags.is_some(), "wide: {l:?}");
        assert_eq!(l.chips.len(), 4);
        assert_eq!(tags, title.tags_text());
        // Small: the chips and the button stay whole inside the bar, the name is cut, the tags are in the hover.
        let small = PanelLayout::new([0.0, 0.0, 130.0 * scale, 200.0 * scale], scale).title;
        let (l, tags, all) = window_text::lay_out_toolbar_title(&title, &toolbar.chips, true, small, scale);
        assert!(l.name_cut, "the name truncates");
        assert!(!all && tags.is_empty() && l.tags.is_none(), "the tags move to the hover");
        for chip in &l.chips {
            assert!(chip[0] >= small[0] && chip[0] + chip[2] <= small[0] + small[2] + 0.01, "{chip:?} stays inside {small:?}");
        }
        let button = l.settings.unwrap();
        assert!(button[0] + button[2] <= small[0] + small[2] + 0.01);
        assert!(l.name[0] + l.name[2] <= l.chips[0][0], "the name stops before the first chip");
        // The hover then carries the name and its tags.
        let hover = window_text::hover_text(&title, true, 60);
        assert!(hover.starts_with(&format!("{NAME}  (")), "{hover}");
    }
}

#[test]
fn the_channel_shows_in_the_tags_and_luma_says_so() {
    let tags = |channel: Option<&str>| {
        let settings: Settings = channel.map(|c| [("channel".to_string(), c.to_string())].into()).unwrap_or_default();
        panel_title(NAME, &DisplayContext { settings, ..DisplayContext::default() }, String::new()).tags
    };
    assert_eq!(tags(Some("luma"))[0], "Luma");
    assert_eq!(tags(Some("red"))[0], "Red");
    assert_eq!(tags(None)[0], "Blue");
}

// ------------------------------------------------------------------------------------------------ B/W (#118)

use scopecreep::{arri_false_colour, picture_view, skin_tone_view, stops_false_colour};

/// Whether a Panel's own settings say its unpainted picture is grey, and whether its toolbar has a B/W chip on.
fn bw_state(name: &str, defs: &[scopecreep::settings::SettingDef], toolbar: &[scopecreep::toolbar::ToolbarDef], set: &[(&str, &str)]) -> (Vec<String>, Vec<bool>) {
    let mut s = PanelSettings::default();
    for (k, v) in set {
        s.set(name, k, v);
    }
    let t = Toolbar::new(toolbar, &s.resolve(name, defs));
    (t.chips.iter().map(|c| c.text.clone()).collect(), t.on)
}

#[test]
fn the_clean_picture_and_skin_tone_start_in_colour_and_arri_and_stops_start_grey() {
    let (chips, on) = bw_state("Clean picture", &picture_view::SETTINGS, &picture_view::TOOLBAR, &[]);
    assert_eq!((chips, on), (vec!["B/W".to_string()], vec![false]));
    assert_eq!(bw_state("Skin tone", &skin_tone_view::SETTINGS, &skin_tone_view::TOOLBAR, &[]).1, [false]);
    assert_eq!(bw_state("ARRI False Colour", &arri_false_colour::SETTINGS, &arri_false_colour::TOOLBAR, &[]).1, [true]);
    // Stops' toolbar is the mode chips then B/W: Zones (default), Grey +1, B/W.
    assert_eq!(bw_state("Stops False Colour", &stops_false_colour::SETTINGS, &stops_false_colour::TOOLBAR, &[]).1, [true, false, true]);
}

#[test]
fn a_click_flips_the_toggle_each_way_and_it_is_remembered_per_panel() {
    let mut s = PanelSettings::default();
    let name = "Clean picture";
    let click = |s: &mut PanelSettings| {
        let t = Toolbar::new(&picture_view::TOOLBAR, &s.resolve(name, &picture_view::SETTINGS));
        let (k, v) = t.click(0).unwrap();
        s.set(name, k, v);
    };
    click(&mut s);
    let back = PanelSettings::from_text(&s.to_text());
    assert_eq!(Toolbar::new(&picture_view::TOOLBAR, &back.resolve(name, &picture_view::SETTINGS)).on, [true]);
    assert_eq!(Toolbar::new(&skin_tone_view::TOOLBAR, &back.resolve("Skin tone", &skin_tone_view::SETTINGS)).on, [false], "another Panel is unaffected");
    click(&mut s);
    assert_eq!(Toolbar::new(&picture_view::TOOLBAR, &s.resolve(name, &picture_view::SETTINGS)).on, [false], "a second click turns it off");
}

#[test]
fn stops_mode_is_two_chips_one_at_a_time_remembered_and_b_w_follows_grey_and_plus_one() {
    let name = "Stops False Colour";
    let toolbar = |s: &PanelSettings| {
        let own = s.resolve(name, &stops_false_colour::SETTINGS);
        Toolbar::new(stops_false_colour::toolbar_for(&own), &own)
    };
    let mut s = PanelSettings::default();
    let t = toolbar(&s);
    assert_eq!(t.chips.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["Zones", "Grey +1"], "full zones is the default; B/W is hidden");
    assert_eq!(t.on, [true, false]);
    let (k, v) = t.click(1).unwrap();
    s.set(name, k, v);
    let t = toolbar(&PanelSettings::from_text(&s.to_text()));
    assert_eq!(t.chips.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["Zones", "Grey +1", "B/W"]);
    assert_eq!(t.on, [false, true, true], "remembered, and B/W shows");
    assert_eq!(t.click(1).unwrap(), ("mode", "grey-plus-one"), "clicking the chip already on keeps it on");
    let (k, v) = t.click(0).unwrap();
    s.set(name, k, v);
    assert_eq!(toolbar(&s).on, [true, false]);
}

#[test]
fn stops_shows_bw_only_in_grey_and_plus_one_and_the_heatmap_has_none() {
    let zones: Settings = [("mode".to_string(), "zones".to_string())].into();
    let grey: Settings = [("mode".to_string(), "grey-plus-one".to_string())].into();
    assert_eq!(Toolbar::new(stops_false_colour::toolbar_for(&zones), &zones).chips.len(), 2);
    assert_eq!(Toolbar::new(stops_false_colour::toolbar_for(&grey), &grey).chips.len(), 3);
    assert_eq!(Toolbar::new(stops_false_colour::toolbar_for(&Settings::default()), &Settings::default()).chips.len(), 2, "the default mode is full zones");
}

#[test]
fn bw_is_a_chip_not_a_popover_row() {
    let left = popover_defs(&picture_view::SETTINGS, &picture_view::TOOLBAR);
    assert!(left.is_empty(), "the clean picture has no popover");
    assert_eq!(popover_defs(&stops_false_colour::SETTINGS, &stops_false_colour::TOOLBAR).iter().map(|d| d.key).collect::<Vec<_>>(), ["legend"]);
}

#[test]
fn the_tags_say_b_w_or_colour_when_the_toggle_is_not_at_its_default() {
    let tags = |name: &str, v: &str| {
        let settings: Settings = [("bw".to_string(), v.to_string()), ("mode".to_string(), "grey-plus-one".to_string())].into();
        panel_title(name, &DisplayContext { settings, ..DisplayContext::default() }, String::new()).tags
    };
    assert!(tags("Clean picture", "on").contains(&"B/W".to_string()));
    assert!(!tags("Clean picture", "off").contains(&"B/W".to_string()));
    assert!(tags("Skin tone", "on").contains(&"B/W".to_string()));
    assert!(tags("ARRI False Colour", "off").contains(&"colour".to_string()));
    assert!(!tags("ARRI False Colour", "on").iter().any(|t| t == "B/W" || t == "colour"));
    assert!(tags("Stops False Colour", "off").contains(&"colour".to_string()));
}

// ------------------------------------------------------------------ the luma waveform's chips (#117)

mod waveform_chips {
    use scopecreep::settings::PanelSettings;
    use scopecreep::toolbar::{Toolbar, popover_defs};
    use scopecreep::waveform::{CHANNELS_SETTING, NAME, SETTINGS, TOOLBAR};

    fn on(toolbar: &Toolbar) -> String {
        toolbar.chips.iter().zip(&toolbar.on).filter(|(_, on)| **on).map(|(c, _)| c.text.as_str()).collect()
    }

    fn click(settings: &mut PanelSettings, chip: usize) -> String {
        let toolbar = Toolbar::new(&TOOLBAR, &settings.resolve(NAME, &SETTINGS));
        let (key, value) = toolbar.click(chip).expect("a chip sets the setting");
        assert_eq!(key, CHANNELS_SETTING);
        settings.set(NAME, key, value);
        on(&Toolbar::new(&TOOLBAR, &settings.resolve(NAME, &SETTINGS)))
    }

    #[test]
    fn luma_is_on_by_default_and_the_chips_combine() {
        let mut settings = PanelSettings::default();
        assert_eq!(on(&Toolbar::new(&TOOLBAR, &settings.resolve(NAME, &SETTINGS))), "Y");
        assert_eq!(click(&mut settings, 1), "YR");
        assert_eq!(click(&mut settings, 3), "YRB");
        assert_eq!(click(&mut settings, 0), "RB", "Y off, the others stay");
        assert_eq!(click(&mut settings, 2), "RGB");
    }

    #[test]
    fn at_least_one_chip_stays_on() {
        let mut settings = PanelSettings::default();
        assert_eq!(click(&mut settings, 0), "Y", "the only chip on cannot be turned off");
        click(&mut settings, 2);
        click(&mut settings, 0);
        assert_eq!(click(&mut settings, 2), "G");
    }

    #[test]
    fn every_combination_is_a_valid_setting_and_is_remembered_per_panel() {
        let mut settings = PanelSettings::default();
        click(&mut settings, 1);
        click(&mut settings, 3);
        let back = PanelSettings::from_text(&settings.to_text());
        assert_eq!(on(&Toolbar::new(&TOOLBAR, &back.resolve(NAME, &SETTINGS))), "YRB");
        for mask in 1u8..16 {
            let value: String = ["y", "r", "g", "b"].iter().enumerate().filter(|(i, _)| mask >> i & 1 == 1).map(|(_, c)| *c).collect();
            let mut s = PanelSettings::default();
            s.set(NAME, CHANNELS_SETTING, &value);
            assert_eq!(s.resolve(NAME, &SETTINGS)[CHANNELS_SETTING], value);
        }
    }

    #[test]
    fn the_popover_keeps_the_other_settings() {
        let left = popover_defs(&SETTINGS, &TOOLBAR);
        assert!(left.iter().all(|d| d.key != CHANNELS_SETTING));
        assert_eq!(left.len(), SETTINGS.len() - 2);
    }

    #[test]
    fn the_tags_name_the_channels_when_not_just_luma() {
        use scopecreep::display::DisplayContext;
        use scopecreep::settings::Settings;
        let tags = |value: Option<&str>| {
            let settings: Settings = value.map(|v| [(CHANNELS_SETTING.to_string(), v.to_string())].into()).unwrap_or_default();
            scopecreep::window_text::panel_title(NAME, &DisplayContext { settings, ..DisplayContext::default() }, String::new()).tags
        };
        assert!(!tags(None).contains(&"Y".to_string()) && !tags(Some("y")).iter().any(|t| t == "Y"));
        assert_eq!(tags(Some("rg"))[0], "R G");
        assert_eq!(tags(Some("yrgb"))[0], "Y R G B");
    }
}

// ------------------------------------------------------------------ the luma waveform's Colorise chip (#143)

mod colorise_chip {
    use scopecreep::display::DisplayContext;
    use scopecreep::panel_grid::PanelLayout;
    use scopecreep::settings::{PanelSettings, Settings};
    use scopecreep::toolbar::Toolbar;
    use scopecreep::waveform::{CHANNELS_SETTING, COLORISE_SETTING, NAME, SETTINGS, TOOLBAR};
    use scopecreep::window_text;

    fn toolbar(settings: &PanelSettings) -> Toolbar {
        Toolbar::new(&TOOLBAR, &settings.resolve(NAME, &SETTINGS))
    }

    /// Click chip `i`: what it sets, as the Panel stores it.
    fn click(settings: &mut PanelSettings, i: usize) -> &'static str {
        let toolbar = toolbar(settings);
        let (key, value) = toolbar.click(i).unwrap();
        let value = value.to_string();
        settings.set(NAME, key, &value);
        key
    }

    fn colorise_chip(toolbar: &Toolbar) -> usize {
        toolbar.chips.iter().position(|c| c.text == "Col").expect("a Colorise chip")
    }

    #[test]
    fn the_chip_is_off_by_default_and_a_click_toggles_it_each_way() {
        let mut settings = PanelSettings::default();
        let i = colorise_chip(&toolbar(&settings));
        assert!(!toolbar(&settings).on[i]);
        assert_eq!(click(&mut settings, i), COLORISE_SETTING);
        assert!(toolbar(&settings).on[i]);
        click(&mut settings, i);
        assert!(!toolbar(&settings).on[i]);
    }

    #[test]
    fn it_is_remembered_per_panel_and_does_not_disturb_the_y_r_g_b_chips() {
        let mut settings = PanelSettings::default();
        let i = colorise_chip(&toolbar(&settings));
        click(&mut settings, i);
        assert_eq!(click(&mut settings, 1), CHANNELS_SETTING);
        let back = PanelSettings::from_text(&settings.to_text());
        assert!(toolbar(&back).on[i], "Colorise is remembered");
        assert_eq!(toolbar(&back).on[..4], [true, true, false, false], "Y and R still on");
    }

    #[test]
    fn hovering_names_it_and_it_is_a_chip_not_a_popover_row() {
        let t = toolbar(&PanelSettings::default());
        assert!(t.tooltips[colorise_chip(&t)].starts_with("Colorise"), "{:?}", t.tooltips);
        assert!(scopecreep::toolbar::popover_defs(&SETTINGS, &TOOLBAR).iter().all(|d| d.key != COLORISE_SETTING));
    }

    #[test]
    fn colorise_is_never_a_tag() {
        let tags = |value: &str| {
            let settings: Settings = [(COLORISE_SETTING.to_string(), value.to_string())].into();
            window_text::panel_title(NAME, &DisplayContext { settings, ..DisplayContext::default() }, String::new()).tags
        };
        assert_eq!(tags("on"), tags("off"));
        assert!(!tags("on").iter().any(|t| t.to_lowercase().contains("coloris")));
    }

    #[test]
    fn the_chips_stay_on_a_small_panels_title_bar_and_the_name_truncates_first() {
        let t = toolbar(&PanelSettings::default());
        let title = window_text::panel_title(NAME, &DisplayContext::default(), "the full label".into());
        for scale in [1.0, 2.0] {
            let small = PanelLayout::new([0.0, 0.0, 180.0 * scale, 200.0 * scale], scale).title;
            let (l, _, _) = window_text::lay_out_toolbar_title(&title, &t.chips, true, small, scale);
            assert_eq!(l.chips.len(), 5, "every chip is placed");
            assert!(l.name_cut, "the name truncates");
            for chip in &l.chips {
                assert!(chip[0] >= small[0] && chip[0] + chip[2] <= small[0] + small[2] + 0.01, "{chip:?} stays inside {small:?}");
            }
            assert!(l.name[0] + l.name[2] <= l.chips[0][0], "the name stops before the first chip");
        }
    }
}
