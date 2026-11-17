//! Panel settings and the right-click menu, on the pure seam (#51): no GPU, no window.
use scopecreep::settings::{Choice, Menu, MenuRow, PanelSettings, SettingDef, MENU_ROW};

const DEFS: [SettingDef; 2] = [
    SettingDef { key: "weights", label: "Weights", choices: &[Choice { value: "a", text: "Alpha" }, Choice { value: "b", text: "Beta" }] },
    SettingDef { key: "spread", label: "Spread", choices: &[Choice { value: "on", text: "On" }, Choice { value: "off", text: "Off" }] },
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
