//! The Hotkeys panel through its pure description and the command seam (#148, spec #142): capture, cancel, clear,
//! conflicts (replace and cancel), reserved keys, reset one and all, the menus following a change at once, and the
//! change surviving a restart.
use scopecreep::commands::{Command, Hotkey, Hotkeys, Key};
use scopecreep::components::{MenuEntry, MenuItem};
use scopecreep::hotkeys_panel::{HotkeysPanel, PRESS_A_KEY, PanelAction, State};
use scopecreep::menu_bar::{self, LayoutMode, MenuAction, SETTINGS};

fn key(name: &str) -> Key {
    Key::named(name).unwrap()
}

fn plain(name: &str) -> Hotkey {
    Hotkey::new(key(name))
}

fn ctrl(name: &str) -> Hotkey {
    Hotkey::ctrl(key(name))
}

fn items(menu: &menu_bar::Menu) -> Vec<(usize, MenuItem)> {
    menu.drop
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            MenuEntry::Item(item) => Some((i, item.clone())),
            _ => None,
        })
        .collect()
}

fn row(menu: &menu_bar::Menu, label: &str) -> (usize, MenuItem) {
    items(menu)
        .into_iter()
        .find(|(_, it)| it.label == label)
        .unwrap_or_else(|| panic!("no row {label}"))
}

fn headings(menu: &menu_bar::Menu) -> Vec<String> {
    menu.drop
        .entries
        .iter()
        .filter_map(|e| match e {
            MenuEntry::Heading(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// Click the row called `label` the way the window does: through the menu's action.
fn click(panel: &mut HotkeysPanel, hotkeys: &mut Hotkeys, label: &str) -> bool {
    let menu = panel.menu(hotkeys);
    let (i, _) = row(&menu, label);
    match menu.action(i) {
        Some(MenuAction::Hotkeys(action)) => panel.click(action, hotkeys),
        other => panic!("{label} does {other:?}"),
    }
}

#[test]
fn settings_opens_the_hotkeys_panel() {
    assert_eq!(menu_bar::bar().titles[SETTINGS], "Settings");
    let menu = menu_bar::settings_menu();
    let (i, _) = row(&menu, "Hotkeys…");
    assert_eq!(menu.action(i), Some(MenuAction::OpenHotkeys));
}

#[test]
fn the_panel_lists_every_command_with_its_key() {
    let panel = HotkeysPanel::default();
    let menu = panel.menu(&Hotkeys::default());
    for command in Command::all() {
        let (i, it) = row(&menu, &command.label());
        assert_eq!(
            it.hotkey,
            command
                .default_hotkey()
                .map(|h| h.to_string())
                .unwrap_or_default()
        );
        assert_eq!(
            menu.action(i),
            Some(MenuAction::Hotkeys(PanelAction::Capture(command)))
        );
    }
    let (_, reset_all) = row(&menu, "Reset all to defaults");
    assert!(!reset_all.enabled, "nothing to reset yet");
}

#[test]
fn choosing_a_command_starts_capture_and_the_next_combination_is_assigned() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    assert!(!click(&mut panel, &mut hotkeys, "Go to page 1"));
    assert!(panel.takes_keys());
    let (_, capturing) = row(&panel.menu(&hotkeys), "Go to page 1");
    assert_eq!(capturing.hotkey, PRESS_A_KEY);
    assert!(capturing.on, "the field being captured is marked");
    assert!(panel.key(plain("F5"), &mut hotkeys));
    assert_eq!(panel.state, State::Idle);
    assert_eq!(hotkeys.get(Command::GoToPage(0)), Some(plain("F5")));
    assert_eq!(row(&panel.menu(&hotkeys), "Go to page 1").1.hotkey, "F5");
}

#[test]
fn escape_cancels_and_changes_nothing() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    click(&mut panel, &mut hotkeys, "Go to page 1");
    assert!(!panel.key(plain("Escape"), &mut hotkeys));
    assert_eq!(panel.state, State::Idle);
    assert_eq!(hotkeys, Hotkeys::default());
}

#[test]
fn backspace_or_delete_clears() {
    for clear in ["Backspace", "Delete"] {
        let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
        click(&mut panel, &mut hotkeys, "Go to page 2");
        assert!(panel.key(plain(clear), &mut hotkeys));
        assert_eq!(hotkeys.get(Command::GoToPage(1)), None);
        assert_eq!(row(&panel.menu(&hotkeys), "Go to page 2").1.hotkey, "");
    }
}

#[test]
fn a_conflict_names_the_other_command_and_cancel_keeps_both() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    click(&mut panel, &mut hotkeys, "Go to page 1");
    assert!(!panel.key(ctrl("2"), &mut hotkeys));
    let menu = panel.menu(&hotkeys);
    assert!(headings(&menu).contains(&"Ctrl+2 is used by Go to page 2".to_string()));
    assert!(panel.takes_keys(), "a key now must not run Go to page 2");
    assert!(!click(&mut panel, &mut hotkeys, "Cancel"));
    assert_eq!(hotkeys, Hotkeys::default());
    assert_eq!(panel.state, State::Idle);
}

#[test]
fn replace_takes_the_key_from_the_other_command() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    click(&mut panel, &mut hotkeys, "Go to page 1");
    panel.key(ctrl("2"), &mut hotkeys);
    assert!(click(&mut panel, &mut hotkeys, "Replace"));
    assert_eq!(hotkeys.command_for(ctrl("2")), Some(Command::GoToPage(0)));
    assert_eq!(hotkeys.get(Command::GoToPage(1)), None);
    let menu = panel.menu(&hotkeys);
    assert_eq!(row(&menu, "Go to page 1").1.hotkey, "Ctrl+2");
    assert_eq!(row(&menu, "Go to page 2").1.hotkey, "");
}

#[test]
fn escape_also_cancels_a_conflict() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    click(&mut panel, &mut hotkeys, "Go to page 1");
    panel.key(ctrl("2"), &mut hotkeys);
    panel.key(plain("Escape"), &mut hotkeys);
    assert_eq!(panel.state, State::Idle);
    assert_eq!(hotkeys, Hotkeys::default());
}

#[test]
fn a_reserved_key_is_refused_with_a_line_and_capture_goes_on() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    click(&mut panel, &mut hotkeys, "Default layout");
    assert!(!panel.key(plain("Enter"), &mut hotkeys));
    assert!(panel.takes_keys());
    assert!(
        headings(&panel.menu(&hotkeys))
            .contains(&"Escape, Enter, Backspace and Delete are kept by the app".to_string())
    );
    assert!(panel.key(ctrl("D"), &mut hotkeys));
    assert_eq!(hotkeys.get(Command::DefaultLayout), Some(ctrl("D")));
}

#[test]
fn pressing_the_key_a_command_already_has_changes_nothing() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    click(&mut panel, &mut hotkeys, "Go to page 3");
    assert!(!panel.key(ctrl("3"), &mut hotkeys));
    assert_eq!(panel.state, State::Idle);
}

#[test]
fn clicking_the_field_being_captured_stops_capturing() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    click(&mut panel, &mut hotkeys, "Go to page 1");
    click(&mut panel, &mut hotkeys, "Go to page 1");
    assert_eq!(panel.state, State::Idle);
    click(&mut panel, &mut hotkeys, "Go to page 1");
    click(&mut panel, &mut hotkeys, "Go to page 4");
    assert_eq!(
        panel.state,
        State::Capturing {
            command: Command::GoToPage(3),
            refused: false
        },
        "another field takes over"
    );
}

#[test]
fn reset_one_puts_the_captured_command_back() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    hotkeys.clear(Command::GoToPage(0));
    click(&mut panel, &mut hotkeys, "Go to page 1");
    assert!(click(&mut panel, &mut hotkeys, "Reset to default (Ctrl+1)"));
    assert_eq!(hotkeys, Hotkeys::default());
    assert_eq!(panel.state, State::Idle);
    // At its default, reset is greyed.
    click(&mut panel, &mut hotkeys, "Go to page 1");
    let (i, reset) = row(&panel.menu(&hotkeys), "Reset to default (Ctrl+1)");
    assert!(!reset.enabled);
    assert_eq!(panel.menu(&hotkeys).action(i), None);
}

#[test]
fn reset_all_puts_every_command_back() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    hotkeys.clear(Command::GoToPage(0));
    hotkeys.replace(Command::DuplicatePage, ctrl("9")).unwrap();
    assert!(click(&mut panel, &mut hotkeys, "Reset all to defaults"));
    assert_eq!(hotkeys, Hotkeys::default());
}

#[test]
fn a_changed_hotkey_shows_in_the_layout_menu_at_once() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    let names = vec!["Main".to_string(), "Pictures".to_string()];
    click(&mut panel, &mut hotkeys, "Go to page 1");
    panel.key(plain("F7"), &mut hotkeys);
    let menu = menu_bar::layout_menu(&names, 0, &hotkeys, LayoutMode::Show);
    let (i, it) = row(&menu, "1  Main");
    assert_eq!(it.hotkey, "F7");
    assert_eq!(
        menu.action(i),
        Some(MenuAction::Page(0)),
        "the entry still works"
    );
}

#[test]
fn a_command_with_no_hotkey_still_works_from_its_menu() {
    let mut hotkeys = Hotkeys::default();
    hotkeys.clear(Command::DefaultLayout);
    hotkeys.clear(Command::GoToPage(0));
    let names = vec!["Main".to_string()];
    let menu = menu_bar::layout_menu(&names, 0, &hotkeys, LayoutMode::Show);
    let (i, it) = row(&menu, "Default layout");
    assert_eq!(it.hotkey, "");
    assert_eq!(
        menu.action(i),
        Some(MenuAction::Run(Command::DefaultLayout))
    );
    let (i, it) = row(&menu, "1  Main");
    assert_eq!(it.hotkey, "");
    assert_eq!(menu.action(i), Some(MenuAction::Page(0)));
}

#[test]
fn a_change_made_in_the_panel_survives_a_restart() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    click(&mut panel, &mut hotkeys, "Go to page 1");
    panel.key(ctrl("2"), &mut hotkeys);
    click(&mut panel, &mut hotkeys, "Replace");
    click(&mut panel, &mut hotkeys, "Default layout");
    panel.key(plain("F12").with_alt(), &mut hotkeys);
    let restarted = Hotkeys::from_text(&hotkeys.to_text());
    assert_eq!(restarted, hotkeys);
    assert_eq!(restarted.command_for(ctrl("2")), Some(Command::GoToPage(0)));
}

#[test]
fn an_idle_panel_takes_no_keys() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    assert!(!panel.takes_keys());
    assert!(!panel.key(ctrl("1"), &mut hotkeys));
    assert_eq!(hotkeys, Hotkeys::default());
}

#[test]
fn the_panel_fits_the_default_window_under_the_bar_at_1x_and_2x() {
    let mut panel = HotkeysPanel::default();
    let mut hotkeys = Hotkeys::default();
    click(&mut panel, &mut hotkeys, "Go to page 1");
    panel.key(ctrl("2"), &mut hotkeys);
    for scale in [1.0, 2.0] {
        let window = [1280.0 * scale, 760.0 * scale];
        let title = menu_bar::bar().title_rects(0.0, 0.0, scale)[SETTINGS];
        let r = panel.menu(&hotkeys).drop.rect(title, window, scale);
        assert!(r[1] >= title[3] - 0.01, "under the bar");
        assert!(r[1] + r[3] <= window[1] + 0.01 && r[0] + r[2] <= window[0] + 0.01);
    }
}

#[test]
fn the_fit_toggle_is_listed_with_z_and_can_be_reassigned_there() {
    let (mut panel, mut hotkeys) = (HotkeysPanel::default(), Hotkeys::default());
    assert_eq!(
        row(&panel.menu(&hotkeys), "Zoom: Fit / last zoom").1.hotkey,
        "Z"
    );
    click(&mut panel, &mut hotkeys, "Zoom: Fit / last zoom");
    assert!(panel.key(plain("F"), &mut hotkeys));
    assert_eq!(hotkeys.command_for(plain("F")), Some(Command::ZoomFit));
}
