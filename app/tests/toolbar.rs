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
        assert_eq!(left.len(), SETTINGS.len() - 1);
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
