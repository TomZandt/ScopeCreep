//! The menu bar at the pure UI seam (#147, spec #142): the bar's and a menu's size and hit area at 1x and 2x,
//! their states, opening, switching and closing, the Layout menu's entries (empty, in use, current) and that each
//! entry runs the command its shown hotkey runs, and the menus on a small Window.
use scopecreep::commands::{Command, Hotkey, Hotkeys, Key};
use scopecreep::components::*;
use scopecreep::layout_tree::LayoutTree;
use scopecreep::menu_bar::{
    self, BarState, LAYOUT, LayoutMode, MenuAction, SETTINGS, Template, VIEW, layout_menu,
    view_menu,
};
use scopecreep::style;

const SCALES: [f32; 2] = [1.0, 2.0];
const NAMES: [&str; 7] = [
    "Clean picture",
    "Luma waveform",
    "ARRI False Colour",
    "Vectorscope",
    "Saturation heatmap",
    "Skin tone",
    "Stops False Colour",
];

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

fn pages(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

#[test]
fn the_layout_menu_lists_the_pages_then_what_to_do_with_them() {
    let menu = layout_menu(
        &pages(&["Main", "Pictures", "Scopes"]),
        1,
        &Hotkeys::default(),
        LayoutMode::Show,
    );
    let labels: Vec<String> = items(&menu).into_iter().map(|(_, i)| i.label).collect();
    assert_eq!(
        labels,
        [
            "1  Main",
            "2  Pictures",
            "3  Scopes",
            "New page from…",
            "Duplicate page",
            "Rename page",
            "Delete page",
            "Add panel…",
            "Close panel",
            "Auto layout",
            "Auto resize",
            "Undo layout change",
            "Default layout",
        ]
    );
}

#[test]
fn auto_resize_is_a_command_in_the_layout_menu_with_no_default_hotkey() {
    let hk = Hotkeys::default();
    let menu = layout_menu(&pages(&["Main"]), 0, &hk, LayoutMode::Show);
    let (i, resize) = item(&menu, "Auto resize");
    assert!(resize.enabled);
    assert_eq!(resize.hotkey, "");
    assert_eq!(menu.action(i), Some(MenuAction::Run(Command::AutoResize)));
    assert!(Command::all().contains(&Command::AutoResize));
    assert_eq!(hk.get(Command::AutoResize), None);
}

#[test]
fn auto_layout_is_a_command_in_the_layout_menu_with_no_default_hotkey() {
    let hk = Hotkeys::default();
    let menu = layout_menu(&pages(&["Main"]), 0, &hk, LayoutMode::Show);
    let (i, layout) = item(&menu, "Auto layout");
    assert!(layout.enabled);
    assert_eq!(layout.hotkey, "");
    assert_eq!(menu.action(i), Some(MenuAction::Run(Command::AutoLayout)));
    assert!(Command::all().contains(&Command::AutoLayout));
    assert_eq!(hk.get(Command::AutoLayout), None);
    assert_eq!(Command::AutoLayout.label(), "Page: auto layout");
}

#[test]
fn a_page_goes_to_itself_and_shows_the_hotkey_that_does_the_same() {
    let menu = layout_menu(
        &pages(&["Main", "Pictures"]),
        0,
        &Hotkeys::default(),
        LayoutMode::Show,
    );
    let (i, entry) = item(&menu, "2  Pictures");
    assert_eq!(menu.action(i), Some(MenuAction::Page(1)));
    assert_eq!(entry.hotkey, "Ctrl+2");
    // A tenth page has no hotkey.
    let many: Vec<String> = (1..=10).map(|n| format!("P{n}")).collect();
    let menu = layout_menu(&many, 0, &Hotkeys::default(), LayoutMode::Show);
    let (i, tenth) = item(&menu, "10  P10");
    assert_eq!(tenth.hotkey, "");
    assert_eq!(menu.action(i), Some(MenuAction::Page(9)));
}

#[test]
fn the_current_page_is_marked() {
    let hk = Hotkeys::default();
    let marked = |current| {
        items(&layout_menu(
            &pages(&["Main", "Pictures", "Scopes"]),
            current,
            &hk,
            LayoutMode::Show,
        ))
        .into_iter()
        .filter(|(_, i)| i.on)
        .map(|(_, i)| i.label)
        .collect::<Vec<_>>()
    };
    assert_eq!(marked(0), ["1  Main"]);
    assert_eq!(marked(2), ["3  Scopes"]);
}

#[test]
fn delete_is_greyed_with_one_page_and_the_page_entries_are_commands() {
    let hk = Hotkeys::default();
    let one = layout_menu(&pages(&["Main"]), 0, &hk, LayoutMode::Show);
    let (i, delete) = item(&one, "Delete page");
    assert!(!delete.enabled);
    assert_eq!(one.action(i), None);
    let two = layout_menu(&pages(&["Main", "B"]), 0, &hk, LayoutMode::Show);
    let (i, delete) = item(&two, "Delete page");
    assert!(delete.enabled);
    assert_eq!(two.action(i), Some(MenuAction::Run(Command::DeletePage)));
    let (i, _) = item(&two, "Duplicate page");
    assert_eq!(two.action(i), Some(MenuAction::Run(Command::DuplicatePage)));
    let (i, _) = item(&two, "Rename page");
    assert_eq!(two.action(i), Some(MenuAction::RenamePage));
    let (i, _) = item(&two, "Default layout");
    assert_eq!(two.action(i), Some(MenuAction::Run(Command::DefaultLayout)));
    let (i, _) = item(&two, "New page from…");
    assert_eq!(two.action(i), Some(MenuAction::Mode(LayoutMode::NewFrom)));
}

#[test]
fn undo_is_greyed_when_there_is_nothing_to_undo() {
    let hk = Hotkeys::default();
    let names = pages(&["Main"]);
    let menu = menu_bar::layout_menu_with_undo(&names, 0, &hk, LayoutMode::Show, false);
    let (i, undo) = item(&menu, "Undo layout change");
    assert!(!undo.enabled);
    assert_eq!(menu.action(i), None);
}

#[test]
fn new_page_from_offers_the_three_templates_and_goes_back() {
    let menu = layout_menu(
        &pages(&["Main"]),
        0,
        &Hotkeys::default(),
        LayoutMode::NewFrom,
    );
    let labels: Vec<String> = items(&menu).into_iter().map(|(_, i)| i.label).collect();
    assert_eq!(
        labels,
        [
            "Empty page",
            "Default layout (7 Panels)",
            "Copy of this page",
            "Back"
        ]
    );
    for (label, template) in [
        ("Empty page", Template::Empty),
        ("Default layout (7 Panels)", Template::Default),
        ("Copy of this page", Template::Copy),
    ] {
        let (i, _) = item(&menu, label);
        assert_eq!(menu.action(i), Some(MenuAction::NewPage(template)));
    }
    let (i, _) = item(&menu, "Back");
    assert_eq!(menu.action(i), Some(MenuAction::Mode(LayoutMode::Show)));
}

#[test]
fn every_entry_runs_the_command_its_shown_hotkey_runs() {
    // With changed hotkeys too: the menu follows them at once.
    let mut changed = Hotkeys::default();
    changed
        .assign(
            Command::DefaultLayout,
            Hotkey::ctrl(Key::named("D").unwrap()),
        )
        .unwrap();
    changed.clear(Command::GoToPage(3));
    changed
        .replace(
            Command::DuplicatePage,
            Hotkey::new(Key::named("F9").unwrap()),
        )
        .unwrap();
    let names = pages(&["A", "B", "C", "D", "E"]);
    for hotkeys in [Hotkeys::default(), changed] {
        let all_hotkeys: Vec<(String, Command)> = Command::all()
            .into_iter()
            .filter_map(|c| hotkeys.get(c).map(|h| (h.to_string(), c)))
            .collect();
        for mode in [LayoutMode::Show, LayoutMode::NewFrom] {
            let menu = layout_menu(&names, 0, &hotkeys, mode);
            for (i, entry) in items(&menu) {
                let command = match menu.action(i) {
                    Some(MenuAction::Run(command)) => command,
                    // A page entry goes to its page: the same as Go to page n.
                    Some(MenuAction::Page(page)) if page < 9 => Command::GoToPage(page),
                    // New page from: Empty and Copy are the commands of those names.
                    Some(MenuAction::NewPage(Template::Empty)) => Command::NewPage,
                    Some(MenuAction::NewPage(Template::Copy)) => Command::DuplicatePage,
                    _ => {
                        assert!(
                            entry.hotkey.is_empty(),
                            "{} shows a key but runs no command",
                            entry.label
                        );
                        continue;
                    }
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
fn every_layout_menu_fits_the_smallest_window() {
    // The smallest the Window can be made counts the Layout menu with the most pages.
    let names: Vec<String> = (0..scopecreep::pages::MAX_PAGES)
        .map(|_| "A long page name here".to_string())
        .collect();
    let [min_w, min_h] = scopecreep::layout::min_window(
        &LayoutTree::default_for(&NAMES),
        &menu_bar::largest_menus(),
    );
    for scale in SCALES {
        let window = [min_w * scale, min_h * scale];
        let title = menu_bar::bar().title_rects(0.0, 0.0, scale)[LAYOUT];
        for mode in [LayoutMode::Show, LayoutMode::NewFrom] {
            let menu = layout_menu(&names, 0, &Hotkeys::default(), mode);
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

// ---- The View menu (#191, spec #192) ----

#[test]
fn the_view_title_sits_between_layout_and_settings_and_is_hit_where_drawn() {
    let bar = menu_bar::bar();
    assert_eq!(bar.titles, ["Layout", "View", "Settings"]);
    assert_eq!((LAYOUT, VIEW, SETTINGS), (0, 1, 2));
    for scale in SCALES {
        let rects = bar.title_rects(0.0, 0.0, scale);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0][0] + rects[0][2], rects[1][0], "no gap");
        assert_eq!(rects[1][0] + rects[1][2], rects[2][0], "no gap");
        assert_eq!(rects[VIEW][3], MenuBar::height(scale));
        assert_eq!(bar.hit(0.0, 0.0, centre(rects[VIEW]), scale), Some(VIEW));
    }
    let (one, two) = (
        bar.title_rects(0.0, 0.0, 1.0)[VIEW],
        bar.title_rects(0.0, 0.0, 2.0)[VIEW],
    );
    for k in 0..4 {
        assert!((one[k] * 2.0 - two[k]).abs() < 1e-3, "doubles at 2x");
    }
}

#[test]
fn the_views_first_entry_shows_or_hides_the_legends_and_is_marked_when_every_legend_is_on() {
    let hotkeys = Hotkeys::default();
    for on in [false, true] {
        let menu = view_menu(&hotkeys, on);
        let (i, entry) = item(&menu, "Show legends");
        assert_eq!(i, 0, "the first entry");
        assert_eq!(entry.on, on);
        assert!(entry.enabled);
        assert_eq!(entry.hotkey, "", "no default hotkey");
        assert_eq!(
            menu.action(i),
            Some(MenuAction::Run(Command::ToggleLegends))
        );
        for scale in SCALES {
            let title = menu_bar::bar().title_rects(0.0, 0.0, scale)[VIEW];
            let r = menu.drop.rect(title, [800.0 * scale, 600.0 * scale], scale);
            assert_eq!(r[0], title[0], "drops under its title");
            let row = menu.drop.entry_rects(r, scale)[0];
            let hit = menu.drop.hit(r, centre(row), scale);
            assert_eq!(hit, Some(0), "the entry is hit at {scale}x");
        }
    }
}

#[test]
fn the_views_entry_shows_the_hotkey_the_command_has() {
    let mut hotkeys = Hotkeys::default();
    hotkeys
        .assign(
            Command::ToggleLegends,
            Hotkey::ctrl(Key::named("L").unwrap()),
        )
        .unwrap();
    let (_, entry) = item(&view_menu(&hotkeys, true), "Show legends");
    assert_eq!(entry.hotkey, "Ctrl+L");
}

#[test]
fn the_view_menu_is_among_the_menus_the_minimum_size_is_worked_out_from_with_its_widest_hotkey() {
    let menus = menu_bar::largest_menus();
    assert!(
        menus.iter().any(|m| m.entries.iter().any(|e| matches!(
            e,
            MenuEntry::Item(i) if i.label == "Show legends" && i.hotkey == "Ctrl+Shift+Alt+Page Down"
        ))),
        "{menus:?}"
    );
}
