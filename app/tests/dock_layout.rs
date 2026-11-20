//! Docking on the layout tree at the pure seam (ADR 0038, #210): a drop on each quarter edge splits that edge with
//! the dragged Panel taking that half, the source's neighbour takes its old space, a drop on its own area does
//! nothing, Shift swaps, and a side dock that breaks a minimum changes nothing. No GPU, no window.
use scopecreep::layout_tree::{Edge, LayoutTree, Placed};
use scopecreep::panel_grid::{MIN_PANEL, min_panel_size};

const AREA: [f32; 4] = [12.0, 12.0, 1256.0, 716.0];
const GAP: f32 = 12.0;
const EDGES: [Edge; 4] = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];

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

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01)
}

fn cell_of(tree: &LayoutTree, name: &str) -> [f32; 4] {
    tree.cells(AREA, GAP)[index(name)]
}

#[test]
fn a_dock_on_each_edge_gives_the_dragged_panel_that_half() {
    let target = "Clean picture";
    for edge in EDGES {
        let mut tree = default_tree();
        let skin = index("Skin tone");
        let [x, y, w, h] = cell_of(&tree, target);
        tree.dock(skin, index(target), edge, AREA, GAP, min_panel_size(1.0));
        let (hw, hh) = ((w - GAP) / 2.0, (h - GAP) / 2.0);
        let (mine, other) = match edge {
            Edge::Left => ([x, y, hw, h], [x + hw + GAP, y, hw, h]),
            Edge::Right => ([x + hw + GAP, y, hw, h], [x, y, hw, h]),
            Edge::Top => ([x, y, w, hh], [x, y + hh + GAP, w, hh]),
            Edge::Bottom => ([x, y + hh + GAP, w, hh], [x, y, w, hh]),
        };
        assert!(near(cell_of(&tree, "Skin tone"), mine), "{edge:?}");
        assert!(near(cell_of(&tree, target), other), "{edge:?}");
        assert_eq!(tree.focused(), Some(skin));
        assert_eq!(tree.len(), 7);
    }
}

#[test]
fn the_sources_neighbour_takes_its_old_space() {
    let mut tree = default_tree();
    let before = cell_of(&tree, "Skin tone");
    // Stops False Colour sits directly above Skin tone in the right-hand column.
    let above = cell_of(&tree, "Stops False Colour");
    tree.dock(
        index("Skin tone"),
        index("Clean picture"),
        Edge::Right,
        AREA,
        GAP,
        min_panel_size(1.0),
    );
    let after = cell_of(&tree, "Stops False Colour");
    assert!((after[1] - above[1]).abs() < 0.01);
    assert!((after[3] - (above[3] + GAP + before[3])).abs() < 0.01);
}

#[test]
fn a_drop_on_its_own_area_does_nothing() {
    let mut tree = default_tree();
    let was = tree.clone();
    let skin = index("Skin tone");
    for edge in EDGES {
        tree.dock(skin, skin, edge, AREA, GAP, min_panel_size(1.0));
        assert_eq!(tree, was);
    }
}

#[test]
fn a_side_dock_that_breaks_a_minimum_changes_nothing_and_says_so() {
    let mut tree = default_tree();
    let was = tree.clone();
    let area = [12.0, 12.0, 300.0, 200.0];
    let cells = tree.cells(area, GAP);
    let narrow = cells
        .iter()
        .position(|c| c[2] > 0.0 && c[2] < 2.0 * MIN_PANEL[0] + GAP - 0.01)
        .unwrap();
    let other = (narrow + 1) % 7;
    let placed = tree.dock(other, narrow, Edge::Left, area, GAP, min_panel_size(1.0));
    // No room to split: the dragged Panel is a tab of the target's group, shown, and not lost (#211).
    assert_eq!(placed, Placed::Tabbed);
    assert_ne!(tree, was);
    assert_eq!(tree.len(), 7);
    let group = tree.tab_group(narrow).unwrap();
    assert!(group.tabs.contains(&other) && group.tabs.contains(&narrow));
    assert_eq!(group.tabs[group.active], other);
    assert!(
        placed
            .dock_notice()
            .unwrap()
            .contains(&MIN_PANEL[0].to_string())
    );
}

#[test]
fn shift_swap_still_exchanges_two_places() {
    let mut tree = default_tree();
    let (a, b) = (index("Luma waveform"), index("Skin tone"));
    let (ca, cb) = (cell_of(&tree, "Luma waveform"), cell_of(&tree, "Skin tone"));
    tree.swap(a, b);
    assert!(near(cell_of(&tree, "Luma waveform"), cb));
    assert!(near(cell_of(&tree, "Skin tone"), ca));
}
