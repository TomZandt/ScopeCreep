//! Undo layout change at the pure layout seam (ADR 0038, #207): the history steps back through add, close, swap and a
//! divider drag, one step each, 20 deep, and touches nothing but the arrangement. No GPU, no window.
use scopecreep::commands::native_shortcut;
use scopecreep::layout_history::{DEPTH, LayoutHistory};
use scopecreep::layout_tree::LayoutTree;
use scopecreep::panel_grid::min_panel_size;

const AREA: [f32; 4] = [8.0, 8.0, 1424.0, 738.0];
const GAP: f32 = 8.0;
const NAMES: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];

fn default_tree() -> LayoutTree {
    LayoutTree::default_for(&NAMES)
}

fn min() -> [f32; 2] {
    min_panel_size(1.0)
}

#[test]
fn a_new_history_has_nothing_to_undo() {
    let mut tree = default_tree();
    let mut history = LayoutHistory::default();
    assert!(!history.can_undo());
    assert!(!history.undo(&mut tree));
    assert_eq!(tree, default_tree());
}

#[test]
fn undo_steps_back_through_close_and_add_one_step_each() {
    let start = default_tree();
    let mut tree = start.clone();
    let mut history = LayoutHistory::default();
    history.change(&mut tree, |t| t.close(5));
    let closed = tree.clone();
    assert_ne!(closed, start);
    history.change(&mut tree, |t| t.add(5, AREA, GAP, min()));
    assert!(history.can_undo());
    assert!(history.undo(&mut tree));
    assert_eq!(tree, closed);
    assert!(history.undo(&mut tree));
    assert_eq!(tree, start);
    assert!(!history.can_undo());
}

#[test]
fn undo_steps_back_through_a_swap() {
    let start = default_tree();
    let mut tree = start.clone();
    let mut history = LayoutHistory::default();
    history.change(&mut tree, |t| t.swap(0, 3));
    assert_ne!(tree, start);
    assert!(history.undo(&mut tree));
    assert_eq!(tree, start);
}

#[test]
fn a_divider_drag_is_one_step_however_long_it_is() {
    let start = default_tree();
    let mut tree = start.clone();
    let mut history = LayoutHistory::default();
    let hero = tree.cells(AREA, GAP)[0];
    let on_border = [hero[0] + hero[2] + GAP / 2.0, hero[1] + 40.0];
    let border = tree
        .border_at(AREA, GAP, on_border, 4.0)
        .expect("a border right of the first Panel");
    history.begin_drag(&tree);
    for step in 1..=10 {
        tree.drag(
            &border,
            [on_border[0] - 5.0 * step as f32, on_border[1]],
            AREA,
            GAP,
            min(),
        );
    }
    history.end_drag(&tree);
    assert_ne!(tree, start);
    assert!(history.undo(&mut tree));
    assert_eq!(tree, start);
    assert!(!history.can_undo());
}

#[test]
fn a_drag_that_moves_nothing_is_not_a_step() {
    let tree = default_tree();
    let mut history = LayoutHistory::default();
    history.begin_drag(&tree);
    history.end_drag(&tree);
    assert!(!history.can_undo());
}

#[test]
fn a_change_that_changes_nothing_is_not_a_step() {
    let mut tree = default_tree();
    let mut history = LayoutHistory::default();
    // Panel 0 is on the page: adding it only focuses it.
    history.change(&mut tree, |t| t.add(0, AREA, GAP, min()));
    assert!(!history.can_undo());
}

#[test]
fn an_add_with_no_room_is_one_step_and_undo_takes_the_tab_away() {
    let mut tree = LayoutTree::automatic(2);
    tree.close(1);
    let before = tree.clone();
    let mut history = LayoutHistory::default();
    let tiny = [0.0, 0.0, 100.0, 100.0];
    let placed = history.change(&mut tree, |t| t.add(1, tiny, GAP, min()));
    assert_eq!(placed, scopecreep::layout_tree::Placed::Tabbed);
    assert!(tree.contains(1));
    assert!(history.undo(&mut tree));
    assert_eq!(tree, before);
    assert!(!history.can_undo());
}

#[test]
fn the_history_is_twenty_deep() {
    assert_eq!(DEPTH, 20);
    let mut tree = default_tree();
    let mut history = LayoutHistory::default();
    let mut states = vec![tree.clone()];
    for i in 0..25 {
        // Alternate swaps so every change is different from the last.
        history.change(&mut tree, |t| t.swap(i % 3, 3 + i % 4));
        states.push(tree.clone());
    }
    let mut undone = 0;
    while history.undo(&mut tree) {
        undone += 1;
    }
    assert_eq!(undone, 20);
    // The oldest 5 changes can no longer be undone: 25 changes, 20 undone, lands on the state after the 5th.
    assert_eq!(tree, states[5]);
}

#[test]
fn each_tree_has_its_own_history() {
    let mut a = default_tree();
    let mut history_a = LayoutHistory::default();
    let history_b = LayoutHistory::default();
    history_a.change(&mut a, |t| t.close(1));
    assert!(history_a.can_undo());
    assert!(!history_b.can_undo());
}

#[test]
fn undo_leaves_the_focus_on_a_panel_that_is_on_the_page() {
    let mut tree = default_tree();
    let mut history = LayoutHistory::default();
    history.change(&mut tree, |t| t.close(5));
    history.change(&mut tree, |t| t.add(5, AREA, GAP, min()));
    assert_eq!(tree.focused(), Some(5));
    history.undo(&mut tree);
    let focus = tree.focused().expect("the page has Panels");
    assert!(tree.contains(focus));
}

// The Command and the Layout menu entry.
use scopecreep::commands::{Command, Hotkey, Hotkeys, Key};
use scopecreep::components::MenuEntry;
use scopecreep::menu_bar::{LayoutMode, MenuAction, layout_menu_with_undo};

#[test]
fn undo_layout_change_is_ctrl_z_by_default_and_can_be_reassigned() {
    let ctrl_z = Hotkey::ctrl(Key::named("Z").unwrap());
    let mut hotkeys = Hotkeys::default();
    assert_eq!(hotkeys.command_for(ctrl_z), Some(Command::UndoLayout));
    assert!(Command::all().contains(&Command::UndoLayout));
    assert_eq!(Command::UndoLayout.label(), "Layout: undo change");
    let other = Hotkey::ctrl(Key::named("U").unwrap());
    hotkeys.assign(Command::UndoLayout, other).unwrap();
    assert_eq!(hotkeys.command_for(ctrl_z), None);
    assert_eq!(hotkeys.command_for(other), Some(Command::UndoLayout));
}

fn undo_entry(can_undo: bool) -> (Option<MenuAction>, bool, String) {
    let menu = layout_menu_with_undo(
        &["Main".to_string()],
        0,
        &Hotkeys::default(),
        LayoutMode::Show,
        can_undo,
    );
    let at = menu
        .drop
        .entries
        .iter()
        .position(|e| matches!(e, MenuEntry::Item(i) if i.label == "Undo layout change"))
        .expect("the Layout menu has Undo layout change");
    let MenuEntry::Item(item) = &menu.drop.entries[at] else {
        unreachable!()
    };
    (menu.action(at), item.enabled, item.hotkey.clone())
}

#[test]
fn the_layout_menu_has_undo_greyed_when_there_is_nothing_to_undo() {
    let (action, enabled, hotkey) = undo_entry(false);
    assert!(!enabled);
    assert_eq!(action, None, "a greyed entry does nothing");
    assert_eq!(hotkey, native_shortcut("Ctrl+Z"));
    let (action, enabled, _) = undo_entry(true);
    assert!(enabled);
    assert_eq!(action, Some(MenuAction::Run(Command::UndoLayout)));
}
