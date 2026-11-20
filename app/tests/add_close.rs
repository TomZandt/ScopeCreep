//! Add panel and Close on the layout tree at the pure seam (ADR 0038, #206): where an added Panel goes, that it is
//! focused, what happens with no room, closing, the empty page, and remembering a layout with Panels closed. No GPU,
//! no window. Rectangles are left, top, width, height in device pixels.
use scopecreep::layout_tree::LayoutTree;
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

fn index(name: &str) -> usize {
    NAMES.iter().position(|n| *n == name).unwrap()
}

fn default_tree() -> LayoutTree {
    LayoutTree::default_for(&NAMES)
}

fn shown(cell: [f32; 4]) -> bool {
    cell[2] > 0.0 && cell[3] > 0.0
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01)
}

#[test]
fn an_empty_page_gets_one_panel_that_fills_the_area_and_has_the_focus() {
    let mut tree = LayoutTree::automatic(0);
    assert!(tree.is_empty());
    assert_eq!(tree.focused(), None);
    tree.add(3, AREA, GAP, min_panel_size(1.0)).unwrap();
    assert_eq!(tree.cells(AREA, GAP)[3], AREA);
    assert_eq!(tree.focused(), Some(3));
    assert!(tree.contains(3));
}

#[test]
fn an_add_on_a_wide_cell_goes_on_the_right_at_fifty_fifty_and_is_focused() {
    let mut tree = LayoutTree::automatic(1);
    tree.add(1, AREA, GAP, min_panel_size(1.0)).unwrap();
    let half = (AREA[2] - GAP) / 2.0;
    let cells = tree.cells(AREA, GAP);
    assert!(
        near(cells[0], [AREA[0], AREA[1], half, AREA[3]]),
        "{cells:?}"
    );
    assert!(near(
        cells[1],
        [AREA[0] + half + GAP, AREA[1], half, AREA[3]]
    ));
    assert_eq!(tree.focused(), Some(1));
}

#[test]
fn an_add_on_a_tall_cell_goes_below_at_fifty_fifty() {
    let tall = [0.0, 0.0, 600.0, 900.0];
    let mut tree = LayoutTree::automatic(1);
    tree.add(1, tall, GAP, min_panel_size(1.0)).unwrap();
    let half = (tall[3] - GAP) / 2.0;
    let cells = tree.cells(tall, GAP);
    assert!(near(cells[0], [0.0, 0.0, 600.0, half]), "{cells:?}");
    assert!(near(cells[1], [0.0, half + GAP, 600.0, half]));
}

#[test]
fn a_square_cell_counts_as_wide() {
    let square = [0.0, 0.0, 700.0, 700.0];
    let mut tree = LayoutTree::automatic(1);
    tree.add(1, square, GAP, min_panel_size(1.0)).unwrap();
    assert!(
        tree.cells(square, GAP)[1][0] > 0.0,
        "the new Panel is on the right"
    );
}

#[test]
fn an_add_splits_the_focused_cell_only() {
    let mut tree = default_tree();
    let (skin, vectorscope) = (index("Skin tone"), index("Vectorscope"));
    tree.close(skin);
    let before = tree.cells(AREA, GAP);
    tree.set_focus(vectorscope);
    tree.add(skin, AREA, GAP, min_panel_size(1.0)).unwrap();
    let after = tree.cells(AREA, GAP);
    let old = before[vectorscope];
    let (first, second) = if old[2] >= old[3] {
        let half = (old[2] - GAP) / 2.0;
        (
            [old[0], old[1], half, old[3]],
            [old[0] + half + GAP, old[1], half, old[3]],
        )
    } else {
        let half = (old[3] - GAP) / 2.0;
        (
            [old[0], old[1], old[2], half],
            [old[0], old[1] + half + GAP, old[2], half],
        )
    };
    assert!(
        near(after[vectorscope], first),
        "{:?} vs {first:?}",
        after[vectorscope]
    );
    assert!(near(after[skin], second), "{:?} vs {second:?}", after[skin]);
    for (i, cell) in before.iter().enumerate() {
        if i != vectorscope && i != skin {
            assert!(near(after[i], *cell), "Panel {i} did not move");
        }
    }
    assert_eq!(tree.focused(), Some(skin));
}

#[test]
fn an_add_with_no_room_leaves_the_tree_unchanged_and_the_notice_names_the_minimum() {
    // Wide, but half of it is under the minimum width.
    let narrow = [0.0, 0.0, 2.0 * MIN_PANEL[0] + GAP - 1.0, 150.0];
    let mut tree = LayoutTree::automatic(1);
    let before = tree.clone();
    let refused = tree.add(1, narrow, GAP, min_panel_size(1.0)).unwrap_err();
    assert_eq!(tree, before);
    assert_eq!(tree.cells(narrow, GAP), before.cells(narrow, GAP));
    assert_eq!(tree.focused(), before.focused());
    let notice = refused.notice();
    assert!(
        notice.contains(&format!("{} x {}", MIN_PANEL[0], MIN_PANEL[1])),
        "{notice}"
    );
    // Tall, but half of it is under the minimum height.
    let short = [0.0, 0.0, 200.0, 2.0 * MIN_PANEL[1] + GAP - 1.0];
    assert!(tree.add(1, short, GAP, min_panel_size(1.0)).is_err());
    // Just enough room is enough.
    let exact = [0.0, 0.0, 2.0 * MIN_PANEL[0] + GAP, 150.0];
    assert!(tree.add(1, exact, GAP, min_panel_size(1.0)).is_ok());
}

#[test]
fn a_panel_already_on_the_page_is_only_focused() {
    let mut tree = default_tree();
    let before = tree.clone();
    tree.add(2, AREA, GAP, min_panel_size(1.0)).unwrap();
    assert_eq!(tree, before);
    assert_eq!(tree.focused(), Some(2));
}

#[test]
fn closing_a_panel_gives_its_space_to_its_sibling() {
    // Stops False Colour is stacked on Skin tone: closing it, Skin tone has both.
    let mut tree = default_tree();
    let (stops, skin) = (index("Stops False Colour"), index("Skin tone"));
    let before = tree.cells(AREA, GAP);
    tree.close(stops);
    let after = tree.cells(AREA, GAP);
    assert!(!shown(after[stops]));
    assert!(!tree.contains(stops));
    let (a, b) = (before[stops], before[skin]);
    assert!(
        near(after[skin], [a[0], a[1], a[2], b[1] + b[3] - a[1]]),
        "{:?}",
        after[skin]
    );
    for (i, cell) in before.iter().enumerate() {
        if i != stops && i != skin {
            assert!(near(after[i], *cell));
        }
    }
}

#[test]
fn closing_a_panel_whose_sibling_is_a_group_gives_the_group_the_space() {
    // Luma waveform is above the Vectorscope, Stops False Colour and Skin tone: they share its height by their
    // own shares, and the column still ends where it did.
    let mut tree = default_tree();
    let (luma, vectorscope, skin) = (
        index("Luma waveform"),
        index("Vectorscope"),
        index("Skin tone"),
    );
    let before = tree.cells(AREA, GAP);
    tree.close(luma);
    let after = tree.cells(AREA, GAP);
    assert_eq!(after[vectorscope][1], before[luma][1]);
    assert_eq!(after[vectorscope][0], before[luma][0]);
    assert_eq!(after[vectorscope][2], before[luma][2]);
    let bottom = |c: [f32; 4]| c[1] + c[3];
    assert!((bottom(after[skin]) - bottom(before[skin])).abs() < 0.01);
    assert!(after[vectorscope][3] > before[vectorscope][3]);
}

#[test]
fn closing_the_focused_panel_focuses_its_neighbour_and_another_panel_keeps_the_focus() {
    let mut tree = default_tree();
    let (luma, vectorscope, skin) = (
        index("Luma waveform"),
        index("Vectorscope"),
        index("Skin tone"),
    );
    tree.set_focus(luma);
    tree.close(skin);
    assert_eq!(tree.focused(), Some(luma));
    tree.close(luma);
    assert_eq!(tree.focused(), Some(vectorscope));
}

#[test]
fn closing_the_last_panel_leaves_an_empty_page() {
    let mut tree = LayoutTree::automatic(2);
    tree.close(0);
    assert!(!tree.is_empty());
    assert!(near(tree.cells(AREA, GAP)[1], AREA));
    tree.close(1);
    assert!(tree.is_empty());
    assert_eq!(tree.focused(), None);
    assert!(tree.cells(AREA, GAP).iter().all(|c| !shown(*c)));
    // Closing what is not there changes nothing.
    tree.close(1);
    assert!(tree.is_empty());
}

#[test]
fn closed_panels_and_the_empty_page_survive_a_restart() {
    let mut tree = default_tree();
    tree.close(index("Skin tone"));
    tree.close(index("Clean picture"));
    let restored = LayoutTree::from_text(&NAMES, &tree.to_text(&NAMES));
    assert_eq!(restored, tree);
    assert_eq!(restored.len(), 5);
    for name in NAMES {
        tree.close(index(name));
    }
    assert!(tree.is_empty());
    let restored = LayoutTree::from_text(&NAMES, &tree.to_text(&NAMES));
    assert!(restored.is_empty());
    assert_eq!(restored, tree);
}

#[test]
fn a_panel_closed_and_added_again_is_in_the_text_where_it_went() {
    let mut tree = default_tree();
    let skin = index("Skin tone");
    tree.close(skin);
    tree.add(skin, AREA, GAP, min_panel_size(1.0)).unwrap();
    let restored = LayoutTree::from_text(&NAMES, &tree.to_text(&NAMES));
    assert_eq!(restored, tree);
    assert_eq!(restored.len(), 7);
}

// ---- Duplicate, Replace and forgetting a removed display (#208, ADR 0039) ----

use scopecreep::layout_tree::{Dir, Edge};

#[test]
fn duplicate_right_and_below_split_the_cell_in_half_beside_the_original() {
    let min = min_panel_size(1.0);
    for check in [0usize, 1usize] {
        let mut tree = default_tree();
        let (target, new) = (index("Clean picture"), 7);
        let before = tree.cells(AREA, GAP)[target];
        let edge = if check == 0 {
            Edge::Right
        } else {
            Edge::Bottom
        };
        tree.add_at_edge(new, target, edge, AREA, GAP, min).unwrap();
        let cells = tree.cells(AREA, GAP);
        let (a, b) = (cells[target], cells[new]);
        if check == 0 {
            let half = (before[2] - GAP) / 2.0;
            assert!(near(a, [before[0], before[1], half, before[3]]), "{a:?}");
            assert!(
                near(b, [before[0] + half + GAP, before[1], half, before[3]]),
                "{b:?}"
            );
        } else {
            let half = (before[3] - GAP) / 2.0;
            assert!(near(a, [before[0], before[1], before[2], half]), "{a:?}");
            assert!(
                near(b, [before[0], before[1] + half + GAP, before[2], half]),
                "{b:?}"
            );
        }
        assert_eq!(tree.focused(), Some(new));
    }
}

#[test]
fn a_duplicate_with_no_room_leaves_the_tree_alone() {
    let mut tree = LayoutTree::automatic(1);
    let before = tree.clone();
    let tiny = [0.0, 0.0, 200.0, 150.0];
    let min = min_panel_size(1.0);
    assert!(!tree.has_room_beside(0, Dir::Row, tiny, GAP, min));
    assert!(tree.add_at_edge(1, 0, Edge::Right, tiny, GAP, min).is_err());
    assert_eq!(tree, before);
    assert_eq!(tree.len(), before.len());
}

#[test]
fn replace_takes_the_place_and_the_focus_and_forget_index_moves_later_panels_up() {
    let mut tree = default_tree();
    let (old, before) = (index("Skin tone"), tree.cells(AREA, GAP));
    tree.set_focus(old);
    tree.replace(old, 7);
    let cells = tree.cells(AREA, GAP);
    assert!(near(cells[7], before[old]));
    assert!(!tree.contains(old));
    assert_eq!(tree.focused(), Some(7));
    // Display 5 (Skin tone, closed now) is gone for good: 6 and 7 become 5 and 6, nothing else moves.
    tree.forget_index(old);
    assert!(tree.contains(6) && !tree.contains(7));
    assert!(near(tree.cells(AREA, GAP)[6], before[old]));
    assert_eq!(tree.focused(), Some(6));
    assert!(near(
        tree.cells(AREA, GAP)[5],
        before[index("Stops False Colour")]
    ));
}
