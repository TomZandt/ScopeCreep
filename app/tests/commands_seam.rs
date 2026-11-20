//! Commands and hotkeys at the pure seam (#146, ADR 0036): the defaults are the layout shortcuts as they were,
//! assigning, clearing and resetting, conflicts and reserved keys, and the hotkeys file.
use scopecreep::commands::{Command, Hotkey, Hotkeys, Key, Refused};
use winit::keyboard::KeyCode;

fn key(name: &str) -> Key {
    Key::named(name).unwrap()
}

fn ctrl(name: &str) -> Hotkey {
    Hotkey::ctrl(key(name))
}

fn ctrl_shift(name: &str) -> Hotkey {
    ctrl(name).with_shift()
}

#[test]
fn ctrl_digits_go_to_a_page_and_ctrl_shift_digits_do_nothing_any_more() {
    let hotkeys = Hotkeys::default();
    assert_eq!(hotkeys.command_for(ctrl("1")), Some(Command::GoToPage(0)));
    assert_eq!(hotkeys.command_for(ctrl("9")), Some(Command::GoToPage(8)));
    assert_eq!(
        hotkeys.command_for(ctrl_shift("3")),
        None,
        "the save-into-slot Commands are gone"
    );
}

#[test]
fn other_keys_do_nothing_by_default() {
    let hotkeys = Hotkeys::default();
    assert_eq!(
        hotkeys.command_for(Hotkey::new(key("1"))),
        None,
        "a digit alone"
    );
    assert_eq!(
        hotkeys.command_for(Hotkey::new(key("1")).with_shift()),
        None
    );
    assert_eq!(hotkeys.command_for(ctrl("0")), None, "there is no slot 0");
    assert_eq!(
        hotkeys.command_for(ctrl("1").with_alt()),
        None,
        "Alt is another hotkey"
    );
    assert_eq!(hotkeys.get(Command::DefaultLayout), None);
}

#[test]
fn every_page_up_to_nine_has_a_go_to_command_and_the_save_commands_are_gone() {
    let all = Command::all();
    assert_eq!(
        all.len(),
        1 + 9 + 3 + 8,
        "Default layout, go to each of nine pages, new, duplicate and delete page, auto layout, auto resize, the Fit toggle, fill the Window, the legends, add and close a Panel, undo a layout change"
    );
    for page in 0..9 {
        assert!(all.contains(&Command::GoToPage(page)));
    }
    assert_eq!(Command::GoToPage(3).label(), "Go to page 4");
    assert!(
        all.iter()
            .all(|c| !c.id().starts_with("layout-save") && !c.id().starts_with("layout-recall")),
        "no slot commands"
    );
    for command in [
        Command::NewPage,
        Command::DuplicatePage,
        Command::DeletePage,
    ] {
        assert!(all.contains(&command));
        assert_eq!(Hotkeys::default().get(command), None, "no default hotkey");
    }
}

#[test]
fn a_saved_hotkey_line_that_names_a_removed_command_is_skipped() {
    let hotkeys =
        Hotkeys::from_text("hotkeys 1\nlayout-save-1 F2\nlayout-recall-2 F3\npage-go-3 F4\n");
    assert_eq!(hotkeys.command_for(Hotkey::new(key("F2"))), None);
    assert_eq!(hotkeys.command_for(Hotkey::new(key("F3"))), None);
    assert_eq!(
        hotkeys.get(Command::GoToPage(1)),
        Some(ctrl("2")),
        "the line for the removed command changed nothing"
    );
    assert_eq!(
        hotkeys.get(Command::GoToPage(2)),
        Some(Hotkey::new(key("F4"))),
        "a line for a command that exists still applies"
    );
}

#[test]
fn keys_are_by_position_so_shift_does_not_change_the_digit() {
    assert_eq!(Key::from_code(KeyCode::Digit1), Some(key("1")));
    assert_eq!(Key::from_code(KeyCode::KeyZ), Some(key("Z")));
    assert_eq!(Key::from_code(KeyCode::ShiftLeft), None, "a modifier alone");
}

#[test]
fn a_hotkey_is_shown_as_desktop_menus_show_it() {
    assert_eq!(ctrl_shift("1").to_string(), "Ctrl+Shift+1");
    assert_eq!(Hotkey::new(key("Z")).to_string(), "Z");
    assert_eq!(ctrl("Minus").to_string(), "Ctrl+-");
    assert_eq!(Hotkey::new(key("PageUp")).to_string(), "Page Up");
}

#[test]
fn assigning_moves_the_command_to_the_new_hotkey() {
    let mut hotkeys = Hotkeys::default();
    let f5 = Hotkey::new(key("F5"));
    assert_eq!(hotkeys.assign(Command::GoToPage(0), f5), Ok(()));
    assert_eq!(hotkeys.get(Command::GoToPage(0)), Some(f5));
    assert_eq!(hotkeys.command_for(f5), Some(Command::GoToPage(0)));
    assert_eq!(
        hotkeys.command_for(ctrl("1")),
        None,
        "the old hotkey is free"
    );
}

#[test]
fn a_command_with_no_hotkey_can_be_given_one() {
    let mut hotkeys = Hotkeys::default();
    let d = ctrl("D");
    assert_eq!(hotkeys.assign(Command::DefaultLayout, d), Ok(()));
    assert_eq!(hotkeys.command_for(d), Some(Command::DefaultLayout));
}

#[test]
fn a_conflict_names_the_other_command_and_cancel_changes_nothing() {
    let mut hotkeys = Hotkeys::default();
    let before = hotkeys.clone();
    assert_eq!(
        hotkeys.assign(Command::GoToPage(0), ctrl("2")),
        Err(Refused::Conflict(Command::GoToPage(1)))
    );
    assert_eq!(hotkeys, before, "cancelling leaves every hotkey as it was");
}

#[test]
fn replace_takes_the_hotkey_from_the_other_command() {
    let mut hotkeys = Hotkeys::default();
    assert_eq!(hotkeys.replace(Command::GoToPage(0), ctrl("2")), Ok(()));
    assert_eq!(hotkeys.command_for(ctrl("2")), Some(Command::GoToPage(0)));
    assert_eq!(hotkeys.get(Command::GoToPage(1)), None);
}

#[test]
fn reassigning_the_same_hotkey_is_not_a_conflict() {
    let mut hotkeys = Hotkeys::default();
    assert_eq!(hotkeys.assign(Command::GoToPage(0), ctrl("1")), Ok(()));
    assert_eq!(hotkeys, Hotkeys::default());
}

#[test]
fn reserved_keys_are_refused() {
    let mut hotkeys = Hotkeys::default();
    for hotkey in [
        Hotkey::new(key("Escape")),
        ctrl("Escape"),
        Hotkey::new(key("Enter")),
        Hotkey::new(key("Backspace")),
        Hotkey::new(key("Delete")),
    ] {
        assert_eq!(
            hotkeys.assign(Command::DefaultLayout, hotkey),
            Err(Refused::Reserved),
            "{hotkey}"
        );
        assert_eq!(
            hotkeys.replace(Command::DefaultLayout, hotkey),
            Err(Refused::Reserved)
        );
    }
    assert_eq!(
        hotkeys.assign(Command::DefaultLayout, ctrl("Enter")),
        Ok(()),
        "Enter with a modifier is free"
    );
}

#[test]
fn clearing_leaves_the_command_without_a_hotkey() {
    let mut hotkeys = Hotkeys::default();
    hotkeys.clear(Command::GoToPage(4));
    assert_eq!(hotkeys.get(Command::GoToPage(4)), None);
    assert_eq!(hotkeys.command_for(ctrl("5")), None);
}

#[test]
fn reset_puts_one_command_back_and_takes_its_default_from_whoever_had_it() {
    let mut hotkeys = Hotkeys::default();
    hotkeys
        .assign(Command::GoToPage(0), Hotkey::new(key("F1")))
        .unwrap();
    hotkeys.assign(Command::DefaultLayout, ctrl("1")).unwrap();
    hotkeys.reset(Command::GoToPage(0));
    assert_eq!(hotkeys.command_for(ctrl("1")), Some(Command::GoToPage(0)));
    assert_eq!(
        hotkeys.get(Command::DefaultLayout),
        None,
        "no two commands share a hotkey"
    );
}

#[test]
fn reset_all_puts_every_command_back() {
    let mut hotkeys = Hotkeys::default();
    hotkeys.clear(Command::GoToPage(0));
    hotkeys.replace(Command::DuplicatePage, ctrl("2")).unwrap();
    hotkeys.reset_all();
    assert_eq!(hotkeys, Hotkeys::default());
}

#[test]
fn the_file_round_trips() {
    let mut hotkeys = Hotkeys::default();
    hotkeys.clear(Command::GoToPage(2));
    hotkeys
        .replace(Command::DefaultLayout, ctrl_shift("9").with_alt())
        .unwrap();
    hotkeys
        .assign(Command::GoToPage(0), Hotkey::new(key("Slash")))
        .unwrap();
    assert_eq!(Hotkeys::from_text(&hotkeys.to_text()), hotkeys);
    assert_eq!(
        Hotkeys::from_text(&Hotkeys::default().to_text()),
        Hotkeys::default()
    );
}

#[test]
fn a_damaged_line_falls_back_to_that_commands_default() {
    let text = "hotkeys 1\n\
                page-go-1 F2\n\
                page-go-2 Ctrl+Banana\n\
                page-go-3\n\
                no-such-command F3\n\
                page-go-4 Escape\n\
                page-go-5 none\n";
    let hotkeys = Hotkeys::from_text(text);
    assert_eq!(
        hotkeys.get(Command::GoToPage(0)),
        Some(Hotkey::new(key("F2")))
    );
    assert_eq!(hotkeys.get(Command::GoToPage(1)), Some(ctrl("2")));
    assert_eq!(hotkeys.get(Command::GoToPage(2)), Some(ctrl("3")));
    assert_eq!(
        hotkeys.get(Command::GoToPage(3)),
        Some(ctrl("4")),
        "reserved"
    );
    assert_eq!(hotkeys.get(Command::GoToPage(4)), None);
}

#[test]
fn a_file_of_another_version_or_rubbish_gives_the_defaults() {
    assert_eq!(
        Hotkeys::from_text("hotkeys 2\npage-go-1 F2\n"),
        Hotkeys::default()
    );
    assert_eq!(Hotkeys::from_text("\u{0}\u{1}garbage"), Hotkeys::default());
    assert_eq!(Hotkeys::from_text(""), Hotkeys::default());
}

#[test]
fn a_hand_edited_file_that_shares_a_hotkey_keeps_the_defaults_for_the_clash() {
    // Go to page 1 is given Go to page 2's hotkey, which keeps it: the edit is dropped, not the default.
    let hotkeys = Hotkeys::from_text("hotkeys 1\npage-go-1 Ctrl+2\n");
    assert_eq!(hotkeys.command_for(ctrl("2")), Some(Command::GoToPage(1)));
    assert_eq!(hotkeys.get(Command::GoToPage(0)), Some(ctrl("1")));
    // Two edits on one hotkey: neither command takes it.
    let hotkeys = Hotkeys::from_text("hotkeys 1\npage-go-1 F4\npage-go-2 F4\n");
    assert_eq!(hotkeys, Hotkeys::default());
}

#[test]
fn a_saved_replace_reads_back_whichever_order_the_lines_are_in() {
    let mut hotkeys = Hotkeys::default();
    hotkeys.replace(Command::DuplicatePage, ctrl("1")).unwrap();
    let text = hotkeys.to_text();
    let mut lines: Vec<&str> = text.lines().collect();
    lines[1..].reverse();
    assert_eq!(Hotkeys::from_text(&lines.join("\n")), hotkeys);
}

#[test]
fn z_is_the_fit_toggle_by_default_and_can_be_changed_like_any_hotkey() {
    let mut hotkeys = Hotkeys::default();
    assert_eq!(
        hotkeys.command_for(Hotkey::new(key("Z"))),
        Some(Command::ZoomFit)
    );
    assert_eq!(Command::ZoomFit.label(), "Zoom: Fit / last zoom");
    hotkeys
        .assign(Command::ZoomFit, Hotkey::new(key("Z")).with_shift())
        .unwrap();
    assert_eq!(hotkeys.command_for(Hotkey::new(key("Z"))), None);
    hotkeys.clear(Command::ZoomFit);
    assert_eq!(hotkeys.get(Command::ZoomFit), None);
    hotkeys.reset(Command::ZoomFit);
    assert_eq!(hotkeys, Hotkeys::default());
    let mut saved = Hotkeys::default();
    saved.assign(Command::ZoomFit, ctrl("Y")).unwrap();
    assert_eq!(Hotkeys::from_text(&saved.to_text()), saved);
}

#[test]
fn ctrl_f_fills_the_window_by_default_and_can_be_changed_like_any_hotkey() {
    let mut hotkeys = Hotkeys::default();
    assert_eq!(hotkeys.command_for(ctrl("F")), Some(Command::FillWindow));
    assert_eq!(Command::FillWindow.label(), "Panel: fill the Window");
    assert_eq!(
        hotkeys.command_for(Hotkey::new(key("F"))),
        None,
        "F alone is not it"
    );
    hotkeys
        .assign(Command::FillWindow, Hotkey::new(key("F11")))
        .unwrap();
    assert_eq!(hotkeys.command_for(ctrl("F")), None);
    hotkeys.clear(Command::FillWindow);
    assert_eq!(hotkeys.get(Command::FillWindow), None);
    hotkeys.reset(Command::FillWindow);
    assert_eq!(hotkeys, Hotkeys::default());
    let mut saved = Hotkeys::default();
    saved.assign(Command::FillWindow, ctrl("Enter")).unwrap();
    assert_eq!(Hotkeys::from_text(&saved.to_text()), saved);
}

#[test]
fn the_legends_command_has_no_hotkey_until_one_is_given_and_it_round_trips() {
    let mut hotkeys = Hotkeys::default();
    assert_eq!(Command::ToggleLegends.label(), "Legends: show / hide");
    assert_eq!(hotkeys.get(Command::ToggleLegends), None);
    assert!(Command::all().contains(&Command::ToggleLegends));
    hotkeys.assign(Command::ToggleLegends, ctrl("L")).unwrap();
    assert_eq!(hotkeys.command_for(ctrl("L")), Some(Command::ToggleLegends));
    let saved = hotkeys.clone();
    assert_eq!(Hotkeys::from_text(&saved.to_text()), saved);
    hotkeys.clear(Command::ToggleLegends);
    assert_eq!(hotkeys.get(Command::ToggleLegends), None);
    hotkeys.assign(Command::ToggleLegends, ctrl("L")).unwrap();
    hotkeys.reset(Command::ToggleLegends);
    assert_eq!(hotkeys, Hotkeys::default(), "reset puts back no hotkey");
    hotkeys.assign(Command::ToggleLegends, ctrl("L")).unwrap();
    hotkeys.reset_all();
    assert_eq!(hotkeys, Hotkeys::default());
}
