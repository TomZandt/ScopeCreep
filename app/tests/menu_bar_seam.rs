//! The menu bar at the pure UI seam (#147, spec #142): the bar's and a menu's size and hit area at 1x and 2x,
//! their states, opening, switching and closing, the Layout menu's entries (empty, in use, current) and that each
//! entry runs the command its shown hotkey runs, and the menus on a small Window.
use scopecreep::commands::{Command, Hotkey, Hotkeys, Key};
use scopecreep::components::*;
use scopecreep::layouts::{Layouts, Rename, SLOTS};
use scopecreep::menu_bar::{self, BarState, Current, LAYOUT, LayoutMode, MenuAction, layout_menu};
use scopecreep::panel_grid::PanelGrid;
use scopecreep::style;

const SCALES: [f32; 2] = [1.0, 2.0];
const NAMES: [&str; 3] = ["Clean picture", "Luma waveform", "Vectorscope"];

fn fills(prims: &[Prim]) -> Vec<[f32; 4]> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Rect { colour, .. } => Some(*colour),
            _ => None,
        })
        .collect()
}

fn text_colours(prims: &[Prim]) -> Vec<(String, [u8; 3])> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Text { text, colour, .. } => Some((text.clone(), *colour)),
            _ => None,
        })
        .collect()
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

fn item(menu: &menu_bar::Menu, label: &str) -> (usize, MenuItem) {
    items(menu)
        .into_iter()
        .find(|(_, it)| it.label == label)
        .unwrap_or_else(|| panic!("no entry {label}"))
}

fn arranged() -> PanelGrid {
    let mut grid = PanelGrid::default_for(&NAMES);
    grid.swap(0, 1);
    grid
}

fn centre(r: [f32; 4]) -> [f32; 2] {
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

// ---- The bar ----

#[test]
fn the_bar_is_thin_and_its_titles_are_hit_where_drawn() {
    let bar = menu_bar::bar();
    for scale in SCALES {
        assert_eq!(MenuBar::height(scale), style::SPACE_MENU_BAR * scale);
        assert!(
            MenuBar::height(scale) < BarKind::Top.height(scale),
            "thinner than the top bar"
        );
        let rects = bar.title_rects(0.0, 0.0, scale);
        assert_eq!(rects.len(), bar.titles.len());
        for (i, r) in rects.iter().enumerate() {
            assert_eq!(r[1], 0.0);
            assert_eq!(r[3], MenuBar::height(scale));
            assert_eq!(bar.hit(0.0, 0.0, centre(*r), scale), Some(i));
        }
        assert_eq!(
            bar.hit(
                0.0,
                0.0,
                [rects[0][0] + 1.0, MenuBar::height(scale) + 1.0],
                scale
            ),
            None,
            "below the bar"
        );
    }
}

#[test]
fn bar_titles_sit_side_by_side_and_scale_with_the_display() {
    let bar = MenuBar {
        titles: vec!["Layout".to_string(), "Settings".to_string()],
    };
    let one = bar.title_rects(0.0, 0.0, 1.0);
    let two = bar.title_rects(0.0, 0.0, 2.0);
    assert_eq!(one[0][0] + one[0][2], one[1][0], "no gap, no overlap");
    for (a, b) in one.iter().zip(&two) {
        for k in 0..4 {
            assert!((a[k] * 2.0 - b[k]).abs() < 1e-3);
        }
    }
}

#[test]
fn the_open_title_is_pressed_and_the_one_under_the_pointer_lit() {
    let bar = menu_bar::bar();
    for scale in SCALES {
        let quiet = fills(&bar.draw(0.0, 0.0, 800.0, None, None, scale));
        assert!(!quiet.contains(&style::rgba(style::STATE_HOVER)));
        assert!(!quiet.contains(&style::rgba(style::STATE_PRESSED)));
        let hover = fills(&bar.draw(0.0, 0.0, 800.0, None, Some(0), scale));
        assert!(hover.contains(&style::rgba(style::STATE_HOVER)));
        let open = fills(&bar.draw(0.0, 0.0, 800.0, Some(0), Some(0), scale));
        assert!(open.contains(&style::rgba(style::STATE_PRESSED)));
        assert!(!open.contains(&style::rgba(style::STATE_HOVER)));
    }
}

// ---- Opening, switching and closing ----

#[test]
fn a_click_opens_a_menu_and_a_second_click_closes_it() {
    let mut bar = BarState::default();
    bar.click_title(LAYOUT);
    assert_eq!(bar.open, Some(LAYOUT));
    bar.click_title(LAYOUT);
    assert_eq!(bar.open, None);
}

#[test]
fn with_a_menu_open_moving_along_the_bar_opens_the_next() {
    let mut bar = BarState::default();
    bar.hover_title(1);
    assert_eq!(bar.open, None, "resting on a title opens nothing");
    bar.click_title(0);
    bar.hover_title(1);
    assert_eq!(bar.open, Some(1));
    bar.click_title(0);
    assert_eq!(bar.open, Some(0), "a click on another title opens it");
}

#[test]
fn close_shuts_the_open_menu() {
    let mut bar = BarState::default();
    bar.click_title(0);
    bar.close();
    assert_eq!(bar, BarState::default());
}

// ---- A dropped menu ----

fn sample() -> DropMenu {
    let mut off = MenuItem::new("Rename layout…", "");
    off.enabled = false;
    let mut on = MenuItem::new("1  Grade", "Ctrl+1");
    on.on = true;
    DropMenu {
        entries: vec![
            MenuEntry::Item(MenuItem::new("Default layout", "")),
            MenuEntry::Separator,
            MenuEntry::Item(on),
            MenuEntry::Heading("Save into".to_string()),
            MenuEntry::Item(off),
        ],
    }
}

#[test]
fn a_menu_drops_under_its_title_and_stays_in_the_window() {
    let menu = sample();
    for scale in SCALES {
        let title = [40.0 * scale, 0.0, 60.0 * scale, MenuBar::height(scale)];
        let window = [1280.0 * scale, 760.0 * scale];
        let r = menu.rect(title, window, scale);
        assert_eq!([r[0], r[1]], [title[0], title[3]]);
        assert_eq!([r[2], r[3]], menu.size(scale));
        assert!(r[2] >= style::SPACE_MENU_WIDTH * scale);
        let small = [300.0 * scale, 160.0 * scale];
        let r = menu.rect(
            [180.0 * scale, 0.0, 60.0 * scale, 24.0 * scale],
            small,
            scale,
        );
        assert!(r[0] >= 0.0 && r[0] + r[2] <= small[0] + 0.01);
        assert!(r[1] >= 0.0 && r[1] + r[3] <= small[1] + 0.01);
    }
}

#[test]
fn a_menu_is_rows_and_thin_separators_and_scales_with_the_display() {
    let menu = sample();
    for scale in SCALES {
        let area = menu.rect([0.0, 0.0, 10.0, 10.0], [4000.0, 4000.0], scale);
        let rows = menu.entry_rects(area, scale);
        assert_eq!(rows[0][3], style::SPACE_MENU_ROW * scale);
        assert_eq!(rows[1][3], style::SPACE_MENU_SEPARATOR * scale);
        assert_eq!(rows[0][1], area[1] + style::SPACE_MENU_PAD * scale);
        for pair in rows.windows(2) {
            assert!(
                (pair[0][1] + pair[0][3] - pair[1][1]).abs() < 1e-3,
                "rows touch"
            );
        }
        let last = rows.last().unwrap();
        assert!(
            (last[1] + last[3] + style::SPACE_MENU_PAD * scale - (area[1] + area[3])).abs() < 1e-3
        );
    }
}

#[test]
fn only_enabled_items_are_hit() {
    let menu = sample();
    for scale in SCALES {
        let area = menu.rect([0.0, 0.0, 10.0, 10.0], [4000.0, 4000.0], scale);
        let rows = menu.entry_rects(area, scale);
        assert_eq!(menu.hit(area, centre(rows[0]), scale), Some(0));
        assert_eq!(menu.hit(area, centre(rows[1]), scale), None, "separator");
        assert_eq!(menu.hit(area, centre(rows[2]), scale), Some(2));
        assert_eq!(menu.hit(area, centre(rows[3]), scale), None, "heading");
        assert_eq!(menu.hit(area, centre(rows[4]), scale), None, "disabled");
        assert_eq!(
            menu.hit(area, [area[0] - 1.0, rows[0][1] + 1.0], scale),
            None,
            "left of the menu"
        );
    }
}

#[test]
fn an_item_lights_on_hover_darkens_on_press_and_a_disabled_one_is_grey_and_never_lights() {
    let menu = sample();
    for scale in SCALES {
        let area = menu.rect([0.0, 0.0, 10.0, 10.0], [4000.0, 4000.0], scale);
        let quiet = fills(&menu.draw(area, None, false, scale));
        assert!(!quiet.contains(&style::rgba(style::STATE_HOVER)));
        assert!(
            fills(&menu.draw(area, Some(0), false, scale))
                .contains(&style::rgba(style::STATE_HOVER))
        );
        assert!(
            fills(&menu.draw(area, Some(0), true, scale))
                .contains(&style::rgba(style::STATE_PRESSED))
        );
        let disabled = menu.draw(area, Some(4), false, scale);
        assert!(!fills(&disabled).contains(&style::rgba(style::STATE_HOVER)));
        let texts = text_colours(&disabled);
        assert!(texts.contains(&("Rename layout…".to_string(), style::TEXT_DISABLED)));
        assert!(texts.contains(&("Default layout".to_string(), style::TEXT_PRIMARY)));
        assert!(
            texts.contains(&("Ctrl+1".to_string(), style::TEXT_SECONDARY)),
            "the hotkey beside its entry, quieter"
        );
        assert!(
            quiet.contains(&style::rgba(style::STATE_ON)),
            "the current entry is marked"
        );
    }
}

#[test]
fn an_entry_with_no_hotkey_draws_no_hotkey_text() {
    let menu = DropMenu {
        entries: vec![MenuEntry::Item(MenuItem::new("Back", ""))],
    };
    let area = menu.rect([0.0, 0.0, 10.0, 10.0], [4000.0, 4000.0], 1.0);
    assert_eq!(text_colours(&menu.draw(area, None, false, 1.0)).len(), 1);
}

#[test]
fn a_long_hotkey_widens_the_menu_so_name_and_key_never_meet() {
    let wide = DropMenu {
        entries: vec![MenuEntry::Item(MenuItem::new(
            "Save current layout into slot 9 for the grade",
            "Ctrl+Shift+Alt+Page Down",
        ))],
    };
    assert!(wide.size(1.0)[0] > style::SPACE_MENU_WIDTH);
}

// ---- The Layout menu ----

#[test]
fn the_layout_menu_lists_default_layout_and_the_nine_slots_empty_or_by_name() {
    let mut layouts = Layouts::default();
    layouts.save(1, &arranged(), &NAMES);
    layouts.rename(1, "Colour pass");
    let menu = layout_menu(&layouts, &Hotkeys::default(), None, LayoutMode::Show, None);
    let labels: Vec<String> = items(&menu).into_iter().map(|(_, i)| i.label).collect();
    assert_eq!(labels[0], "Default layout");
    assert_eq!(labels[1], "1  empty");
    assert_eq!(labels[2], "2  Colour pass");
    assert_eq!(labels[9], "9  empty");
    assert_eq!(
        &labels[10..],
        ["Save current layout into…", "Rename layout…"]
    );
}

#[test]
fn a_saved_slot_shows_its_layout_and_an_empty_one_saves_into_it() {
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    let menu = layout_menu(&layouts, &Hotkeys::default(), None, LayoutMode::Show, None);
    let (i, saved) = item(&menu, "1  Layout 1");
    assert_eq!(
        menu.action(i),
        Some(MenuAction::Run(Command::RecallLayout(0)))
    );
    assert_eq!(saved.hotkey, "Ctrl+1");
    let (i, empty) = item(&menu, "2  empty");
    assert_eq!(
        menu.action(i),
        Some(MenuAction::Run(Command::SaveLayout(1)))
    );
    assert_eq!(
        empty.hotkey, "Ctrl+Shift+2",
        "the key that does what the click does"
    );
    let (i, _) = item(&menu, "Default layout");
    assert_eq!(
        menu.action(i),
        Some(MenuAction::Run(Command::DefaultLayout))
    );
}

#[test]
fn the_layout_on_screen_is_marked() {
    let mut layouts = Layouts::default();
    layouts.save(2, &arranged(), &NAMES);
    let hk = Hotkeys::default();
    let marked = |current| {
        items(&layout_menu(&layouts, &hk, current, LayoutMode::Show, None))
            .into_iter()
            .filter(|(_, i)| i.on)
            .map(|(_, i)| i.label)
            .collect::<Vec<_>>()
    };
    assert_eq!(marked(Some(Current::Default)), ["Default layout"]);
    assert_eq!(marked(Some(Current::Slot(2))), ["3  Layout 3"]);
    assert!(
        marked(None).is_empty(),
        "an arrangement saved nowhere marks nothing"
    );
}

#[test]
fn rename_is_greyed_until_a_layout_is_saved() {
    let empty = layout_menu(
        &Layouts::default(),
        &Hotkeys::default(),
        None,
        LayoutMode::Show,
        None,
    );
    let (i, rename) = item(&empty, "Rename layout…");
    assert!(!rename.enabled);
    assert_eq!(empty.action(i), None);
    let mut layouts = Layouts::default();
    layouts.save(4, &arranged(), &NAMES);
    let menu = layout_menu(&layouts, &Hotkeys::default(), None, LayoutMode::Show, None);
    let (i, _) = item(&menu, "Rename layout…");
    assert_eq!(menu.action(i), Some(MenuAction::Mode(LayoutMode::Rename)));
}

#[test]
fn save_lists_every_slot_to_save_into_and_goes_back() {
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    let menu = layout_menu(&layouts, &Hotkeys::default(), None, LayoutMode::Save, None);
    let all = items(&menu);
    assert_eq!(all.len(), SLOTS + 1);
    for slot in 0..SLOTS {
        let (i, it) = &all[slot];
        assert!(it.enabled, "a saved slot can be saved over, from here only");
        assert_eq!(
            menu.action(*i),
            Some(MenuAction::Run(Command::SaveLayout(slot)))
        );
        assert_eq!(it.hotkey, format!("Ctrl+Shift+{}", slot + 1));
    }
    let (i, _) = item(&menu, "Back");
    assert_eq!(menu.action(i), Some(MenuAction::Mode(LayoutMode::Show)));
}

#[test]
fn rename_offers_only_saved_slots_and_shows_what_is_typed() {
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    let menu = layout_menu(
        &layouts,
        &Hotkeys::default(),
        None,
        LayoutMode::Rename,
        None,
    );
    let (i, saved) = item(&menu, "1  Layout 1");
    assert!(saved.enabled);
    assert_eq!(menu.action(i), Some(MenuAction::Rename(0)));
    let (i, empty) = item(&menu, "2  empty");
    assert!(!empty.enabled);
    assert_eq!(menu.action(i), None);
    let mut typing = Rename::start(&layouts, 0).unwrap();
    typing.text = "Gra".to_string();
    let menu = layout_menu(
        &layouts,
        &Hotkeys::default(),
        None,
        LayoutMode::Rename,
        Some(&typing),
    );
    item(&menu, "1  Gra|");
}

#[test]
fn every_entry_runs_the_command_its_shown_hotkey_runs() {
    let mut layouts = Layouts::default();
    for slot in [0, 3, 8] {
        layouts.save(slot, &arranged(), &NAMES);
    }
    // With changed hotkeys too: the menu follows them at once.
    let mut changed = Hotkeys::default();
    changed
        .assign(
            Command::DefaultLayout,
            Hotkey::ctrl(Key::named("D").unwrap()),
        )
        .unwrap();
    changed.clear(Command::RecallLayout(3));
    changed
        .replace(
            Command::SaveLayout(0),
            Hotkey::new(Key::named("F9").unwrap()),
        )
        .unwrap();
    for hotkeys in [Hotkeys::default(), changed] {
        let all_hotkeys: Vec<(String, Command)> = Command::all()
            .into_iter()
            .filter_map(|c| hotkeys.get(c).map(|h| (h.to_string(), c)))
            .collect();
        for mode in [LayoutMode::Show, LayoutMode::Save] {
            let menu = layout_menu(&layouts, &hotkeys, None, mode, None);
            for (i, entry) in items(&menu) {
                let Some(MenuAction::Run(command)) = menu.action(i) else {
                    assert!(
                        entry.hotkey.is_empty(),
                        "{} shows a key but runs no command",
                        entry.label
                    );
                    continue;
                };
                let shown = hotkeys
                    .get(command)
                    .map(|h| h.to_string())
                    .unwrap_or_default();
                assert_eq!(entry.hotkey, shown, "{}", entry.label);
                if !shown.is_empty() {
                    let by_key = all_hotkeys
                        .iter()
                        .find(|(t, _)| *t == shown)
                        .map(|(_, c)| *c);
                    assert_eq!(
                        by_key,
                        Some(command),
                        "{}: the key runs the same command",
                        entry.label
                    );
                }
            }
        }
    }
}

#[test]
fn every_layout_menu_fits_a_small_window() {
    let mut layouts = Layouts::default();
    for slot in 0..SLOTS {
        layouts.save(slot, &arranged(), &NAMES);
        layouts.rename(slot, "A long layout name here");
    }
    for scale in SCALES {
        let window = [480.0 * scale, 400.0 * scale];
        let title = menu_bar::bar().title_rects(0.0, 0.0, scale)[LAYOUT];
        for mode in [LayoutMode::Show, LayoutMode::Save, LayoutMode::Rename] {
            let menu = layout_menu(&layouts, &Hotkeys::default(), None, mode, None);
            let r = menu.drop.rect(title, window, scale);
            assert!(
                r[0] >= 0.0 && r[0] + r[2] <= window[0] + 0.01,
                "{mode:?} fits across"
            );
            assert!(
                r[1] >= MenuBar::height(scale) - 0.01 && r[1] + r[3] <= window[1] + 0.01,
                "{mode:?} fits under the bar"
            );
        }
    }
}
