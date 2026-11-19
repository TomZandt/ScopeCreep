//! Panel settings and the right-click menu, on the pure seam (#51): no GPU, no window.
use scopecreep::settings::{Choice, Menu, MenuRow, PanelSettings, SettingDef, MENU_ROW};

const DEFS: [SettingDef; 2] = [
    SettingDef { key: "weights", label: "Weights", amount: None, choices: &[Choice { value: "a", text: "Alpha" }, Choice { value: "b", text: "Beta" }] },
    SettingDef { key: "spread", label: "Spread", amount: None, choices: &[Choice { value: "on", text: "On" }, Choice { value: "off", text: "Off" }] },
];

#[test]
fn an_untouched_panel_uses_the_first_choice_of_each_setting() {
    let settings = PanelSettings::default().resolve("Panel", &DEFS);
    assert_eq!(settings["weights"], "a");
    assert_eq!(settings["spread"], "on");
}

#[test]
fn settings_are_kept_per_panel() {
    let mut all = PanelSettings::default();
    all.set("One", "weights", "b");
    assert_eq!(all.resolve("One", &DEFS)["weights"], "b");
    assert_eq!(all.resolve("Two", &DEFS)["weights"], "a", "another Panel is not affected");
}

#[test]
fn settings_round_trip_through_the_saved_text() {
    let mut all = PanelSettings::default();
    all.set("Luma waveform", "luma", "match-resolve");
    all.set("Other panel", "spread", "off");
    assert_eq!(PanelSettings::from_text(&all.to_text()), all);
}

#[test]
fn rubbish_in_the_saved_text_is_ignored_and_a_stale_choice_is_not_used() {
    let all = PanelSettings::from_text("not a setting\n\nPanel\tweights\tzzz\nPanel\tspread\toff\na\tb\n");
    let settings = all.resolve("Panel", &DEFS);
    assert_eq!(settings["weights"], "a", "zzz is not a choice, so the default");
    assert_eq!(settings["spread"], "off");
}

#[test]
fn a_panel_with_no_settings_has_no_menu() {
    assert_eq!(Menu::new(0, &[], &Default::default()), None);
}

#[test]
fn the_menu_lists_each_setting_with_its_choices_and_ticks_the_current_one() {
    let mut all = PanelSettings::default();
    all.set("P", "weights", "b");
    let menu = Menu::new(3, &DEFS, &all.resolve("P", &DEFS)).unwrap();
    assert_eq!(menu.panel, 3);
    let ticked: Vec<&str> = menu.rows.iter().filter_map(|r| if let MenuRow::Choice { value, on: true, .. } = r { Some(*value) } else { None }).collect();
    assert_eq!(ticked, ["b", "on"]);
    assert_eq!(menu.rows.len(), 6, "two headings and four choices");
    assert_eq!(menu.rows[0], MenuRow::Heading("Weights".into()));
}

#[test]
fn a_click_lands_on_the_choice_under_it_and_never_on_a_heading() {
    let menu = Menu::new(0, &DEFS, &PanelSettings::default().resolve("P", &DEFS)).unwrap();
    for scale in [1.0_f32, 2.0] {
        let rect = menu.rect([100.0, 100.0], [1280.0, 760.0], scale);
        let row_middle = |row: usize| [rect[0] + rect[2] / 2.0, Menu::row_top(rect, row, scale) + MENU_ROW * scale / 2.0];
        assert_eq!(menu.choice_at(rect, row_middle(0), scale), None, "a heading");
        assert_eq!(menu.choice_at(rect, row_middle(1), scale), Some(("weights", "a")));
        assert_eq!(menu.choice_at(rect, row_middle(2), scale), Some(("weights", "b")));
        assert_eq!(menu.choice_at(rect, row_middle(3), scale), None, "the next heading");
        assert_eq!(menu.choice_at(rect, row_middle(5), scale), Some(("spread", "off")));
        assert_eq!(menu.choice_at(rect, [rect[0] - 1.0, row_middle(1)[1]], scale), None, "left of the menu");
        assert_eq!(menu.choice_at(rect, [rect[0] + 5.0, rect[1] + rect[3] + 2.0], scale), None, "under the menu");
    }
}

#[test]
fn the_menu_stays_inside_the_window() {
    let menu = Menu::new(0, &DEFS, &PanelSettings::default().resolve("P", &DEFS)).unwrap();
    let rect = menu.rect([1270.0, 750.0], [1280.0, 760.0], 1.0);
    assert!(rect[0] >= 0.0 && rect[1] >= 0.0 && rect[0] + rect[2] <= 1280.0 && rect[1] + rect[3] <= 760.0, "{rect:?}");
}

#[test]
fn the_version_goes_up_only_when_a_choice_changes() {
    let mut all = PanelSettings::default();
    let v0 = all.version();
    all.set("One", "weights", "b");
    let v1 = all.version();
    assert!(v1 > v0);
    all.set("One", "weights", "b");
    assert_eq!(all.version(), v1, "the same choice again changes nothing");
    assert!(all.set_state("One", "zoom", "200"));
    assert!(!all.set_state("One", "zoom", "200"));
    assert_eq!(all.version(), v1, "remembered state is not a setting");
}

#[test]
fn saving_replaces_the_file_whole_and_leaves_no_temporary_behind() {
    let dir = std::env::temp_dir().join(format!("scopecreep-save-test-{}", std::process::id()));
    let path = dir.join("nested").join("panel-settings");
    scopecreep::settings::save(&path, "first\n").unwrap();
    scopecreep::settings::save(&path, "second\n").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second\n");
    let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["panel-settings"]);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Amounts and sliders (#84) ----

use scopecreep::settings::Amount;
use scopecreep::waveform::{look_from, Brightness, Look, BRIGHTNESS_AMOUNT};

const AMOUNT_DEFS: [SettingDef; 1] = [SettingDef { key: "brightness", label: "Trace brightness", amount: Some(BRIGHTNESS_AMOUNT), choices: &[] }];

fn brightness_of(all: &PanelSettings) -> f32 {
    all.resolve("P", &AMOUNT_DEFS)["brightness"].parse().unwrap()
}

#[test]
fn an_untouched_amount_is_its_default_and_the_default_is_todays_normal_look() {
    assert_eq!(brightness_of(&PanelSettings::default()), 100.0);
    assert_eq!(look_from(&PanelSettings::default().resolve("P", &AMOUNT_DEFS)), Look::default());
    assert_eq!(Look::default().brightness, Brightness(100.0));
}

#[test]
fn an_amount_is_clamped_to_its_range_and_rubbish_falls_back_to_the_default() {
    let read = |v: &str| brightness_of(&PanelSettings::from_text(&format!("P\tbrightness\t{v}\n")));
    assert_eq!(read("170"), 170.0);
    assert_eq!(read("9999"), BRIGHTNESS_AMOUNT.max);
    assert_eq!(read("-5"), BRIGHTNESS_AMOUNT.min);
    assert_eq!(read("lots"), 100.0);
    assert_eq!(read("NaN"), 100.0);
}

#[test]
fn a_list_value_saved_by_an_older_version_maps_to_the_number_it_produced() {
    let read = |v: &str| brightness_of(&PanelSettings::from_text(&format!("P\tbrightness\t{v}\n")));
    assert_eq!((read("normal"), read("dimmer"), read("dim"), read("brighter")), (100.0, 48.0, 48.0, 240.0));
    // The gains they gave before were 0.12, 0.25 and 0.6: 48 %, 100 % and 240 % of 0.25.
    let gain = |v: &str| look_from(&PanelSettings::from_text(&format!("P\tbrightness\t{v}\n")).resolve("P", &AMOUNT_DEFS)).brightness;
    assert_eq!(gain("normal"), Brightness(100.0));
    assert_eq!(gain("dimmer"), Brightness(48.0));
}

#[test]
fn an_amount_round_trips_through_the_saved_text() {
    let mut all = PanelSettings::default();
    all.set("P", "brightness", &Amount::store(137.5));
    assert_eq!(brightness_of(&PanelSettings::from_text(&all.to_text())), 137.5);
}

#[test]
fn a_menu_for_an_amount_has_a_heading_a_slider_and_a_reset_row() {
    let mut all = PanelSettings::default();
    all.set("P", "brightness", "150");
    let menu = Menu::new(0, &AMOUNT_DEFS, &all.resolve("P", &AMOUNT_DEFS)).unwrap();
    assert_eq!(menu.rows.len(), 3);
    assert_eq!(menu.rows[0], MenuRow::Heading("Trace brightness".into()));
    assert!(matches!(&menu.rows[1], MenuRow::Slider { key: "brightness", value, .. } if *value == 150.0));
    assert!(matches!(&menu.rows[2], MenuRow::Reset { value, text, .. } if *value == 100.0 && text == "Reset to default (100 %)"));
}

#[test]
fn pressing_on_the_track_gives_a_value_in_steps_and_dragging_past_the_ends_gives_the_ends() {
    let menu = Menu::new(0, &AMOUNT_DEFS, &PanelSettings::default().resolve("P", &AMOUNT_DEFS)).unwrap();
    for scale in [1.0_f32, 2.0] {
        let rect = menu.rect([100.0, 100.0], [1280.0, 760.0], scale);
        let at = Menu::slider_origin(rect, 1, scale);
        let slider = scopecreep::components::Slider { value: 0.0, number: String::new() };
        let track = slider.layout(at, scale).track;
        let y = at[1] + 12.0 * scale;
        let (min, max) = (BRIGHTNESS_AMOUNT.min, BRIGHTNESS_AMOUNT.max);
        assert_eq!(menu.slider_at(rect, [track[0], y], scale), Some(("brightness", min)));
        assert_eq!(menu.slider_at(rect, [track[0] + track[2], y], scale), Some(("brightness", max)));
        let (_, middle) = menu.slider_at(rect, [track[0] + track[2] / 2.0, y], scale).unwrap();
        assert_eq!((middle - min) % BRIGHTNESS_AMOUNT.step, 0.0, "a multiple of the step: {middle}");
        assert!((middle - (min + max) / 2.0).abs() <= BRIGHTNESS_AMOUNT.step);
        // A drag goes on beyond the track and out of the menu, and clamps.
        assert_eq!(menu.slider_value(rect, "brightness", track[0] - 500.0, scale), Some(min));
        assert_eq!(menu.slider_value(rect, "brightness", track[0] + track[2] + 5000.0, scale), Some(max));
        assert_eq!(menu.slider_value(rect, "nothing", track[0], scale), None);
        // A press on the heading is not a slider press, and neither is one in the slider row's left padding.
        assert_eq!(menu.slider_at(rect, [track[0], Menu::row_top(rect, 0, scale) + 2.0], scale), None);
        assert_eq!(menu.slider_at(rect, [rect[0] + 1.0, y], scale), None);
        // Neither the slider row nor the reset row is a choice.
        assert_eq!(menu.choice_at(rect, [track[0], y], scale), None);
    }
}

#[test]
fn the_reset_row_gives_the_default_and_is_not_a_slider() {
    let menu = Menu::new(0, &AMOUNT_DEFS, &PanelSettings::default().resolve("P", &AMOUNT_DEFS)).unwrap();
    let rect = menu.rect([100.0, 100.0], [1280.0, 760.0], 1.0);
    let on_reset = [rect[0] + 40.0, Menu::row_top(rect, 2, 1.0) + MENU_ROW / 2.0];
    assert_eq!(menu.reset_at(rect, on_reset, 1.0), Some(("brightness", 100.0)));
    assert_eq!(menu.slider_at(rect, on_reset, 1.0), None);
    assert_eq!(menu.reset_at(rect, [on_reset[0], Menu::row_top(rect, 1, 1.0) + 5.0], 1.0), None);
}

#[test]
fn the_slider_and_its_number_are_drawn_at_the_scale_and_inside_the_menu() {
    use scopecreep::components::Prim;
    let mut all = PanelSettings::default();
    all.set("P", "brightness", "240");
    let menu = Menu::new(0, &AMOUNT_DEFS, &all.resolve("P", &AMOUNT_DEFS)).unwrap();
    for scale in [1.0_f32, 2.0] {
        let rect = menu.rect([100.0, 100.0], [1280.0, 760.0], scale);
        let prims = scopecreep::components::popover(&menu, rect, [-1.0, -1.0], scale);
        let numbers: Vec<String> = prims.iter().filter_map(|p| if let Prim::Text { text, .. } = p { Some(text.clone()) } else { None }).collect();
        assert!(numbers.contains(&"240 %".to_string()), "{numbers:?}");
        for p in &prims {
            let (Prim::Rect { rect: r, .. } | Prim::Text { rect: r, .. }) = p;
            assert!(r[0] >= rect[0] - 2.0 * scale && r[0] + r[2] <= rect[0] + rect[2] + 2.0 * scale, "inside the menu's width: {r:?} in {rect:?}");
        }
    }
}

#[test]
fn a_saved_clean_picture_b_w_reads_as_luma_or_colour_and_other_panels_keep_theirs() {
    use scopecreep::{arri_false_colour, picture_view, stops_false_colour};
    let saved = PanelSettings::from_text("Clean picture\tbw\ton\nARRI False Colour\tbw\toff\nStops False Colour\tbw\ton\n");
    assert_eq!(saved.resolve("Clean picture", &picture_view::SETTINGS)["channel"], "luma");
    assert_eq!(saved.resolve("ARRI False Colour", &arri_false_colour::SETTINGS)["bw"], "off");
    assert_eq!(saved.resolve("Stops False Colour", &stops_false_colour::SETTINGS)["bw"], "on");
    let off = PanelSettings::from_text("Clean picture\tbw\toff\n");
    assert_eq!(off.resolve("Clean picture", &picture_view::SETTINGS)["channel"], "off");
    let chosen = PanelSettings::from_text("Clean picture\tbw\ton\nClean picture\tchannel\tred\n");
    assert_eq!(chosen.resolve("Clean picture", &picture_view::SETTINGS)["channel"], "red", "a chosen channel wins");
    let stale = PanelSettings::from_text("Clean picture\tchannel\tpurple\n");
    assert_eq!(stale.resolve("Clean picture", &picture_view::SETTINGS)["channel"], "off", "no longer offered falls back to colour");
}

#[test]
fn the_clean_picture_loupe_state_round_trips_and_rubbish_falls_back_to_the_defaults() {
    use scopecreep::picture_view;
    let mut s = PanelSettings::default();
    s.set("Clean picture", "loupe", "on");
    s.set("Clean picture", "size", "large");
    let back = PanelSettings::from_text(&s.to_text());
    let own = back.resolve("Clean picture", &picture_view::SETTINGS);
    assert_eq!((own["loupe"].as_str(), own["size"].as_str()), ("on", "large"));
    let default = PanelSettings::default().resolve("Clean picture", &picture_view::SETTINGS);
    assert_eq!((default["loupe"].as_str(), default["size"].as_str()), ("off", "medium"));
    let rubbish = PanelSettings::from_text("Clean picture\tloupe\tmaybe\nClean picture\tsize\thuge\n").resolve("Clean picture", &picture_view::SETTINGS);
    assert_eq!((rubbish["loupe"].as_str(), rubbish["size"].as_str()), ("off", "medium"));
}
