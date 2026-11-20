//! Tab groups on the layout tree at the pure seam (ADR 0038, #211): join as a tab, switch tab, drag a tab out (the
//! others remain), close a tab, a one-Panel tab group is as before, and a tab group saves and loads. No GPU, no
//! window.
use scopecreep::layout_history::LayoutHistory;
use scopecreep::layout_tree::{Edge, LayoutTree, Placed};
use scopecreep::panel_grid::min_panel_size;

const AREA: [f32; 4] = [12.0, 12.0, 1256.0, 716.0];
const GAP: f32 = 12.0;
const NAMES: [&str; 7] = [
    "Clean picture",
    "Luma waveform",
    "ARRI False Colour",
    "Vectorscope",
    "Saturation heatmap",
    "Skin tone",
    "Stops False Colour",
];

fn tree() -> LayoutTree {
    LayoutTree::default_for(&NAMES)
}

fn shown(cell: [f32; 4]) -> bool {
    cell[2] > 0.0 && cell[3] > 0.0
}

#[test]
fn a_panel_dropped_in_the_middle_becomes_a_shown_tab_of_that_group() {
    let mut t = tree();
    let before = t.cells(AREA, GAP);
    t.join_tab(5, 0);
    let group = t.tab_group(0).unwrap();
    assert_eq!(group.tabs, vec![0, 5]);
    assert_eq!(group.tabs[group.active], 5);
    assert_eq!(t.focused(), Some(5));
    assert_eq!(t.len(), 7);
    let after = t.cells(AREA, GAP);
    // The dragged Panel is in the target's cell, the target is hidden, and the old cell's neighbour took its space.
    assert_eq!(after[5], before[0]);
    assert!(!shown(after[0]));
    assert!(after[6][3] > before[6][3]);
}

#[test]
fn clicking_a_tab_shows_that_panel_and_only_that_one_has_a_cell() {
    let mut t = tree();
    t.join_tab(5, 0);
    let cell = t.cells(AREA, GAP)[5];
    t.show(0);
    let cells = t.cells(AREA, GAP);
    assert_eq!(cells[0], cell);
    assert!(!shown(cells[5]));
    assert_eq!(t.focused(), Some(0));
    assert_eq!(t.tab_group(5).unwrap().active, 0);
    // The shown Panels: one per tab group.
    let flags = t.shown_panels();
    assert_eq!(flags.iter().filter(|f| **f).count(), 6);
    assert!(flags[0] && !flags[5]);
}

#[test]
fn a_tab_dragged_out_docks_elsewhere_and_the_others_stay() {
    let mut t = tree();
    t.join_tab(5, 0);
    t.join_tab(2, 0);
    assert_eq!(t.tab_group(0).unwrap().tabs, vec![0, 5, 2]);
    let target = 3;
    let placed = t.dock(5, target, Edge::Left, AREA, GAP, min_panel_size(1.0));
    assert_eq!(placed, Placed::Split);
    assert_eq!(t.tab_group(0).unwrap().tabs, vec![0, 2]);
    assert_eq!(t.tab_group(5).unwrap().tabs, vec![5]);
    assert_eq!(t.len(), 7);
    let cells = t.cells(AREA, GAP);
    assert!(shown(cells[5]) && shown(cells[2]) && !shown(cells[0]));
}

#[test]
fn the_active_tab_dragged_to_its_own_groups_edge_docks_and_the_next_tab_is_shown() {
    let mut t = tree();
    t.join_tab(5, 0);
    assert_eq!(t.tab_group(0).unwrap().active, 1);
    t.dock(5, 5, Edge::Right, AREA, GAP, min_panel_size(1.0));
    assert_eq!(t.tab_group(0).unwrap().tabs, vec![0]);
    assert_eq!(t.tab_group(5).unwrap().tabs, vec![5]);
    let cells = t.cells(AREA, GAP);
    assert!(shown(cells[0]) && shown(cells[5]));
}

#[test]
fn closing_a_tab_leaves_the_others_and_closing_the_shown_tab_shows_the_next() {
    let mut t = tree();
    t.join_tab(5, 0);
    t.join_tab(2, 0);
    t.show(5);
    t.close(5);
    let g = t.tab_group(0).unwrap();
    assert_eq!(g.tabs, vec![0, 2]);
    assert_eq!(g.tabs[g.active], 2, "the next tab takes its place");
    assert_eq!(t.focused(), Some(2));
    // Closing a tab that is not shown leaves the shown one shown.
    t.show(0);
    t.close(2);
    let g = t.tab_group(0).unwrap();
    assert_eq!((g.tabs.clone(), g.active), (vec![0], 0));
}

#[test]
fn a_one_panel_tab_group_is_as_a_panel_was() {
    let t = tree();
    for p in 0..7 {
        let g = t.tab_group(p).unwrap();
        assert_eq!((g.tabs, g.active), (vec![p], 0));
    }
    assert!(t.shown_panels().iter().all(|s| *s));
    // A tab out leaves the group it came from as it was before the tab joined.
    let mut u = t.clone();
    u.join_tab(5, 0);
    u.dock(5, 6, Edge::Left, AREA, GAP, min_panel_size(1.0));
    assert_eq!(u.tab_group(0).unwrap().tabs, vec![0]);
}

#[test]
fn dropping_a_tab_on_its_own_group_only_shows_it() {
    let mut t = tree();
    t.join_tab(5, 0);
    t.show(0);
    let before = t.cells(AREA, GAP);
    t.join_tab(5, 0);
    assert_eq!(t.tab_group(0).unwrap().tabs, vec![0, 5]);
    assert_eq!(t.tab_group(0).unwrap().active, 1);
    assert_eq!(t.cells(AREA, GAP)[5], before[0]);
}

#[test]
fn tabbing_is_one_undo_step() {
    let mut t = tree();
    let start = t.clone();
    let mut history = LayoutHistory::default();
    history.change(&mut t, |t| t.join_tab(5, 0));
    assert_ne!(t, start);
    assert!(history.undo(&mut t));
    assert_eq!(t, start);
    assert!(!history.can_undo());
}

#[test]
fn a_no_room_add_edge_add_and_dock_are_one_undo_step_and_never_lose_the_panel() {
    let narrow = [0.0, 0.0, 200.0, 150.0];
    let min = min_panel_size(1.0);
    // Add panel.
    let mut t = LayoutTree::automatic(2);
    t.close(1);
    let start = t.clone();
    let mut h = LayoutHistory::default();
    let placed = h.change(&mut t, |t| t.add(1, narrow, GAP, min));
    assert_eq!(placed, Placed::Tabbed);
    assert!(t.contains(1) && t.len() == 2);
    assert!(placed.notice().unwrap().contains("tab"));
    h.undo(&mut t);
    assert_eq!(t, start);
    // Edge add.
    let placed = h.change(&mut t, |t| {
        t.add_at_edge(1, 0, Edge::Right, narrow, GAP, min)
    });
    assert_eq!(placed, Placed::Tabbed);
    assert_eq!(t.tab_group(0).unwrap().tabs, vec![0, 1]);
    h.undo(&mut t);
    assert_eq!(t, start);
    assert!(!h.can_undo());
    // Side dock.
    let mut t = LayoutTree::automatic(2);
    let start = t.clone();
    let placed = h.change(&mut t, |t| t.dock(1, 0, Edge::Right, narrow, GAP, min));
    assert_eq!(placed, Placed::Tabbed);
    assert_eq!(t.len(), 2);
    assert!(placed.dock_notice().unwrap().contains("120"));
    h.undo(&mut t);
    assert_eq!(t, start);
}

#[test]
fn a_tab_group_and_its_shown_tab_survive_a_restart() {
    let mut t = tree();
    t.join_tab(5, 0);
    t.join_tab(2, 0);
    t.show(5);
    let text = t.to_text(&NAMES);
    let back = LayoutTree::from_text(&NAMES, &text);
    assert_eq!(back, t);
    let g = back.tab_group(0).unwrap();
    assert_eq!((g.tabs, g.active), (vec![0, 5, 2], 1));
}
