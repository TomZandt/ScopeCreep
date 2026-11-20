//! Add panel and Close at the pure UI seam (#206, ADR 0038): the focus outline, the Close entry in a Panel's
//! popover, the empty page's Add panel button, and the Add panel and Close entries in the Layout menu, at 1x and 2x.
//! The two Commands are in the command table and the Hotkeys panel. No GPU, no window.
use scopecreep::commands::{Command, Hotkeys};
use scopecreep::components::*;
use scopecreep::hotkeys_panel::HotkeysPanel;
use scopecreep::menu_bar::{self, AddEntry, LayoutMode, MenuAction, add_menu, layout_menu};
use scopecreep::settings::{Menu, MenuRow, Settings};
use scopecreep::style;

const SCALES: [f32; 2] = [1.0, 2.0];

fn rects(prims: &[Prim]) -> Vec<([f32; 4], [f32; 4])> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Rect { rect, colour } => Some((*rect, [colour[0], colour[1], colour[2], 1.0])),
            _ => None,
        })
        .collect()
}

fn texts(prims: &[Prim]) -> Vec<String> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_focused_panel_has_a_quiet_outline_just_inside_its_frame() {
    for scale in SCALES {
        let frame = [100.0, 50.0, 400.0, 300.0];
        let prims = focus_outline(frame, scale);
        let edges = rects(&prims);
        assert_eq!(edges.len(), 4, "four thin edges");
        let weight = (style::LINE_WEIGHT * scale).max(1.0);
        for (rect, colour) in &edges {
            assert_eq!(*colour, style::rgba(style::STATE_FOCUS));
            assert!(rect[2] == weight || rect[3] == weight, "{rect:?} is thin");
            // Inside the frame.
            assert!(rect[0] >= frame[0] && rect[1] >= frame[1]);
            assert!(rect[0] + rect[2] <= frame[0] + frame[2]);
            assert!(rect[1] + rect[3] <= frame[1] + frame[3]);
        }
        // Quiet: a neutral grey, not the accent.
        assert_ne!(style::STATE_FOCUS, style::ACCENT);
    }
}

#[test]
fn the_empty_page_has_one_large_add_panel_button_in_the_middle() {
    for scale in SCALES {
        let area = [12.0 * scale, 60.0 * scale, 1256.0 * scale, 716.0 * scale];
        let button = add_panel_button_rect(area, scale);
        let centre = [button[0] + button[2] / 2.0, button[1] + button[3] / 2.0];
        assert!((centre[0] - (area[0] + area[2] / 2.0)).abs() < 1.0);
        assert!((centre[1] - (area[1] + area[3] / 2.0)).abs() < 1.0);
        // Large: bigger than a chip, a comfortable target.
        let chip = Chip::new("Add panel").rect([0.0, 0.0], scale);
        assert!(button[2] > chip[2] && button[3] > chip[3]);
        assert!(button[3] >= 40.0 * scale);
        let prims = add_panel_button(area, State::Normal, scale);
        assert_eq!(texts(&prims), vec!["Add panel".to_string()]);
        // The hit area is the button's rectangle.
        assert!(contains(button, centre));
        assert!(!contains(button, [button[0] - 1.0, centre[1]]));
        // A hover is lighter than normal.
        let fill = |state| rects(&add_panel_button(area, state, scale))[0].1;
        assert_ne!(fill(State::Normal), fill(State::Hover));
    }
}

#[test]
fn the_button_stays_inside_a_small_area() {
    let area = [0.0, 0.0, 120.0, 105.0];
    let button = add_panel_button_rect(area, 1.0);
    assert!(button[0] >= area[0] && button[0] + button[2] <= area[0] + area[2]);
    assert!(button[1] >= area[1] && button[1] + button[3] <= area[1] + area[3]);
}

fn menu_of(defs_empty: bool) -> Menu {
    let defs = if defs_empty {
        &[][..]
    } else {
        &scopecreep::waveform::SETTINGS[..]
    };
    Menu::for_panel(3, defs, &Settings::new())
}

#[test]
fn a_panels_popover_ends_with_a_close_entry_even_with_no_settings() {
    for with_settings in [false, true] {
        let menu = menu_of(with_settings);
        assert_eq!(menu.panel, 3);
        assert_eq!(menu.rows.last(), Some(&MenuRow::Close));
        assert_eq!(
            menu.rows.iter().filter(|r| **r == MenuRow::Close).count(),
            1
        );
        for scale in SCALES {
            let rect = menu.rect([40.0, 40.0], [1000.0, 800.0], scale);
            let row = menu.rows.len() - 1;
            let top = Menu::row_top(rect, row, scale);
            let on_close = [rect[0] + 20.0 * scale, top + 4.0 * scale];
            assert!(menu.close_at(rect, on_close, scale));
            assert!(!menu.close_at(rect, [rect[0] - 2.0, top + 4.0 * scale], scale));
            assert!(!menu.close_at(rect, [on_close[0], top - 4.0 * scale], scale));
            let prims = popover(&menu, rect, [0.0, 0.0], scale);
            assert!(texts(&prims).contains(&"Close".to_string()));
        }
    }
}

#[test]
fn close_is_not_a_setting_choice() {
    let menu = menu_of(false);
    let rect = menu.rect([0.0, 0.0], [1000.0, 800.0], 1.0);
    let top = Menu::row_top(rect, menu.rows.len() - 1, 1.0);
    assert_eq!(menu.choice_at(rect, [rect[0] + 20.0, top + 4.0], 1.0), None);
}

#[test]
fn add_panel_and_close_are_commands_with_no_hotkey_until_one_is_given() {
    assert!(Command::all().contains(&Command::AddPanel));
    assert!(Command::all().contains(&Command::ClosePanel));
    assert_eq!(Command::AddPanel.default_hotkey(), None);
    assert_eq!(Command::ClosePanel.default_hotkey(), None);
    assert_ne!(Command::AddPanel.id(), Command::ClosePanel.id());
    assert!(Command::AddPanel.label().contains("add"));
    assert!(Command::ClosePanel.label().contains("close"));
    let hotkeys = Hotkeys::default();
    assert_eq!(hotkeys.get(Command::AddPanel), None);
}

#[test]
fn the_hotkeys_panel_lists_add_panel_and_close() {
    let menu = HotkeysPanel::default().menu(&Hotkeys::default());
    let labels: Vec<String> = menu
        .drop
        .entries
        .iter()
        .filter_map(|e| match e {
            MenuEntry::Item(item) => Some(item.label.clone()),
            _ => None,
        })
        .collect();
    for command in [Command::AddPanel, Command::ClosePanel] {
        assert!(labels.contains(&command.label()), "{labels:?}");
    }
}

fn labels(menu: &menu_bar::Menu) -> Vec<(String, bool)> {
    menu.drop
        .entries
        .iter()
        .filter_map(|e| match e {
            MenuEntry::Item(item) => Some((item.label.clone(), item.enabled)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_layout_menu_has_add_panel_and_close_that_run_their_commands() {
    let hotkeys = Hotkeys::default();
    let menu = layout_menu(&["Main".to_string()], 0, &hotkeys, LayoutMode::Show);
    let find = |label: &str| {
        menu.drop
            .entries
            .iter()
            .position(|e| matches!(e, MenuEntry::Item(i) if i.label == label))
            .unwrap_or_else(|| panic!("no entry {label}: {:?}", labels(&menu)))
    };
    let add = find("Add panel…");
    let close = find("Close panel");
    assert_eq!(menu.action(add), Some(MenuAction::Run(Command::AddPanel)));
    assert_eq!(
        menu.action(close),
        Some(MenuAction::Run(Command::ClosePanel))
    );
}

#[test]
fn a_hotkey_given_to_add_panel_shows_beside_its_entry() {
    let mut hotkeys = Hotkeys::default();
    let key = scopecreep::commands::Key::named("N").unwrap();
    assert!(
        hotkeys
            .assign(Command::AddPanel, scopecreep::commands::Hotkey::ctrl(key))
            .is_ok()
    );
    let menu = layout_menu(&["Main".to_string()], 0, &hotkeys, LayoutMode::Show);
    let shown = menu.drop.entries.iter().find_map(|e| match e {
        MenuEntry::Item(i) if i.label == "Add panel…" => Some(i.hotkey.clone()),
        _ => None,
    });
    assert_eq!(shown.as_deref(), Some("Ctrl+N"));
}

#[test]
fn the_add_list_offers_each_display_and_greys_the_ones_already_on_the_page() {
    let entries = vec![
        AddEntry {
            panel: 0,
            name: "Channel Viewer".into(),
            enabled: false,
        },
        AddEntry {
            panel: 1,
            name: "Waveform".into(),
            enabled: true,
        },
    ];
    let menu = add_menu(&entries);
    assert_eq!(
        labels(&menu),
        vec![
            ("Channel Viewer".to_string(), false),
            ("Waveform".to_string(), true),
            ("Back".to_string(), true),
        ]
    );
    let first = menu
        .drop
        .entries
        .iter()
        .position(|e| matches!(e, MenuEntry::Item(i) if i.label == "Waveform"))
        .unwrap();
    assert_eq!(menu.action(first), Some(MenuAction::Add(1)));
    let greyed = menu
        .drop
        .entries
        .iter()
        .position(|e| matches!(e, MenuEntry::Item(i) if i.label == "Channel Viewer"))
        .unwrap();
    assert_eq!(menu.action(greyed), None);
    let back = menu.drop.entries.len() - 1;
    assert_eq!(menu.action(back), Some(MenuAction::Mode(LayoutMode::Show)));
}

#[test]
fn the_add_list_fits_the_menu_sizes_at_1x_and_2x() {
    // The Window's minimum size counts the Add list too (#172).
    let entries: Vec<AddEntry> = scopecreep::layout_tree::default_names()
        .iter()
        .enumerate()
        .map(|(panel, name)| AddEntry {
            panel,
            name: name.to_string(),
            enabled: true,
        })
        .collect();
    assert_eq!(entries.len(), 7);
    let menu = add_menu(&entries);
    for scale in SCALES {
        let [w, h] = menu.drop.size(scale);
        assert!(w > 0.0 && h > 0.0);
    }
    assert!(menu_bar::largest_menus().len() > 5);
}

// ---- The Panel section: Replace with, Duplicate right and below, Close (#208, ADR 0039) ----

#[test]
fn the_panel_section_lists_replace_with_duplicate_right_duplicate_below_then_close() {
    use scopecreep::settings::PanelAction;
    for with_settings in [false, true] {
        let menu = menu_of(with_settings);
        let n = menu.rows.len();
        assert_eq!(menu.rows[n - 5], MenuRow::Heading("Panel".to_string()));
        assert_eq!(menu.rows[n - 4], MenuRow::Panel(PanelAction::ReplaceWith));
        assert_eq!(
            menu.rows[n - 3],
            MenuRow::Panel(PanelAction::DuplicateRight)
        );
        assert_eq!(
            menu.rows[n - 2],
            MenuRow::Panel(PanelAction::DuplicateBelow)
        );
        assert_eq!(menu.rows[n - 1], MenuRow::Close);
        for scale in SCALES {
            let rect = menu.rect([40.0, 40.0], [1000.0, 800.0], scale);
            for (row, action) in [
                (n - 4, PanelAction::ReplaceWith),
                (n - 3, PanelAction::DuplicateRight),
                (n - 2, PanelAction::DuplicateBelow),
            ] {
                let top = Menu::row_top(rect, row, scale);
                let on = [rect[0] + 20.0 * scale, top + 4.0 * scale];
                assert_eq!(menu.panel_action_at(rect, on, scale), Some(action));
                assert!(!menu.close_at(rect, on, scale));
                assert_eq!(menu.choice_at(rect, on, scale), None);
            }
            let heading = Menu::row_top(rect, n - 5, scale) + 4.0 * scale;
            assert_eq!(
                menu.panel_action_at(rect, [rect[0] + 20.0 * scale, heading], scale),
                None
            );
            let said = texts(&popover(&menu, rect, [0.0, 0.0], scale));
            for text in [
                "Replace with\u{2026}",
                "Duplicate right",
                "Duplicate below",
                "Close",
            ] {
                assert!(said.contains(&text.to_string()), "{text}");
            }
        }
    }
}

#[test]
fn the_replace_list_names_each_display_then_goes_back() {
    use scopecreep::settings::PanelAction;
    let names = ["Vectorscope".to_string(), "Skin Tone".to_string()];
    let menu = Menu::replace_list(3, &names);
    assert_eq!(menu.panel, 3);
    assert_eq!(
        menu.rows,
        vec![
            MenuRow::Heading("Replace with".to_string()),
            MenuRow::Panel(PanelAction::ReplaceBy("Vectorscope".to_string())),
            MenuRow::Panel(PanelAction::ReplaceBy("Skin Tone".to_string())),
            MenuRow::Panel(PanelAction::Back),
        ]
    );
    for scale in SCALES {
        let rect = menu.rect([40.0, 40.0], [1000.0, 800.0], scale);
        let top = Menu::row_top(rect, 2, scale);
        assert_eq!(
            menu.panel_action_at(rect, [rect[0] + 20.0 * scale, top + 4.0 * scale], scale),
            Some(PanelAction::ReplaceBy("Skin Tone".to_string()))
        );
        let said = texts(&popover(&menu, rect, [0.0, 0.0], scale));
        assert!(said.contains(&"Skin Tone".to_string()) && said.contains(&"Back".to_string()));
    }
}
