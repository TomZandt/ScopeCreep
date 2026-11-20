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
    toolbar
        .chips
        .iter()
        .zip(&toolbar.on)
        .filter(|(_, on)| **on)
        .map(|(c, _)| c.text.as_str())
        .collect()
}

#[test]
fn the_channel_view_has_y_r_g_b_chips_with_blue_on_by_default() {
    let toolbar = Toolbar::new(&TOOLBAR, &current(None));
    assert_eq!(
        toolbar
            .chips
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>(),
        ["Y", "R", "G", "B"]
    );
    assert_eq!(on(&toolbar), ["B"]);
}

#[test]
fn a_click_switches_the_channel_one_at_a_time_and_y_is_luma() {
    let mut settings = PanelSettings::default();
    for (index, (value, chip)) in [("luma", "Y"), ("red", "R"), ("green", "G"), ("blue", "B")]
        .into_iter()
        .enumerate()
    {
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
    assert_eq!(
        on(&Toolbar::new(&TOOLBAR, &back.resolve(NAME, &SETTINGS))),
        ["Y"]
    );
}

#[test]
fn the_chips_are_coloured_when_on_and_grey_when_off_and_hovering_names_them() {
    let toolbar = Toolbar::new(&TOOLBAR, &current(Some("red")));
    let drawn =
        |i: usize| toolbar.chips[i].draw([0.0, 0.0], toolbar.on[i], components::State::Normal, 1.0);
    let fill = |i: usize| match &drawn(i)[0] {
        components::Prim::Rect { colour, .. } => *colour,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        fill(1),
        scopecreep::style::rgba(scopecreep::style::CHANNEL_R),
        "R is on, in red"
    );
    assert_eq!(
        fill(2),
        scopecreep::style::rgba(scopecreep::style::SURFACE_CONTROL),
        "G is off, grey"
    );
    assert_eq!(toolbar.tooltips, ["Luma (Y)", "Red", "Green", "Blue"]);
    assert_eq!(Tooltip::new(&toolbar.tooltips[0]).lines(), ["Luma (Y)"]);
}

#[test]
fn the_popover_holds_the_settings_the_toolbar_does_not_show() {
    let left = popover_defs(&SETTINGS, &TOOLBAR);
    assert_eq!(
        left.iter().map(|d| d.key).collect::<Vec<_>>(),
        ["size"],
        "the channel is chips now, the Loupe size stays in the popover"
    );
    let menu = Menu::new(0, &left, &current(None)).unwrap();
    assert!(
        menu.rows
            .iter()
            .all(|r| !matches!(r, MenuRow::Choice { key: "channel", .. }))
    );
    // A display whose settings are all chips has no popover.
    assert!(Menu::new(0, &popover_defs(&SETTINGS[..1], &TOOLBAR), &current(None)).is_none());
}

#[test]
fn the_popover_opened_from_the_settings_button_stays_inside_the_window() {
    let menu = Menu::new(0, &popover_defs(&SETTINGS, &TOOLBAR), &current(None)).unwrap();
    for scale in [1.0, 2.0] {
        let window = [1280.0 * scale, 760.0 * scale];
        // A Panel at the window's right edge, and at its bottom right.
        for cell in [
            [900.0 * scale, 20.0 * scale, 360.0 * scale, 300.0 * scale],
            [900.0 * scale, 440.0 * scale, 360.0 * scale, 300.0 * scale],
        ] {
            let bar = PanelLayout::new(cell, scale).title;
            let title = panel_title(NAME, &DisplayContext::default(), String::new());
            let toolbar = Toolbar::new(&TOOLBAR, &current(None));
            let (layout, _, _) =
                window_text::lay_out_toolbar_title(&title, &toolbar.chips, true, bar, scale);
            let button = layout.settings.expect("the settings button is on the bar");
            let rect = menu.rect([button[0], button[1] + button[3]], window, scale);
            assert!(
                rect[0] >= 0.0
                    && rect[1] >= 0.0
                    && rect[0] + rect[2] <= window[0]
                    && rect[1] + rect[3] <= window[1],
                "{rect:?} in {window:?}"
            );
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
        let (l, tags, all) =
            window_text::lay_out_toolbar_title(&title, &toolbar.chips, true, wide, scale);
        assert!(
            !l.name_cut && all && !l.tags_in_tooltip && l.tags.is_some(),
            "wide: {l:?}"
        );
        assert_eq!(l.chips.len(), 4);
        assert_eq!(tags, title.tags_text());
        // Small: the chips and the button stay whole inside the bar, the name is cut, the tags are in the hover.
        let small = PanelLayout::new([0.0, 0.0, 130.0 * scale, 200.0 * scale], scale).title;
        let (l, tags, all) =
            window_text::lay_out_toolbar_title(&title, &toolbar.chips, true, small, scale);
        assert!(l.name_cut, "the name truncates");
        assert!(
            !all && tags.is_empty() && l.tags.is_none(),
            "the tags move to the hover"
        );
        for chip in &l.chips {
            assert!(
                chip[0] >= small[0] && chip[0] + chip[2] <= small[0] + small[2] + 0.01,
                "{chip:?} stays inside {small:?}"
            );
        }
        let button = l.settings.unwrap();
        assert!(button[0] + button[2] <= small[0] + small[2] + 0.01);
        assert!(
            l.name[0] + l.name[2] <= l.chips[0][0],
            "the name stops before the first chip"
        );
        // The hover then carries the name and its tags.
        let hover = window_text::hover_text(&title, true, 60);
        assert!(hover.starts_with(&format!("{NAME}  (")), "{hover}");
    }
}

#[test]
fn the_channel_shows_in_the_tags_and_luma_says_so() {
    let tags = |channel: Option<&str>| {
        let settings: Settings = channel
            .map(|c| [("channel".to_string(), c.to_string())].into())
            .unwrap_or_default();
        panel_title(
            NAME,
            &DisplayContext {
                settings,
                ..DisplayContext::default()
            },
            String::new(),
        )
        .tags
    };
    assert_eq!(tags(Some("luma"))[0], "Luma");
    assert_eq!(tags(Some("red"))[0], "Red");
    assert_eq!(tags(None)[0], "Blue");
}

// ------------------------------------------------------------------------------------------------ B/W (#118)

use scopecreep::{arri_false_colour, picture_view, skin_tone_view, stops_false_colour};

/// Whether a Panel's own settings say its unpainted picture is grey, and whether its toolbar has a B/W chip on.
fn bw_state(
    name: &str,
    defs: &[scopecreep::settings::SettingDef],
    toolbar: &[scopecreep::toolbar::ToolbarDef],
    set: &[(&str, &str)],
) -> (Vec<String>, Vec<bool>) {
    let mut s = PanelSettings::default();
    for (k, v) in set {
        s.set(name, k, v);
    }
    let t = Toolbar::new(toolbar, &s.resolve(name, defs));
    (t.chips.iter().map(|c| c.text.clone()).collect(), t.on)
}

#[test]
fn the_clean_picture_and_skin_tone_start_in_colour_and_arri_and_stops_start_grey() {
    let (chips, on) = bw_state(
        "Clean picture",
        &picture_view::SETTINGS,
        &picture_view::TOOLBAR,
        &[],
    );
    assert_eq!(
        (chips, on),
        (
            ["Y", "R", "G", "B", "L"].map(String::from).to_vec(),
            vec![false; 5]
        )
    );
    assert_eq!(
        bw_state(
            "Skin tone",
            &skin_tone_view::SETTINGS,
            &skin_tone_view::TOOLBAR,
            &[]
        )
        .1,
        [false]
    );
    assert_eq!(
        bw_state(
            "ARRI False Colour",
            &arri_false_colour::SETTINGS,
            &arri_false_colour::TOOLBAR,
            &[]
        )
        .1,
        [true]
    );
    // Stops' toolbar is the mode chips then B/W: Zones (default), Grey +1, B/W.
    assert_eq!(
        bw_state(
            "Stops False Colour",
            &stops_false_colour::SETTINGS,
            &stops_false_colour::TOOLBAR,
            &[]
        )
        .1,
        [true, false, true]
    );
}

#[test]
fn the_clean_picture_channel_chips_switch_turn_off_on_a_second_click_and_are_remembered() {
    let mut s = PanelSettings::default();
    let name = "Clean picture";
    let click = |s: &mut PanelSettings, i: usize| {
        let t = Toolbar::new(
            &picture_view::TOOLBAR,
            &s.resolve(name, &picture_view::SETTINGS),
        );
        let (k, v) = t.click(i).unwrap();
        s.set(name, k, v);
    };
    let on = |s: &PanelSettings| {
        Toolbar::new(
            &picture_view::TOOLBAR,
            &s.resolve(name, &picture_view::SETTINGS),
        )
        .on
    };
    click(&mut s, 3);
    let back = PanelSettings::from_text(&s.to_text());
    assert_eq!(on(&back), [false, false, false, true, false]);
    assert_eq!(
        Toolbar::new(
            &skin_tone_view::TOOLBAR,
            &back.resolve("Skin tone", &skin_tone_view::SETTINGS)
        )
        .on,
        [false],
        "another Panel is unaffected"
    );
    click(&mut s, 1);
    assert_eq!(
        on(&s),
        [false, true, false, false, false],
        "straight to another channel"
    );
    click(&mut s, 1);
    assert_eq!(on(&s), [false; 5], "a second click is colour again");
}

// ---- small Panels (#171): chips that do not fit go to the popover, never past the title bar

/// A zoom chip as a picture Panel shows it at Fit.
fn fit_readout() -> scopecreep::zoom::Readout {
    scopecreep::zoom::Readout {
        text: "Fit 75 %".into(),
        short: "75 %".into(),
        percent: 75.0,
        fit: 75.0,
        max: 800.0,
    }
}

/// The title bar of a Panel `width` logical pixels wide at `scale`.
fn bar(width: f32, scale: f32) -> [f32; 4] {
    PanelLayout::new([0.0, 0.0, width * scale, 200.0 * scale], scale).title
}

/// Lay `toolbar` out on `bar` after fitting it, and check every chip and the settings button lie inside the bar.
/// Returns the layout and the popover's setting keys.
fn fit_inside(
    toolbar: &mut Toolbar,
    defs: &[scopecreep::settings::SettingDef],
    toolbar_defs: &[scopecreep::toolbar::ToolbarDef],
    bar: [f32; 4],
    scale: f32,
    what: &str,
) -> (components::TitleLayout, Vec<&'static str>) {
    toolbar.fit_to(bar[2], !popover_defs(defs, toolbar_defs).is_empty(), scale);
    let popover = scopecreep::toolbar::popover_with(defs, toolbar_defs, &toolbar.collapsed);
    let title = panel_title(what, &DisplayContext::default(), String::new());
    let (l, _, _) =
        window_text::lay_out_toolbar_title(&title, &toolbar.chips, !popover.is_empty(), bar, scale);
    let inside = |r: [f32; 4]| r[0] >= bar[0] && r[0] + r[2] <= bar[0] + bar[2] + 0.01;
    assert_eq!(l.chips.len(), toolbar.chips.len(), "{what}");
    for chip in &l.chips {
        assert!(inside(*chip), "{what}: {chip:?} stays inside {bar:?}");
    }
    let button = l.settings.expect("the settings button always stays");
    assert!(inside(button), "{what}: {button:?} stays inside {bar:?}");
    if let Some(last) = l.chips.last() {
        assert!(
            last[0] + last[2] <= button[0],
            "{what}: no chip runs under the settings button"
        );
    }
    (l, popover.iter().map(|d| d.key).collect())
}

fn clean_picture(set: &[(&str, &str)]) -> Settings {
    let mut s = PanelSettings::default();
    for (k, v) in set {
        s.set("Clean picture", k, v);
    }
    s.resolve("Clean picture", &picture_view::SETTINGS)
}

#[test]
fn at_the_minimum_panel_width_the_clean_picture_chips_go_to_the_popover_and_stay_reachable() {
    let own = clean_picture(&[("channel", "blue"), ("loupe", "on")]);
    for scale in [1.0, 2.0] {
        let small = bar(scopecreep::style::SPACE_MIN_PANEL_WIDTH, scale);
        let mut toolbar = Toolbar::new(&picture_view::TOOLBAR, &own)
            .with_zoom(Some(&fit_readout()))
            .with_link(true);
        let (_, popover) = fit_inside(
            &mut toolbar,
            &picture_view::SETTINGS,
            &picture_view::TOOLBAR,
            small,
            scale,
            "Clean picture",
        );
        assert_eq!(
            toolbar
                .chips
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            ["75 %"],
            "the zoom chip, first, stays; the rest leave from the right"
        );
        assert_eq!(toolbar.collapsed, ["link", "channel", "loupe"]);
        assert_eq!(
            popover,
            ["link", "channel", "loupe", "size"],
            "every collapsed choice is in the popover, before the Loupe size"
        );
        // The popover shows each choice as it is, and its rows are what the chips would set.
        let mut current = own.clone();
        current.insert("link".into(), "on".into());
        let defs = scopecreep::toolbar::popover_with(
            &picture_view::SETTINGS,
            &picture_view::TOOLBAR,
            &toolbar.collapsed,
        );
        let menu = Menu::new(0, &defs, &current).unwrap();
        let lit: Vec<(&str, &str)> = menu
            .rows
            .iter()
            .filter_map(|r| match r {
                MenuRow::Choice {
                    key,
                    value,
                    on: true,
                    ..
                } => Some((*key, *value)),
                _ => None,
            })
            .collect();
        assert_eq!(
            lit,
            [
                ("link", "on"),
                ("channel", "blue"),
                ("loupe", "on"),
                ("size", "medium")
            ]
        );
    }
}

#[test]
fn chips_leave_a_setting_at_a_time_so_a_lit_channel_is_never_hidden_beside_its_neighbours() {
    // Without a zoom chip (as the toolbar is built on its own), the Loupe leaves first, then all of Y R G B.
    let own = clean_picture(&[("channel", "blue")]);
    for scale in [1.0, 2.0] {
        let mut toolbar = Toolbar::new(&picture_view::TOOLBAR, &own);
        fit_inside(
            &mut toolbar,
            &picture_view::SETTINGS,
            &picture_view::TOOLBAR,
            bar(170.0, scale),
            scale,
            "Clean picture",
        );
        assert_eq!(
            toolbar
                .chips
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            ["Y", "R", "G", "B"]
        );
        assert_eq!(toolbar.on, [false, false, false, true]);
        assert_eq!(toolbar.collapsed, ["loupe"]);
        let mut toolbar = Toolbar::new(&picture_view::TOOLBAR, &own);
        fit_inside(
            &mut toolbar,
            &picture_view::SETTINGS,
            &picture_view::TOOLBAR,
            bar(scopecreep::style::SPACE_MIN_PANEL_WIDTH, scale),
            scale,
            "Clean picture",
        );
        assert!(toolbar.chips.is_empty(), "never a part of the channel row");
        assert_eq!(toolbar.collapsed, ["channel", "loupe"]);
    }
}

#[test]
fn with_room_the_clean_picture_shows_every_chip_as_before() {
    let own = clean_picture(&[("channel", "blue")]);
    for scale in [1.0, 2.0] {
        let mut toolbar = Toolbar::new(&picture_view::TOOLBAR, &own);
        let (_, popover) = fit_inside(
            &mut toolbar,
            &picture_view::SETTINGS,
            &picture_view::TOOLBAR,
            // Five chips, the settings button and the pads take 190 px; the Panel's border takes 2 more.
            bar(192.0, scale),
            scale,
            "Clean picture",
        );
        assert_eq!(
            toolbar
                .chips
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            ["Y", "R", "G", "B", "L"]
        );
        assert!(toolbar.collapsed.is_empty());
        assert_eq!(popover, ["size"], "the popover is as before");
        let mut toolbar = Toolbar::new(&picture_view::TOOLBAR, &own)
            .with_zoom(Some(&fit_readout()))
            .with_link(true);
        fit_inside(
            &mut toolbar,
            &picture_view::SETTINGS,
            &picture_view::TOOLBAR,
            bar(400.0, scale),
            scale,
            "Clean picture",
        );
        assert_eq!(
            toolbar
                .chips
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            ["Fit 75 %", "Link", "Y", "R", "G", "B", "L"]
        );
        assert!(toolbar.collapsed.is_empty());
    }
}

#[test]
fn the_zoom_and_link_chips_of_the_other_picture_panels_obey_the_minimum_panel_width() {
    let grey: Settings = [("mode".to_string(), "grey-plus-one".to_string())].into();
    let panels: [(
        &str,
        &[scopecreep::settings::SettingDef],
        &[scopecreep::toolbar::ToolbarDef],
        Settings,
    ); 3] = [
        (
            "ARRI False Colour",
            &arri_false_colour::SETTINGS,
            &arri_false_colour::TOOLBAR,
            Settings::default(),
        ),
        (
            "Skin tone",
            &skin_tone_view::SETTINGS,
            &skin_tone_view::TOOLBAR,
            Settings::default(),
        ),
        (
            "Stops False Colour",
            &stops_false_colour::SETTINGS,
            stops_false_colour::toolbar_for(&grey),
            grey.clone(),
        ),
    ];
    for (name, defs, toolbar_defs, own) in panels {
        for scale in [1.0, 2.0] {
            let mut toolbar = Toolbar::new(toolbar_defs, &own)
                .with_zoom(Some(&fit_readout()))
                .with_link(false);
            let (_, popover) = fit_inside(
                &mut toolbar,
                defs,
                toolbar_defs,
                bar(scopecreep::style::SPACE_MIN_PANEL_WIDTH, scale),
                scale,
                name,
            );
            assert_eq!(toolbar.chips[0].text, "75 %", "{name}: the zoom chip stays");
            assert!(toolbar.collapsed.contains(&"link"), "{name}");
            for key in &toolbar.collapsed {
                assert!(popover.contains(key), "{name}: {key} is in the popover");
            }
        }
    }
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
    assert_eq!(
        t.chips.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
        ["Zones", "Grey +1"],
        "full zones is the default; B/W is hidden"
    );
    assert_eq!(t.on, [true, false]);
    let (k, v) = t.click(1).unwrap();
    s.set(name, k, v);
    let t = toolbar(&PanelSettings::from_text(&s.to_text()));
    assert_eq!(
        t.chips.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
        ["Zones", "Grey +1", "B/W"]
    );
    assert_eq!(t.on, [false, true, true], "remembered, and B/W shows");
    assert_eq!(
        t.click(1).unwrap(),
        ("mode", "grey-plus-one"),
        "clicking the chip already on keeps it on"
    );
    let (k, v) = t.click(0).unwrap();
    s.set(name, k, v);
    assert_eq!(toolbar(&s).on, [true, false]);
}

#[test]
fn stops_shows_bw_only_in_grey_and_plus_one_and_the_heatmap_has_none() {
    let zones: Settings = [("mode".to_string(), "zones".to_string())].into();
    let grey: Settings = [("mode".to_string(), "grey-plus-one".to_string())].into();
    assert_eq!(
        Toolbar::new(stops_false_colour::toolbar_for(&zones), &zones)
            .chips
            .len(),
        2
    );
    assert_eq!(
        Toolbar::new(stops_false_colour::toolbar_for(&grey), &grey)
            .chips
            .len(),
        3
    );
    assert_eq!(
        Toolbar::new(
            stops_false_colour::toolbar_for(&Settings::default()),
            &Settings::default()
        )
        .chips
        .len(),
        2,
        "the default mode is full zones"
    );
}

#[test]
fn the_channel_is_a_chip_not_a_popover_row() {
    let left = popover_defs(&picture_view::SETTINGS, &picture_view::TOOLBAR);
    assert_eq!(
        left.iter().map(|d| d.key).collect::<Vec<_>>(),
        ["size"],
        "the clean picture's popover holds the Loupe size only"
    );
    assert_eq!(
        popover_defs(&stops_false_colour::SETTINGS, &stops_false_colour::TOOLBAR)
            .iter()
            .map(|d| d.key)
            .collect::<Vec<_>>(),
        ["legend"]
    );
}

#[test]
fn the_tags_say_b_w_or_colour_when_the_toggle_is_not_at_its_default() {
    let tags = |name: &str, v: &str| {
        let settings: Settings = [
            ("bw".to_string(), v.to_string()),
            ("mode".to_string(), "grey-plus-one".to_string()),
        ]
        .into();
        panel_title(
            name,
            &DisplayContext {
                settings,
                ..DisplayContext::default()
            },
            String::new(),
        )
        .tags
    };
    let channel_tags = |v: &str| {
        let settings: Settings = [("channel".to_string(), v.to_string())].into();
        panel_title(
            "Clean picture",
            &DisplayContext {
                settings,
                ..DisplayContext::default()
            },
            String::new(),
        )
        .tags
    };
    assert!(channel_tags("blue").contains(&"Blue".to_string()));
    assert!(channel_tags("luma").contains(&"Luma".to_string()));
    assert!(
        !channel_tags("off")
            .iter()
            .any(|t| matches!(t.as_str(), "Luma" | "Red" | "Green" | "Blue" | "colour")),
        "colour is the default and is not tagged"
    );
    assert!(tags("Skin tone", "on").contains(&"B/W".to_string()));
    assert!(tags("ARRI False Colour", "off").contains(&"colour".to_string()));
    assert!(
        !tags("ARRI False Colour", "on")
            .iter()
            .any(|t| t == "B/W" || t == "colour")
    );
    assert!(tags("Stops False Colour", "off").contains(&"colour".to_string()));
}

// ------------------------------------------------------------------ the luma waveform's chips (#117)

mod waveform_chips {
    use scopecreep::settings::PanelSettings;
    use scopecreep::toolbar::{Toolbar, popover_defs};
    use scopecreep::waveform::{CHANNELS_SETTING, NAME, SETTINGS, TOOLBAR};

    fn on(toolbar: &Toolbar) -> String {
        toolbar
            .chips
            .iter()
            .zip(&toolbar.on)
            .filter(|(_, on)| **on)
            .map(|(c, _)| c.text.as_str())
            .collect()
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
        assert_eq!(
            on(&Toolbar::new(&TOOLBAR, &settings.resolve(NAME, &SETTINGS))),
            "Y"
        );
        assert_eq!(click(&mut settings, 1), "YR");
        assert_eq!(click(&mut settings, 3), "YRB");
        assert_eq!(click(&mut settings, 0), "RB", "Y off, the others stay");
        assert_eq!(click(&mut settings, 2), "RGB");
    }

    #[test]
    fn at_least_one_chip_stays_on() {
        let mut settings = PanelSettings::default();
        assert_eq!(
            click(&mut settings, 0),
            "Y",
            "the only chip on cannot be turned off"
        );
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
        assert_eq!(
            on(&Toolbar::new(&TOOLBAR, &back.resolve(NAME, &SETTINGS))),
            "YRB"
        );
        for mask in 1u8..16 {
            let value: String = ["y", "r", "g", "b"]
                .iter()
                .enumerate()
                .filter(|(i, _)| mask >> i & 1 == 1)
                .map(|(_, c)| *c)
                .collect();
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
            let settings: Settings = value
                .map(|v| [(CHANNELS_SETTING.to_string(), v.to_string())].into())
                .unwrap_or_default();
            scopecreep::window_text::panel_title(
                NAME,
                &DisplayContext {
                    settings,
                    ..DisplayContext::default()
                },
                String::new(),
            )
            .tags
        };
        assert!(
            !tags(None).contains(&"Y".to_string()) && !tags(Some("y")).iter().any(|t| t == "Y")
        );
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
        toolbar
            .chips
            .iter()
            .position(|c| c.text == "Col")
            .expect("a Colorise chip")
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
        assert_eq!(
            toolbar(&back).on[..4],
            [true, true, false, false],
            "Y and R still on"
        );
    }

    #[test]
    fn hovering_names_it_and_it_is_a_chip_not_a_popover_row() {
        let t = toolbar(&PanelSettings::default());
        assert!(
            t.tooltips[colorise_chip(&t)].starts_with("Colorise"),
            "{:?}",
            t.tooltips
        );
        assert!(
            scopecreep::toolbar::popover_defs(&SETTINGS, &TOOLBAR)
                .iter()
                .all(|d| d.key != COLORISE_SETTING)
        );
    }

    #[test]
    fn colorise_is_never_a_tag() {
        let tags = |value: &str| {
            let settings: Settings = [(COLORISE_SETTING.to_string(), value.to_string())].into();
            window_text::panel_title(
                NAME,
                &DisplayContext {
                    settings,
                    ..DisplayContext::default()
                },
                String::new(),
            )
            .tags
        };
        assert_eq!(tags("on"), tags("off"));
        assert!(
            !tags("on")
                .iter()
                .any(|t| t.to_lowercase().contains("coloris"))
        );
    }

    #[test]
    fn the_chips_stay_on_a_small_panels_title_bar_and_the_name_truncates_first() {
        let t = toolbar(&PanelSettings::default());
        let title =
            window_text::panel_title(NAME, &DisplayContext::default(), "the full label".into());
        for scale in [1.0, 2.0] {
            let small = PanelLayout::new([0.0, 0.0, 180.0 * scale, 200.0 * scale], scale).title;
            let (l, _, _) =
                window_text::lay_out_toolbar_title(&title, &t.chips, true, small, scale);
            assert_eq!(l.chips.len(), 5, "every chip is placed");
            assert!(l.name_cut, "the name truncates");
            for chip in &l.chips {
                assert!(
                    chip[0] >= small[0] && chip[0] + chip[2] <= small[0] + small[2] + 0.01,
                    "{chip:?} stays inside {small:?}"
                );
            }
            assert!(
                l.name[0] + l.name[2] <= l.chips[0][0],
                "the name stops before the first chip"
            );
        }
    }
}

// ------------------------------------------------------------------ the vectorscope's chips (#123)

mod vectorscope_chips {
    use scopecreep::display::DisplayContext;
    use scopecreep::settings::{PanelSettings, Settings};
    use scopecreep::toolbar::{Toolbar, popover_defs};
    use scopecreep::vectorscope::{NAME, SETTINGS, TOOLBAR};
    use scopecreep::window_text::panel_title;

    fn toolbar(s: &PanelSettings) -> Toolbar {
        Toolbar::new(&TOOLBAR, &s.resolve(NAME, &SETTINGS))
    }

    #[test]
    fn plots_are_three_chips_one_at_a_time_both_by_default_and_mono_is_a_toggle_off_by_default() {
        let mut s = PanelSettings::default();
        let t = toolbar(&s);
        assert_eq!(
            t.chips.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
            [
                "100 %",
                "200 %",
                "Both",
                "Mono",
                "All",
                "Shadows",
                "Mids",
                "Highlights"
            ]
        );
        assert_eq!(t.on, [false, false, true, false, true, false, false, false]);
        let (k, v) = t.click(0).unwrap();
        s.set(NAME, k, v);
        assert_eq!(
            toolbar(&s).on,
            [true, false, false, false, true, false, false, false]
        );
        let t = toolbar(&s);
        let (k, v) = t.click(3).unwrap();
        s.set(NAME, k, v);
        assert_eq!(
            toolbar(&s).on,
            [true, false, false, true, true, false, false, false]
        );
        let t = toolbar(&s);
        let (k, v) = t.click(3).unwrap();
        s.set(NAME, k, v);
        assert_eq!(
            toolbar(&s).on,
            [true, false, false, false, true, false, false, false],
            "a second click turns mono off"
        );
    }

    #[test]
    fn the_choices_are_remembered_per_panel_and_the_popover_keeps_the_rest() {
        let mut s = PanelSettings::default();
        s.set(NAME, "plots", "200");
        s.set(NAME, "mono", "on");
        let back = PanelSettings::from_text(&s.to_text());
        assert_eq!(
            toolbar(&back).on,
            [false, true, false, true, true, false, false, false]
        );
        assert_eq!(
            popover_defs(&SETTINGS, &TOOLBAR)
                .iter()
                .map(|d| d.key)
                .collect::<Vec<_>>(),
            [
                "brightness",
                "spread",
                "graticule",
                "graticule-style",
                "targets",
                "skin-line"
            ]
        );
    }

    #[test]
    fn the_plots_shown_are_tagged_and_the_trace_colour_is_not() {
        let tags = |plots: &str, mono: &str| {
            let settings: Settings = [
                ("plots".to_string(), plots.to_string()),
                ("mono".to_string(), mono.to_string()),
            ]
            .into();
            panel_title(
                NAME,
                &DisplayContext {
                    settings,
                    ..DisplayContext::default()
                },
                String::new(),
            )
            .tags
        };
        assert!(
            !tags("both", "off").iter().any(|t| t.contains("plot")),
            "both is the default"
        );
        assert!(tags("100", "off").contains(&"100 % plot only".to_string()));
        assert!(tags("200", "off").contains(&"200 % plot only".to_string()));
        assert_eq!(
            tags("both", "on"),
            tags("both", "off"),
            "mono changes no reading"
        );
    }

    #[test]
    fn the_tonal_range_is_four_chips_one_at_a_time_all_by_default_tagged_when_not_all_and_remembered()
     {
        let mut s = PanelSettings::default();
        assert_eq!(
            &toolbar(&s).on[4..],
            [true, false, false, false],
            "All is lit when nothing is stored"
        );
        for (chip, value, tag) in [
            (5, "shadows", "shadows only"),
            (6, "mids", "mids only"),
            (7, "highlights", "highlights only"),
        ] {
            let t = toolbar(&s);
            let (k, v) = t.click(chip).unwrap();
            assert_eq!((k, v), ("tonal", value));
            s.set(NAME, k, v);
            let on = toolbar(&PanelSettings::from_text(&s.to_text())).on;
            assert_eq!(
                &on[4..],
                (4..8).map(|i| i == chip).collect::<Vec<_>>(),
                "{value} is the only one lit, and remembered"
            );
            let settings: Settings = [("tonal".to_string(), value.to_string())].into();
            let tags = panel_title(
                NAME,
                &DisplayContext {
                    settings,
                    ..DisplayContext::default()
                },
                String::new(),
            )
            .tags;
            assert!(tags.contains(&tag.to_string()), "{tags:?}");
        }
        let settings: Settings = [("tonal".to_string(), "all".to_string())].into();
        let tags = panel_title(
            NAME,
            &DisplayContext {
                settings,
                ..DisplayContext::default()
            },
            String::new(),
        )
        .tags;
        assert!(
            !tags.iter().any(|t| t.contains("only")),
            "All is not tagged: {tags:?}"
        );
    }
}

#[test]
fn a_vectorscope_setting_is_a_chip_or_in_the_popover_never_both() {
    use scopecreep::vectorscope::{SETTINGS, TOOLBAR};
    let popover: Vec<&str> = popover_defs(&SETTINGS, &TOOLBAR)
        .iter()
        .map(|d| d.key)
        .collect();
    for chip in TOOLBAR.iter() {
        assert!(
            !popover.contains(&chip.key),
            "{} is a chip and in the popover",
            chip.key
        );
        assert!(
            SETTINGS.iter().any(|d| d.key == chip.key),
            "{} is a chip without a setting",
            chip.key
        );
    }
    let mut seen = popover.clone();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), popover.len(), "no setting twice in the popover");
}

#[test]
fn the_clean_picture_loupe_chip_is_off_by_default_toggles_is_remembered_and_tags_only_when_on() {
    let name = "Clean picture";
    let mut s = PanelSettings::default();
    let toolbar = |s: &PanelSettings| {
        Toolbar::new(
            &picture_view::TOOLBAR,
            &s.resolve(name, &picture_view::SETTINGS),
        )
    };
    assert!(!toolbar(&s).on[4], "off by default");
    let tags = |s: &PanelSettings| {
        let mut own = s.resolve(name, &picture_view::SETTINGS);
        own.extend(s.state_of(name, scopecreep::loupe::STATE_KEYS));
        panel_title(
            name,
            &DisplayContext {
                settings: own,
                ..DisplayContext::default()
            },
            String::new(),
        )
        .tags
    };
    assert!(
        !tags(&s).iter().any(|t| t.starts_with("Loupe")),
        "no tag when off"
    );
    let t = toolbar(&s);
    let (k, v) = t.click(4).unwrap();
    s.set(name, k, v);
    let back = PanelSettings::from_text(&s.to_text());
    assert!(toolbar(&back).on[4], "on, and remembered");
    assert!(
        tags(&back).contains(&"Loupe 200 %".to_string()),
        "{:?}",
        tags(&back)
    );
    let mut zoomed = back.clone();
    zoomed.set_state(name, "loupe_zoom", "400");
    assert!(
        tags(&zoomed).contains(&"Loupe 400 %".to_string()),
        "{:?}",
        tags(&zoomed)
    );
    let t = toolbar(&back);
    let (k, v) = t.click(4).unwrap();
    s.set(name, k, v);
    assert!(!toolbar(&s).on[4], "a second click turns it off");
    assert!(!tags(&s).iter().any(|t| t.starts_with("Loupe")));
}
