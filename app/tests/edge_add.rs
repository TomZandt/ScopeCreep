//! Edge add on the layout tree at the pure seam (ADR 0038, #209): the new tab group goes on the chosen side of
//! the chosen Panel at 50/50 and is focused; a side with no room leaves the tree unchanged and gives the notice.
//! No GPU, no window. Rectangles are left, top, width, height in device pixels.
use scopecreep::layout_tree::{Edge, LayoutTree, Placed};
use scopecreep::panel_grid::{MIN_PANEL, min_panel_size};

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

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01)
}

#[test]
fn each_side_puts_the_new_panel_there_at_fifty_fifty_and_focuses_it() {
    let half_w = (AREA[2] - GAP) / 2.0;
    let half_h = (AREA[3] - GAP) / 2.0;
    let first_half_w = [AREA[0], AREA[1], half_w, AREA[3]];
    let second_half_w = [AREA[0] + half_w + GAP, AREA[1], half_w, AREA[3]];
    let first_half_h = [AREA[0], AREA[1], AREA[2], half_h];
    let second_half_h = [AREA[0], AREA[1] + half_h + GAP, AREA[2], half_h];
    let cases = [
        (Edge::Left, second_half_w, first_half_w),
        (Edge::Right, first_half_w, second_half_w),
        (Edge::Top, second_half_h, first_half_h),
        (Edge::Bottom, first_half_h, second_half_h),
    ];
    for (edge, old_cell, new_cell) in cases {
        let mut tree = LayoutTree::automatic(1);
        tree.add_at_edge(1, 0, edge, AREA, GAP, min_panel_size(1.0));
        let cells = tree.cells(AREA, GAP);
        assert!(near(cells[0], old_cell), "{edge:?}: old {:?}", cells[0]);
        assert!(near(cells[1], new_cell), "{edge:?}: new {:?}", cells[1]);
        assert_eq!(tree.focused(), Some(1), "{edge:?}");
    }
}

#[test]
fn an_edge_add_on_a_panel_in_a_bigger_layout_moves_only_that_panel() {
    let mut tree = LayoutTree::default_for(&NAMES);
    tree.close(6);
    let target = 0;
    let before = tree.cells(AREA, GAP);
    tree.add_at_edge(6, target, Edge::Bottom, AREA, GAP, min_panel_size(1.0));
    let after = tree.cells(AREA, GAP);
    let old = before[target];
    assert!(old[3] >= 2.0 * MIN_PANEL[1] + GAP, "{old:?} has room");
    let half = (old[3] - GAP) / 2.0;
    assert!(near(after[target], [old[0], old[1], old[2], half]));
    assert!(near(after[6], [old[0], old[1] + half + GAP, old[2], half]));
    for p in (0..7).filter(|p| *p != target && *p != 6) {
        assert!(near(after[p], before[p]), "Panel {p} moved");
    }
    assert_eq!(tree.focused(), Some(6));
}

#[test]
fn a_side_that_would_break_a_minimum_leaves_the_tree_alone_and_names_it() {
    // A tall cell exactly one minimum wide: it cannot split sideways, but it can split top and bottom.
    let narrow = [AREA[0], AREA[1], MIN_PANEL[0], AREA[3]];
    for edge in [Edge::Left, Edge::Right] {
        let mut tree = LayoutTree::automatic(1);
        let (before, focus) = (tree.clone(), tree.focused());
        let placed = tree.add_at_edge(1, 0, edge, narrow, GAP, min_panel_size(1.0));
        // No room to split: the new Panel is a tab of the target's group (#211).
        assert_eq!(placed, Placed::Tabbed);
        assert_ne!(tree, before);
        assert_eq!(tree.tab_group(0).unwrap().tabs, vec![0, 1]);
        assert_eq!(tree.focused(), Some(1));
        let _ = focus;
        let notice = placed.notice().unwrap();
        assert!(notice.contains("120") && notice.contains("105"), "{notice}");
    }
    for edge in [Edge::Top, Edge::Bottom] {
        let mut tree = LayoutTree::automatic(1);
        tree.add_at_edge(1, 0, edge, narrow, GAP, min_panel_size(1.0));
        assert!(tree.contains(1));
    }
}

#[test]
fn exactly_twice_the_minimum_plus_the_gap_is_enough() {
    let exact = [
        AREA[0],
        AREA[1],
        2.0 * MIN_PANEL[0] + GAP,
        2.0 * MIN_PANEL[1] + GAP,
    ];
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        let mut tree = LayoutTree::automatic(1);
        tree.add_at_edge(1, 0, edge, exact, GAP, min_panel_size(1.0));
        assert!(tree.contains(1), "{edge:?}");
    }
}

#[test]
fn a_panel_already_on_the_page_is_only_focused() {
    let mut tree = LayoutTree::automatic(2);
    let before = tree.clone();
    tree.add_at_edge(0, 1, Edge::Left, AREA, GAP, min_panel_size(1.0));
    assert_eq!(tree, before);
    assert_eq!(tree.focused(), Some(0));
}

#[test]
fn an_edge_add_is_one_change_to_the_tree() {
    // One call adds one Panel and nothing else, so one undo step takes it back (undo itself is #210).
    let mut tree = LayoutTree::automatic(1);
    let count = |t: &LayoutTree| t.cells(AREA, GAP).iter().filter(|c| c[2] > 0.0).count();
    assert_eq!(count(&tree), 1);
    tree.add_at_edge(1, 0, Edge::Right, AREA, GAP, min_panel_size(1.0));
    assert_eq!(count(&tree), 2);
}
