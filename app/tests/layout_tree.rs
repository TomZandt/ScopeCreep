//! The layout tree at the pure seam (ADR 0038, #204): where each Panel goes, the default layout, dragging the
//! dividers, and remembering the layout (including an old grid). No GPU, no window. Rectangles are left, top, width,
//! height in device pixels.
use scopecreep::layout_tree::{Border, Dir, LayoutTree, Press};
use scopecreep::panel_grid::{PanelLayout, min_panel_size};

/// The Window's Panel area at 1280 x 760: a 12 px margin all round, 20 px of status line under it.
const AREA: [f32; 4] = [12.0, 12.0, 1256.0, 716.0];
const GAP: f32 = 12.0;

/// The 7 displays the Window shows, in Panel order.
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

fn index(name: &str) -> usize {
    NAMES.iter().position(|n| *n == name).unwrap()
}

fn cell_of(tree: &LayoutTree, name: &str) -> [f32; 4] {
    tree.cells(AREA, GAP)[index(name)]
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] - 0.01
        && b[0] < a[0] + a[2] - 0.01
        && a[1] < b[1] + b[3] - 0.01
        && b[1] < a[1] + a[3] - 0.01
}

fn near(a: [f32; 4], b: [f32; 4], by: f32) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < by)
}

/// The cells fill `area` exactly: each is inside it, none overlaps another, and every edge is either the area's
/// edge or one gap from a neighbour's edge it faces. So nothing is empty and no space is lost beyond the gaps.
fn assert_fills(tree: &LayoutTree, area: [f32; 4]) {
    let cells = tree.cells(area, GAP);
    let [left, top, width, height] = area;
    let faces = |c: [f32; 4], d: [f32; 4], right: bool, vertical_overlap: bool| {
        let edge = if right {
            c[0] + c[2] + GAP
        } else {
            c[1] + c[3] + GAP
        };
        let start = if right { d[0] } else { d[1] };
        let along = if vertical_overlap {
            c[1] < d[1] + d[3] - 0.01 && d[1] < c[1] + c[3] - 0.01
        } else {
            c[0] < d[0] + d[2] - 0.01 && d[0] < c[0] + c[2] - 0.01
        };
        (edge - start).abs() < 0.01 && along
    };
    for (i, c) in cells.iter().enumerate() {
        assert!(c[2] >= 1.0 && c[3] >= 1.0, "panel {i} has a size: {c:?}");
        assert!(
            c[0] >= left - 0.01 && c[1] >= top - 0.01,
            "panel {i} inside: {c:?}"
        );
        assert!(
            c[0] + c[2] <= left + width + 0.01 && c[1] + c[3] <= top + height + 0.01,
            "panel {i} inside: {c:?}"
        );
        for (j, d) in cells.iter().enumerate().skip(i + 1) {
            assert!(!overlap(*c, *d), "panels {i} and {j} overlap: {c:?} {d:?}");
        }
        let right_ok = (c[0] + c[2] - (left + width)).abs() < 0.01
            || cells.iter().any(|d| faces(*c, *d, true, true));
        let below_ok = (c[1] + c[3] - (top + height)).abs() < 0.01
            || cells.iter().any(|d| faces(*c, *d, false, false));
        assert!(right_ok && below_ok, "panel {i} leaves a hole: {c:?}");
    }
    // The cells and the gaps between them add up to the area.
    let right = cells.iter().map(|c| c[0] + c[2]).fold(0.0, f32::max);
    let bottom = cells.iter().map(|c| c[1] + c[3]).fold(0.0, f32::max);
    assert!(
        (right - (left + width)).abs() < 0.01 && (bottom - (top + height)).abs() < 0.01,
        "fills the area"
    );
}

fn assert_every_panel_at_least_the_minimum(tree: &LayoutTree, area: [f32; 4]) {
    let min = min_panel_size(1.0);
    for (i, c) in tree.cells(area, GAP).iter().enumerate() {
        assert!(
            c[2] >= min[0] - 0.01 && c[3] >= min[1] - 0.01,
            "panel {i} is {c:?}, under {min:?}"
        );
    }
}

/// Where a grid of column and row weights puts each Panel at `area` (worked out here, independently of the tree):
/// `places` are (column, row, columns, rows).
fn grid_cells(
    columns: &[f32],
    rows: &[f32],
    places: &[[usize; 4]],
    area: [f32; 4],
) -> Vec<[f32; 4]> {
    let track = |w: &[f32], total: f32, start: f32| -> Vec<(f32, f32)> {
        let usable = total - GAP * (w.len() - 1) as f32;
        let sum: f32 = w.iter().sum();
        let mut at = start;
        w.iter()
            .map(|v| {
                let s = (at, v / sum * usable);
                at += s.1 + GAP;
                s
            })
            .collect()
    };
    let (c, r) = (
        track(columns, area[2], area[0]),
        track(rows, area[3], area[1]),
    );
    places
        .iter()
        .map(|[col, row, cs, rs]| {
            let (lx, ly) = (c[col + cs - 1], r[row + rs - 1]);
            [
                c[*col].0,
                r[*row].0,
                lx.0 + lx.1 - c[*col].0,
                ly.0 + ly.1 - r[*row].0,
            ]
        })
        .collect()
}

/// The default layout's cells worked out by hand from the owner's saved split ratios: the first side of a split gets
/// `ratio` of the room net of the gap.
fn default_cells() -> Vec<[f32; 4]> {
    let halves = |r: [f32; 4], row: bool, ratio: f32| -> ([f32; 4], [f32; 4]) {
        if row {
            let w = (r[2] - GAP) * ratio;
            (
                [r[0], r[1], w, r[3]],
                [r[0] + w + GAP, r[1], r[2] - GAP - w, r[3]],
            )
        } else {
            let h = (r[3] - GAP) * ratio;
            (
                [r[0], r[1], r[2], h],
                [r[0], r[1] + h + GAP, r[2], r[3] - GAP - h],
            )
        }
    };
    let (top, bottom) = halves(AREA, false, 0.65909094);
    let (clean, right) = halves(top, true, 0.59458417);
    let (above, luma) = halves(right, false, 0.31868654);
    let (vector, heat) = halves(above, true, 0.5728693);
    let (skin, strip) = halves(bottom, true, 0.29629093);
    let (stops, arri) = halves(strip, true, 0.5);
    let by = |name: &str| match name {
        "Channel Viewer" => clean,
        "Vectorscope" => vector,
        "Saturation" => heat,
        "Waveform" => luma,
        "Skin Tone" => skin,
        "False Colour - Stops" => stops,
        _ => arri,
    };
    NAMES.iter().map(|n| by(n)).collect()
}

/// The old default grid (what an old saved grid converts to): 5 equal columns, rows 14 14 11 11, the seven Panels in
/// NAMES order.
fn old_default_cells() -> Vec<[f32; 4]> {
    let place = |name: &str| match name {
        "Channel Viewer" => [0, 0, 3, 2],
        "Waveform" => [3, 0, 2, 1],
        "Vectorscope" => [3, 1, 2, 1],
        "False Colour - Arri" => [0, 2, 3, 1],
        "False Colour - Stops" => [3, 2, 2, 1],
        "Saturation" => [0, 3, 3, 1],
        _ => [3, 3, 2, 1],
    };
    let places: Vec<[usize; 4]> = NAMES.iter().map(|n| place(n)).collect();
    grid_cells(&[1.0; 5], &[14.0, 14.0, 11.0, 11.0], &places, AREA)
}

// ------------------------------------------------------------------ the default layout

#[test]
fn the_default_layout_is_the_owners_arrangement_with_his_split_ratios() {
    let tree = default_tree();
    let cells = tree.cells(AREA, GAP);
    assert_eq!(cells.len(), 7);
    for ((name, got), want) in NAMES.iter().zip(&cells).zip(default_cells()) {
        assert!(near(*got, want, 0.05), "{name}: {got:?}, want {want:?}");
    }
    assert_fills(&tree, AREA);
}

#[test]
fn the_default_layout_has_the_scopes_beside_the_picture_and_the_views_below() {
    let tree = default_tree();
    let picture = cell_of(&tree, "Channel Viewer");
    assert_eq!([picture[0], picture[1]], [AREA[0], AREA[1]], "top left");
    assert!(picture[2] > 0.55 * AREA[2] && picture[3] > 0.55 * AREA[3]);
    for name in ["Vectorscope", "Saturation", "Waveform"] {
        let c = cell_of(&tree, name);
        assert!(
            c[0] >= picture[0] + picture[2],
            "{name} right of the picture"
        );
        assert!(c[1] < picture[1] + picture[3], "{name} beside the picture");
    }
    let (vector, heat, luma) = (
        cell_of(&tree, "Vectorscope"),
        cell_of(&tree, "Saturation"),
        cell_of(&tree, "Waveform"),
    );
    assert!(
        heat[0] > vector[0] && luma[1] > vector[1] + vector[3],
        "scopes on top, the waveform under"
    );
    for name in ["Skin Tone", "False Colour - Stops", "False Colour - Arri"] {
        let c = cell_of(&tree, name);
        assert!(c[1] >= picture[1] + picture[3], "{name} below the picture");
    }
}

#[test]
fn at_1280_by_760_every_panel_is_at_least_the_minimum_size() {
    assert_every_panel_at_least_the_minimum(&default_tree(), AREA);
    for c in default_tree().cells(AREA, GAP) {
        let p = PanelLayout::new(c, 1.0);
        assert!(p.content[2] >= 100.0 && p.content[3] >= 80.0, "{p:?}");
    }
}

#[test]
fn displays_other_than_the_default_ones_get_an_automatic_layout_that_fills() {
    for n in 1..=20 {
        let tree = LayoutTree::default_for(&vec!["x"; n]);
        assert_eq!(tree.len(), n);
        assert_fills(&tree, AREA);
    }
    assert!(LayoutTree::automatic(0).is_empty());
}

#[test]
fn the_tree_fills_whatever_the_area() {
    for area in [
        [0.0, 0.0, 700.0, 500.0],
        [12.0, 12.0, 2536.0, 1400.0],
        [5.0, 9.0, 1000.0, 1000.0],
    ] {
        assert_fills(&default_tree(), area);
        assert_fills(&LayoutTree::automatic(5), area);
    }
}

// ------------------------------------------------------------------ minimum size

#[test]
fn the_least_area_is_where_every_panel_of_the_tree_is_just_at_its_minimum() {
    let tree = default_tree();
    let min = min_panel_size(1.0);
    let need = tree.min_area(GAP, min);
    let under = |a: [f32; 4]| {
        tree.cells(a, GAP)
            .iter()
            .any(|c| c[2] < min[0] - 0.01 || c[3] < min[1] - 0.01)
    };
    assert!(!under([0.0, 0.0, need[0], need[1]]));
    assert!(under([0.0, 0.0, need[0] - 1.0, need[1]]));
    assert!(under([0.0, 0.0, need[0], need[1] - 1.0]));
    // A tab group needs one Panel; two side by side need both and the gap; stacked need the taller.
    let one = LayoutTree::automatic(1).min_area(GAP, min);
    assert_eq!(one, min);
    let two = LayoutTree::automatic(2).min_area(GAP, min);
    assert!(
        two[0] >= 2.0 * min[0] + GAP - 0.01 && two[1] >= min[1],
        "{two:?}"
    );
}

// ------------------------------------------------------------------ dividers

fn border_between(tree: &LayoutTree, a: &str, b: &str) -> (Border, [f32; 2]) {
    let (ca, cb) = (cell_of(tree, a), cell_of(tree, b));
    // The point in the gap between the two cells, level with the middle of their shared edge.
    let point = if (cb[0] - (ca[0] + ca[2] + GAP)).abs() < 0.1 {
        let top = ca[1].max(cb[1]);
        let bottom = (ca[1] + ca[3]).min(cb[1] + cb[3]);
        [ca[0] + ca[2] + GAP / 2.0, (top + bottom) / 2.0]
    } else {
        let left = ca[0].max(cb[0]);
        let right = (ca[0] + ca[2]).min(cb[0] + cb[2]);
        [(left + right) / 2.0, ca[1] + ca[3] + GAP / 2.0]
    };
    (
        tree.border_at(AREA, GAP, point, 4.0)
            .unwrap_or_else(|| panic!("a border between {a} and {b}")),
        point,
    )
}

#[test]
fn dragging_a_divider_moves_only_the_two_sides_of_its_split() {
    let mut tree = default_tree();
    let before = tree.cells(AREA, GAP);
    let (border, point) = border_between(&tree, "Channel Viewer", "Vectorscope");
    assert_eq!(border.dir, Dir::Row);
    tree.drag(
        &border,
        [point[0] + 60.0, point[1]],
        AREA,
        GAP,
        min_panel_size(1.0),
    );
    let after = tree.cells(AREA, GAP);
    {
        let (name, i) = ("Channel Viewer", index("Channel Viewer"));
        assert!(
            (after[i][2] - (before[i][2] + 60.0)).abs() < 0.5,
            "{name} grows"
        );
        assert_eq!([after[i][1], after[i][3]], [before[i][1], before[i][3]]);
    }
    {
        let (name, i) = ("Waveform", index("Waveform"));
        assert!(
            (after[i][2] - (before[i][2] - 60.0)).abs() < 0.5,
            "{name} shrinks"
        );
        assert_eq!([after[i][1], after[i][3]], [before[i][1], before[i][3]]);
    }
    // The scopes column shrinks as a whole; the bottom strip is on the other side of the Window and is untouched.
    for name in ["Vectorscope", "Saturation"] {
        let i = index(name);
        assert!(
            after[i][0] > before[i][0] + 20.0,
            "{name} moves right with its column"
        );
    }
    for name in ["Skin Tone", "False Colour - Stops", "False Colour - Arri"] {
        assert_eq!(
            after[index(name)],
            before[index(name)],
            "{name} is untouched"
        );
    }
    assert_fills(&tree, AREA);
}

#[test]
fn dragging_a_stacked_divider_leaves_the_other_side_of_the_window_alone() {
    let mut tree = default_tree();
    let before = tree.cells(AREA, GAP);
    let (border, point) = border_between(&tree, "Channel Viewer", "Skin Tone");
    assert_eq!(border.dir, Dir::Col);
    tree.drag(
        &border,
        [point[0], point[1] - 50.0],
        AREA,
        GAP,
        min_panel_size(1.0),
    );
    let after = tree.cells(AREA, GAP);
    // Everything above the divider is 50 shorter and everything below 50 taller; no width changes.
    let tall = |c: &[[f32; 4]]| c[index("Vectorscope")][3] + c[index("Waveform")][3];
    assert!(
        (tall(&after) - (tall(&before) - 50.0)).abs() < 0.5,
        "the scopes column is shorter"
    );
    for name in ["Channel Viewer", "Vectorscope", "Saturation", "Waveform"] {
        let i = index(name);
        assert!(after[i][3] < before[i][3], "{name} is shorter");
        assert_eq!([after[i][0], after[i][2]], [before[i][0], before[i][2]]);
    }
    assert!(
        (after[index("Channel Viewer")][3] - (before[index("Channel Viewer")][3] - 50.0)).abs()
            < 0.5
    );
    for name in ["Skin Tone", "False Colour - Stops", "False Colour - Arri"] {
        let i = index(name);
        assert!(
            (after[i][3] - (before[i][3] + 50.0)).abs() < 0.5,
            "{name} is taller"
        );
        assert_eq!([after[i][0], after[i][2]], [before[i][0], before[i][2]]);
    }
    assert_fills(&tree, AREA);
}

type Row = (usize, usize, bool, bool, bool, bool);

/// Who is left of whom (for Panels that share a row of the Window) and above whom (for those that share a column).
fn arrangement(tree: &LayoutTree, area: [f32; 4]) -> Vec<Row> {
    let cells = tree.cells(area, GAP);
    let mut out = Vec::new();
    for (i, a) in cells.iter().enumerate() {
        for (j, b) in cells.iter().enumerate() {
            let beside = a[1] < b[1] + b[3] - 0.01 && b[1] < a[1] + a[3] - 0.01;
            let stacked = a[0] < b[0] + b[2] - 0.01 && b[0] < a[0] + a[2] - 0.01;
            out.push((
                i,
                j,
                a[0] + a[2] <= b[0] + 0.01,
                a[1] + a[3] <= b[1] + 0.01,
                beside,
                stacked,
            ));
        }
    }
    out
}

#[test]
fn a_divider_stops_where_a_panel_would_go_under_its_minimum_and_never_reorders() {
    let min = min_panel_size(1.0);
    let mut tree = default_tree();
    let order = arrangement(&tree, AREA);
    let borders = [
        border_between(&tree, "Channel Viewer", "Vectorscope"),
        border_between(&tree, "Channel Viewer", "Skin Tone"),
        border_between(&tree, "Vectorscope", "Waveform"),
        border_between(&tree, "Vectorscope", "Saturation"),
        border_between(&tree, "Skin Tone", "False Colour - Stops"),
        border_between(&tree, "False Colour - Stops", "False Colour - Arri"),
    ];
    for (border, _) in &borders {
        for far in [[9000.0, 9000.0], [-9000.0, -9000.0], [9000.0, -9000.0]] {
            tree.drag(border, far, AREA, GAP, min);
            assert_every_panel_at_least_the_minimum(&tree, AREA);
            assert_fills(&tree, AREA);
            let now = arrangement(&tree, AREA);
            let broken: Vec<_> = order
                .iter()
                .zip(&now)
                .filter(|(o, n)| (o.2 && o.4 && n.4 && !n.2) || (o.3 && o.5 && n.5 && !n.3))
                .collect();
            assert!(
                broken.is_empty(),
                "who is next to whom is unchanged: {broken:?}"
            );
        }
    }
}

#[test]
fn dragging_a_divider_that_is_not_there_does_nothing() {
    let mut tree = default_tree();
    let (border, point) = border_between(&tree, "Vectorscope", "Waveform");
    let mut other = LayoutTree::automatic(2);
    other.drag(&border, point, AREA, GAP, min_panel_size(1.0));
    assert_eq!(other, LayoutTree::automatic(2));
    tree.drag(&border, point, AREA, GAP, min_panel_size(1.0));
    assert!(near(
        cell_of(&tree, "Channel Viewer"),
        cell_of(&default_tree(), "Channel Viewer"),
        0.5
    ));
}

#[test]
fn a_border_is_only_found_in_a_gap_not_through_a_panel() {
    let tree = default_tree();
    let scope = cell_of(&tree, "Vectorscope");
    assert_eq!(
        tree.border_at(AREA, GAP, [scope[0] + scope[2] / 2.0, scope[1] + 50.0], 4.0),
        None,
        "no border through the Vectorscope"
    );
    let hero = cell_of(&tree, "Channel Viewer");
    assert_eq!(
        tree.border_at(AREA, GAP, [hero[0] + 30.0, hero[1] + 30.0], 4.0),
        None
    );
    assert_eq!(tree.border_at(AREA, GAP, [0.0, 0.0], 4.0), None);
}

// ------------------------------------------------------------------ pressing and dropping

fn title_point(tree: &LayoutTree, name: &str) -> [f32; 2] {
    let t = PanelLayout::new(cell_of(tree, name), 1.0).title;
    [t[0] + t[2] / 2.0, t[1] + t[3] / 2.0]
}

#[test]
fn a_press_on_a_title_bar_starts_a_panel_drag() {
    let tree = default_tree();
    assert_eq!(
        tree.press_at(AREA, GAP, title_point(&tree, "Skin Tone"), 3.0, 1.0),
        Some(Press::Panel(index("Skin Tone")))
    );
}

#[test]
fn a_press_on_a_display_starts_nothing() {
    let tree = default_tree();
    let c = cell_of(&tree, "Skin Tone");
    assert_eq!(
        tree.press_at(AREA, GAP, [c[0] + c[2] / 2.0, c[1] + c[3] - 10.0], 3.0, 1.0),
        None
    );
}

#[test]
fn a_press_in_a_gap_starts_a_border_drag_not_a_panel_drag() {
    let tree = default_tree();
    let c = cell_of(&tree, "False Colour - Stops");
    let level = c[1] + 10.0;
    assert!(matches!(
        tree.press_at(AREA, GAP, [c[0] - GAP / 2.0, level], 3.0, 1.0),
        Some(Press::Border(Border { dir: Dir::Row, .. }))
    ));
    assert!(matches!(
        tree.press_at(AREA, GAP, [c[0] + 20.0, level], 3.0, 1.0),
        Some(Press::Panel(_))
    ));
}

#[test]
fn the_panel_under_a_point_is_found_for_a_drop() {
    let tree = default_tree();
    let c = cell_of(&tree, "False Colour - Stops");
    assert_eq!(
        tree.panel_at(AREA, GAP, [c[0] + c[2] / 2.0, c[1] + c[3] / 2.0]),
        Some(index("False Colour - Stops"))
    );
    assert_eq!(
        tree.panel_at(AREA, GAP, [c[0] - GAP / 2.0, c[1] + 5.0]),
        None
    );
    assert_eq!(tree.panel_at(AREA, GAP, [0.0, 0.0]), None);
}

#[test]
fn swapping_two_panels_gives_each_the_others_cell() {
    let mut tree = default_tree();
    let (hero, skin) = (
        cell_of(&tree, "Channel Viewer"),
        cell_of(&tree, "Skin Tone"),
    );
    tree.swap(0, index("Skin Tone"));
    assert_eq!(cell_of(&tree, "Channel Viewer"), skin);
    assert_eq!(cell_of(&tree, "Skin Tone"), hero);
    assert_fills(&tree, AREA);
}

#[test]
fn swapping_a_panel_with_itself_changes_nothing() {
    let mut tree = default_tree();
    tree.swap(3, 3);
    assert_eq!(tree, default_tree());
}

// ------------------------------------------------------------------ remembering

#[test]
fn the_layout_survives_a_restart() {
    let mut tree = default_tree();
    let min = min_panel_size(1.0);
    let (a, pa) = border_between(&tree, "Channel Viewer", "Waveform");
    tree.drag(&a, [pa[0] + 70.0, pa[1]], AREA, GAP, min);
    let (b, pb) = border_between(&tree, "Waveform", "Vectorscope");
    tree.drag(&b, [pb[0], pb[1] - 30.0], AREA, GAP, min);
    tree.swap(0, 6);
    assert_ne!(tree.cells(AREA, GAP), default_tree().cells(AREA, GAP));
    let restored = LayoutTree::from_text(&NAMES, &tree.to_text(&NAMES));
    for (x, y) in restored.cells(AREA, GAP).iter().zip(tree.cells(AREA, GAP)) {
        assert!(near(*x, y, 0.01), "{x:?} vs {y:?}");
    }
    assert_eq!(restored, tree);
}

#[test]
fn a_grid_saved_by_v0_2_0_is_ignored_and_the_default_used() {
    let v0_2_0 = "panels 14\nrows 0.33333334 0.33333334 0.33333334\ncolumns 0.2 0.2 0.2 0.2 0.2\n";
    assert_eq!(LayoutTree::from_text(&NAMES, v0_2_0), default_tree());
}

#[test]
fn rubbish_gives_the_default_layout() {
    let default = default_tree();
    let good = default.to_text(&NAMES);
    let first_split = good.lines().nth(1).unwrap().to_string();
    let mut bad: Vec<String> = vec![
        String::new(),
        "not a layout".into(),
        "layout 2".into(),
        good.clone() + "leaf 0 Skin Tone\n",
        good.replace("leaf 0 Skin Tone", "leaf 0 Channel Viewer"),
        good.replace("leaf 0 Skin Tone", "leaf 1 Skin Tone"),
        good.replace("leaf 0 Skin Tone", "leaf x Skin Tone"),
        good.replace("split row", "split diagonal"),
        good.replace("split col", "stack col"),
        good.replacen(&first_split, "split row NaN", 1),
        good.replacen(&first_split, "split row inf", 1),
        good.replacen(&first_split, "split row 0", 1),
        good.replacen(&first_split, "split row 1", 1),
        good.replacen(&first_split, "split row -0.4", 1),
        good.replacen(&first_split, "split row x", 1),
        good.replacen(&first_split, "split row", 1),
    ];
    // Nested far deeper than any layout.
    let mut deep = String::from("layout 2\n");
    for _ in 0..200 {
        deep += "split row 0.5\nleaf 0 Clean picture\n";
    }
    bad.push(deep);
    for text in bad {
        assert_eq!(LayoutTree::from_text(&NAMES, &text), default, "{text:?}");
    }
}

#[test]
fn a_layout_without_some_displays_keeps_them_closed() {
    let one = LayoutTree::from_text(&NAMES, "layout 2\nleaf 0 Clean picture\n");
    assert_eq!(one.len(), 1);
    assert!(one.contains(index("Channel Viewer")));
    assert_eq!(one.cells(AREA, GAP).len(), NAMES.len());
    assert_eq!(one.cells(AREA, GAP)[index("Channel Viewer")], AREA);
    // A display that does not exist is dropped, and the layout keeps the rest.
    let good = default_tree().to_text(&NAMES);
    let without = good.replace("leaf 0 Skin Tone", "leaf 0 Gamut/legal warning");
    let tree = LayoutTree::from_text(&NAMES, &without);
    assert_eq!(tree.len(), 6);
    assert!(!tree.contains(index("Skin Tone")));
}

#[test]
fn a_saved_split_that_leaves_a_panel_under_its_minimum_is_moved_off_it() {
    let good = default_tree().to_text(&NAMES);
    let first_split = good.lines().nth(1).unwrap().to_string();
    for text in [
        good.replacen(&first_split, "split row 0.002", 1),
        good.replacen(&first_split, "split row 0.998", 1),
    ] {
        let tree = LayoutTree::from_text(&NAMES, &text);
        assert_ne!(
            tree,
            default_tree(),
            "the saved layout is kept, not the default"
        );
        assert_fills(&tree, AREA);
        assert_every_panel_at_least_the_minimum(&tree, AREA);
    }
}

#[test]
fn a_layout_saved_at_the_minimum_in_a_large_window_survives_a_restart() {
    let big = [12.0, 12.0, 2536.0, 1400.0];
    let min = min_panel_size(1.0);
    let mut tree = default_tree();
    let (border, _) = border_between(&tree, "Channel Viewer", "Waveform");
    tree.drag(&border, [9000.0, 300.0], big, GAP, min);
    let restored = LayoutTree::from_text(&NAMES, &tree.to_text(&NAMES));
    assert_ne!(restored, default_tree());
    assert_fills(&restored, AREA);
    assert_every_panel_at_least_the_minimum(&restored, AREA);
}

#[test]
fn panels_that_cannot_all_fit_the_reference_window_give_the_default() {
    let names: Vec<String> = (0..12).map(|i| format!("p{i}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut text = String::from("layout 2\n");
    for _ in 0..11 {
        text += "split row 0.5\n";
    }
    for n in &names {
        text += &format!("leaf 0 {n}\n");
    }
    // Twelve side by side at the minimum are wider than the reference window.
    assert_eq!(
        LayoutTree::from_text(&names, &text),
        LayoutTree::default_for(&names)
    );
}

#[test]
fn a_saved_panel_that_no_longer_exists_is_dropped_and_its_sibling_takes_its_space() {
    let good = default_tree().to_text(&NAMES);
    let with_extra = good.replacen(
        "leaf 0 Luma waveform\n",
        "split row 0.5\nleaf 0 Luma waveform\nleaf 0 Gamut/legal warning\n",
        1,
    );
    assert_eq!(LayoutTree::from_text(&NAMES, &with_extra), default_tree());
}

// ---- an old grid

const OLD_DEFAULT: &str = "layout 1\ncolumns 0.2 0.2 0.2 0.2 0.2\nrows 0.28 0.28 0.22 0.22\npanel 0 0 3 2 Clean picture\npanel 3 0 2 1 Luma waveform\npanel 3 1 2 1 Vectorscope\npanel 0 2 3 1 ARRI False Colour\npanel 3 2 2 1 Stops False Colour\npanel 0 3 3 1 Saturation heatmap\npanel 3 3 2 1 Skin tone\n";

#[test]
fn an_old_grid_that_is_a_sequence_of_cuts_converts_to_the_same_arrangement() {
    let tree = LayoutTree::from_text(&NAMES, OLD_DEFAULT);
    for (got, want) in tree.cells(AREA, GAP).iter().zip(old_default_cells()) {
        assert!(near(*got, want, 0.05), "{got:?} vs {want:?}");
    }
    // The same for a grid the user had dragged: the weights are kept in the reference window.
    let dragged = OLD_DEFAULT
        .replace("columns 0.2 0.2 0.2 0.2 0.2", "columns 0.3 0.2 0.1 0.2 0.2")
        .replace("rows 0.28 0.28 0.22 0.22", "rows 0.4 0.2 0.2 0.2");
    let tree = LayoutTree::from_text(&NAMES, &dragged);
    let place = |n: &str| match n {
        "Channel Viewer" => [0, 0, 3, 2],
        "Waveform" => [3, 0, 2, 1],
        "Vectorscope" => [3, 1, 2, 1],
        "False Colour - Arri" => [0, 2, 3, 1],
        "False Colour - Stops" => [3, 2, 2, 1],
        "Saturation" => [0, 3, 3, 1],
        _ => [3, 3, 2, 1],
    };
    let places: Vec<[usize; 4]> = NAMES.iter().map(|n| place(n)).collect();
    let want = grid_cells(
        &[0.3, 0.2, 0.1, 0.2, 0.2],
        &[0.4, 0.2, 0.2, 0.2],
        &places,
        AREA,
    );
    for (got, want) in tree.cells(AREA, GAP).iter().zip(want) {
        assert!(near(*got, want, 0.5), "{got:?} vs {want:?}");
    }
    assert_ne!(tree, default_tree());
}

#[test]
fn an_old_grid_that_is_not_a_sequence_of_cuts_gives_the_default() {
    // A pinwheel: four Panels around a fifth in the middle, no cut runs edge to edge.
    let names = ["a", "b", "c", "d", "e"];
    let pinwheel = "layout 1\ncolumns 0.34 0.33 0.33\nrows 0.34 0.33 0.33\npanel 0 0 2 1 a\npanel 2 0 1 2 b\npanel 1 2 2 1 c\npanel 0 1 1 2 d\npanel 1 1 1 1 e\n";
    assert_eq!(
        LayoutTree::from_text(&names, pinwheel),
        LayoutTree::default_for(&names)
    );
}

#[test]
fn an_old_grid_that_is_rubbish_gives_the_default() {
    let default = default_tree();
    for text in [
        OLD_DEFAULT.replace("panel 0 0 3 2 Clean picture", "panel 4 3 3 2 Clean picture"),
        OLD_DEFAULT.to_string() + "panel 0 0 1 1 Skin tone\n",
        OLD_DEFAULT.replace("panel 3 3 2 1 Skin tone", "panel 4 3 1 1 Skin tone"),
        OLD_DEFAULT.replace("columns 0.2 0.2 0.2 0.2 0.2", "columns 0.2 0.2 0.2 0.2 NaN"),
        OLD_DEFAULT.replace("columns 0.2 0.2 0.2 0.2 0.2", "columns 0.3 0.3 0.3 0.3 0.3"),
        OLD_DEFAULT.replace("columns 0.2 0.2 0.2 0.2 0.2", "columns 0.5 0.5 0 0 0"),
        OLD_DEFAULT.replace("rows 0.28 0.28 0.22 0.22", "rows nonsense"),
        OLD_DEFAULT.replace("panel 3 3 2 1 Skin tone\n", ""),
        "layout 1\ncolumns 0.5 0.5\nrows nonsense\n".into(),
    ] {
        assert_eq!(LayoutTree::from_text(&NAMES, &text), default, "{text}");
    }
}

#[test]
fn an_old_grid_saved_with_the_colour_waveform_and_channel_view_recalls_without_them() {
    let nine = "layout 1\ncolumns 0.2 0.2 0.2 0.2 0.2\nrows 0.28 0.28 0.22 0.22\npanel 0 0 3 2 Clean picture\npanel 3 0 2 1 Luma waveform\npanel 3 1 2 1 Vectorscope\npanel 0 2 2 2 Colour waveform\npanel 2 2 1 1 Channel view\npanel 3 2 2 1 ARRI False Colour\npanel 2 3 1 1 Saturation heatmap\npanel 3 3 1 1 Skin tone\npanel 4 3 1 1 Stops False Colour\n";
    let tree = LayoutTree::from_text(&NAMES, nine);
    assert_ne!(tree, default_tree(), "recalled, not the default");
    assert_eq!(tree.len(), 7);
    assert_fills(&tree, AREA);
    assert_every_panel_at_least_the_minimum(&tree, AREA);
    let clean = cell_of(&tree, "Channel Viewer");
    assert_eq!([clean[0], clean[1]], [AREA[0], AREA[1]], "keeps its corner");
}

#[test]
fn an_old_grid_saved_with_the_two_milestone_2_vectorscope_panels_gives_the_default() {
    let old = "layout 1\ncolumns 0.2 0.2 0.2 0.2 0.2\nrows 0.28 0.28 0.22 0.22\npanel 0 0 3 2 Clean picture\npanel 3 0 2 1 Luma waveform\npanel 3 1 1 1 Vectorscope 100 %\npanel 4 1 1 1 Vectorscope 200 %\npanel 0 2 2 2 Colour waveform\npanel 2 2 1 1 Channel view\npanel 3 2 1 1 ARRI False Colour\npanel 4 2 1 1 Black and white\npanel 2 3 1 1 Saturation heatmap\npanel 3 3 1 1 Skin tone\npanel 4 3 1 1 Stops False Colour\n";
    assert_eq!(LayoutTree::from_text(&NAMES, old), default_tree());
}
