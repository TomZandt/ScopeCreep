//! The Panel layout at the pure seam (#96): where each Panel goes (cells and spans), the default layout, dragging
//! the borders between columns and rows, remembering the layout, and what sits inside a Panel (border, title bar,
//! display) at small sizes and on a HiDPI screen. Rectangles are left, top, width, height in device pixels.
use scopecreep::panel_grid::{Border, PanelGrid, PanelLayout, Press, min_panel_size};

/// The Window's Panel area at 1280 x 760: a 12 px margin all round, 20 px of status line under it.
const AREA: [f32; 4] = [12.0, 12.0, 1256.0, 716.0];
const GAP: f32 = 12.0;

/// The 8 displays the Window shows, in Panel order.
const NAMES: [&str; 8] = [
    "Clean picture",
    "Luma waveform",
    "Channel view",
    "ARRI False Colour",
    "Vectorscope",
    "Saturation heatmap",
    "Skin tone",
    "Stops False Colour",
];

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] - 0.01 && b[0] < a[0] + a[2] - 0.01 && a[1] < b[1] + b[3] - 0.01 && b[1] < a[1] + a[3] - 0.01
}

/// Every Panel is inside the area, none overlaps another, and every cell of the grid is used.
fn assert_fills(grid: &PanelGrid) {
    let cells = grid.cells(AREA, GAP);
    for (i, c) in cells.iter().enumerate() {
        assert!(c[2] >= 1.0 && c[3] >= 1.0, "panel {i} has a size: {c:?}");
        assert!(c[0] >= AREA[0] - 0.01 && c[1] >= AREA[1] - 0.01, "panel {i} inside: {c:?}");
        assert!(c[0] + c[2] <= AREA[0] + AREA[2] + 0.01 && c[1] + c[3] <= AREA[1] + AREA[3] + 0.01, "panel {i} inside: {c:?}");
        for (j, d) in cells.iter().enumerate().skip(i + 1) {
            assert!(!overlap(*c, *d), "panels {i} and {j} overlap: {c:?} {d:?}");
        }
    }
    let (columns, rows) = grid.size();
    let mut used = vec![vec![0; columns]; rows];
    for p in grid.places() {
        for r in p.row..p.row + p.rows {
            for c in p.column..p.column + p.columns {
                used[r][c] += 1;
            }
        }
    }
    assert!(used.iter().flatten().all(|n| *n == 1), "every cell is used exactly once: {used:?}");
    let right = cells.iter().map(|c| c[0] + c[2]).fold(0.0, f32::max);
    let bottom = cells.iter().map(|c| c[1] + c[3]).fold(0.0, f32::max);
    assert!((right - (AREA[0] + AREA[2])).abs() < 0.01 && (bottom - (AREA[1] + AREA[3])).abs() < 0.01, "fills the area");
}

fn default_grid() -> PanelGrid {
    PanelGrid::default_for(&NAMES)
}

fn cell_of(grid: &PanelGrid, name: &str) -> [f32; 4] {
    grid.cells(AREA, GAP)[NAMES.iter().position(|n| *n == name).unwrap()]
}

// ------------------------------------------------------------------ the default layout

#[test]
fn the_default_layout_puts_the_clean_picture_three_by_two_at_the_top_left() {
    let grid = default_grid();
    assert_eq!(grid.size(), (5, 4));
    let place = grid.places()[0];
    assert_eq!((place.column, place.row, place.columns, place.rows), (0, 0, 3, 2));
    let picture = cell_of(&grid, "Clean picture");
    assert_eq!([picture[0], picture[1]], [AREA[0], AREA[1]], "top left");
    // Three columns and two rows of the grid and the gaps between them.
    assert!(picture[2] > 0.55 * AREA[2] && picture[3] > 0.45 * AREA[3], "the hero is big: {picture:?}");
}

#[test]
fn the_default_layout_has_the_scopes_beside_the_picture_and_the_views_below() {
    let grid = default_grid();
    let picture = cell_of(&grid, "Clean picture");
    for name in ["Luma waveform", "Vectorscope"] {
        let c = cell_of(&grid, name);
        assert!(c[0] >= picture[0] + picture[2], "{name} is right of the picture: {c:?}");
        assert!(c[1] < picture[1] + picture[3], "{name} is beside the picture: {c:?}");
    }
    for name in ["Channel view", "ARRI False Colour", "Saturation heatmap", "Skin tone", "Stops False Colour"] {
        let c = cell_of(&grid, name);
        assert!(c[1] >= picture[1] + picture[3], "{name} is below the picture: {c:?}");
    }
}

#[test]
fn the_default_layout_uses_every_cell() {
    assert_fills(&default_grid());
}

#[test]
fn a_panel_spans_several_columns_and_rows() {
    let grid = default_grid();
    let one = cell_of(&grid, "Skin tone");
    let hero = cell_of(&grid, "Clean picture");
    let luma = cell_of(&grid, "Luma waveform");
    assert!((hero[2] - (3.0 * one[2] + 2.0 * GAP)).abs() < 0.01, "three columns and the gaps between: {hero:?} {one:?}");
    assert!(hero[3] > one[3] * 1.5, "two rows");
    assert!((luma[2] - (2.0 * one[2] + GAP)).abs() < 0.01, "two columns wide");
}

#[test]
fn at_1280_by_760_every_panel_is_at_least_the_minimum_size() {
    let grid = default_grid();
    let min = min_panel_size(1.0);
    for (name, cell) in NAMES.iter().zip(grid.cells(AREA, GAP)) {
        assert!(cell[2] >= min[0] && cell[3] >= min[1], "{name} is {cell:?}, under {min:?}");
        let p = PanelLayout::new(cell, 1.0);
        assert!(p.content[2] >= 100.0 && p.content[3] >= 80.0, "{name}: {p:?}");
    }
}

#[test]
fn displays_other_than_the_default_ones_get_an_automatic_grid_that_fills() {
    for n in 1..=20 {
        let grid = PanelGrid::default_for(&vec!["x"; n]);
        assert_eq!(grid.places().len(), n);
        assert_fills(&grid);
    }
}

// ------------------------------------------------------------------ borders

#[test]
fn dragging_a_column_border_resizes_the_columns_on_each_side() {
    let mut grid = default_grid();
    let before = grid.cells(AREA, GAP);
    // The border between columns 2 and 3, in the bottom row (Saturation heatmap, Skin tone...).
    let skin = cell_of(&grid, "Skin tone");
    let (x, y) = (skin[0] - GAP / 2.0, skin[1] + skin[3] / 2.0);
    let border = grid.border_at(AREA, GAP, [x, y], 4.0).expect("a border is there");
    assert_eq!(border, Border::Column { after: 2 });
    grid.drag(border, [x + 60.0, y], AREA, GAP, min_panel_size(1.0));
    let after = grid.cells(AREA, GAP);
    let i = NAMES.iter().position(|n| *n == "Skin tone").unwrap();
    let channel = NAMES.iter().position(|n| *n == "Saturation heatmap").unwrap();
    assert!((after[i][2] - (before[i][2] - 60.0)).abs() < 0.5, "the right column shrinks");
    assert!((after[channel][2] - (before[channel][2] + 60.0)).abs() < 0.5, "the left column grows");
    assert!((after[0][2] - before[0][2]).abs() > 1.0, "the hero spans the left column, so it grows with it");
    assert_fills(&grid);
}

#[test]
fn dragging_a_row_border_resizes_the_rows_above_and_below() {
    let mut grid = default_grid();
    let before = grid.cells(AREA, GAP);
    let hero = before[0];
    let (x, y) = (hero[0] + hero[2] / 2.0, hero[1] + hero[3] + GAP / 2.0);
    let border = grid.border_at(AREA, GAP, [x, y], 4.0).unwrap();
    assert_eq!(border, Border::Row { after: 1 });
    grid.drag(border, [x, y - 50.0], AREA, GAP, min_panel_size(1.0));
    let after = grid.cells(AREA, GAP);
    assert!((after[0][3] - (before[0][3] - 50.0)).abs() < 0.5);
    assert!((after[2][3] - (before[2][3] + 50.0)).abs() < 0.5, "the Channel view below grows");
    assert_fills(&grid);
}

#[test]
fn a_border_is_only_found_between_panels_not_through_one_that_spans_it() {
    let grid = default_grid();
    let hero = cell_of(&grid, "Clean picture");
    // Inside the hero, on the line between its columns 0 and 1 (and rows 0 and 1): no border.
    let one_column = (hero[2] - 2.0 * GAP) / 3.0;
    assert_eq!(grid.border_at(AREA, GAP, [hero[0] + one_column + GAP / 2.0, hero[1] + 50.0], 4.0), None);
    assert_eq!(grid.border_at(AREA, GAP, [hero[0] + 50.0, hero[1] + hero[3] / 2.0], 4.0), None);
    // A point inside a Panel is not on a border.
    assert_eq!(grid.border_at(AREA, GAP, [hero[0] + 30.0, hero[1] + 30.0], 4.0), None);
}

#[test]
fn a_panel_cannot_be_dragged_smaller_than_its_minimum() {
    let min = min_panel_size(1.0);
    let mut grid = default_grid();
    for (border, far, sign) in [(Border::Column { after: 2 }, [5000.0, 300.0], 1), (Border::Column { after: 2 }, [-5000.0, 300.0], 0), (Border::Row { after: 2 }, [300.0, 5000.0], 1), (Border::Row { after: 2 }, [300.0, -5000.0], 0)] {
        grid.drag(border, far, AREA, GAP, min);
        for (name, c) in NAMES.iter().zip(grid.cells(AREA, GAP)) {
            assert!(c[2] >= min[0] - 0.01 && c[3] >= min[1] - 0.01, "{name} keeps its minimum after dragging {border:?} ({sign}): {c:?}");
        }
        assert_fills(&grid);
    }
}

// ------------------------------------------------------------------ remembering

#[test]
fn the_layout_survives_a_restart() {
    let mut grid = default_grid();
    grid.drag(Border::Column { after: 2 }, [700.0, 300.0], AREA, GAP, min_panel_size(1.0));
    grid.drag(Border::Row { after: 1 }, [300.0, 300.0], AREA, GAP, min_panel_size(1.0));
    assert_ne!(grid.cells(AREA, GAP), default_grid().cells(AREA, GAP), "the drags moved something");
    let restored = PanelGrid::from_text(&NAMES, &grid.to_text(&NAMES));
    assert_eq!(restored.places(), grid.places());
    // Renormalising may move a position by a rounding error, no more.
    for (a, b) in restored.cells(AREA, GAP).iter().zip(grid.cells(AREA, GAP)) {
        assert!(a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01), "{a:?} vs {b:?}");
    }
}

#[test]
fn a_grid_saved_by_v0_2_0_is_ignored_and_the_default_used() {
    let v0_2_0 = "panels 14\nrows 0.33333334 0.33333334 0.33333334\ncolumns 0.2 0.2 0.2 0.2 0.2\ncolumns 0.2 0.2 0.2 0.2 0.2\ncolumns 0.25 0.25 0.25 0.25\n";
    assert_eq!(PanelGrid::from_text(&NAMES, v0_2_0), default_grid());
}

#[test]
fn rubbish_is_ignored_and_the_default_used() {
    let default = default_grid();
    assert_eq!(PanelGrid::from_text(&NAMES, ""), default);
    assert_eq!(PanelGrid::from_text(&NAMES, "not a layout"), default);
    let good = default.to_text(&NAMES);
    // A Panel outside the grid, a Panel named twice, two Panels in one cell.
    assert_eq!(PanelGrid::from_text(&NAMES, &good.replace("panel 0 0 3 2 Clean picture", "panel 4 3 3 2 Clean picture")), default);
    assert_eq!(PanelGrid::from_text(&NAMES, &(good.clone() + "panel 0 0 1 1 Skin tone\n")), default);
    assert_eq!(PanelGrid::from_text(&NAMES, &good.replace("panel 3 3 1 1 Skin tone", "panel 4 3 1 1 Skin tone")), default);
}

#[test]
fn saved_positions_that_leave_a_column_or_row_too_small_are_ignored() {
    let good = default_grid().to_text(&NAMES);
    let columns = good.lines().nth(1).unwrap();
    assert_eq!(PanelGrid::from_text(&NAMES, &good.replace(columns, "columns 0.96 0.01 0.01 0.01 0.01")), default_grid());
    let rows = good.lines().nth(2).unwrap();
    assert_eq!(PanelGrid::from_text(&NAMES, &good.replace(rows, "rows 0.97 0.01 0.01 0.01")), default_grid());
}

#[test]
fn saved_positions_that_are_a_little_off_are_scaled_to_add_up_to_one() {
    let good = default_grid().to_text(&NAMES);
    let columns = good.lines().nth(1).unwrap();
    let grid = PanelGrid::from_text(&NAMES, &good.replace(columns, "columns 0.2 0.2 0.2 0.2 0.205"));
    assert_fills(&grid);
}

#[test]
fn a_saved_panel_that_no_longer_exists_is_dropped() {
    let text = default_grid().to_text(&NAMES) + "panel 0 0 1 1 Gamut/legal warning\n";
    let grid = PanelGrid::from_text(&NAMES, &text);
    assert_eq!(grid, default_grid());
}

#[test]
fn a_panel_missing_from_a_saved_layout_takes_the_first_free_cell_or_a_new_row() {
    // Saved without the heatmap: it takes the free cell. A display added too has none left, so it gets a new row.
    let mut names: Vec<&str> = NAMES.to_vec();
    names.push("New display");
    let text = default_grid().to_text(&NAMES).replace("panel 2 3 1 1 Saturation heatmap\n", "");
    let grid = PanelGrid::from_text(&names, &text);
    let heatmap = grid.places()[5];
    assert_eq!((heatmap.column, heatmap.row, heatmap.columns, heatmap.rows), (2, 3, 1, 1), "the free cell");
    let new = grid.places()[8];
    assert_eq!((new.column, new.row, new.columns), (0, 4, 5), "a new row of its own");
    assert_eq!(grid.size(), (5, 5));
    assert_fills(&grid);
}

#[test]
fn a_layout_that_merely_lost_a_panel_still_recalls_with_the_hole_closed() {
    // Saved with 8 Panels, recalled by a build without the Channel view: its cell is taken up by a neighbour.
    let text = default_grid().to_text(&NAMES);
    let names: Vec<&str> = NAMES.iter().copied().filter(|n| *n != "Channel view").collect();
    let grid = PanelGrid::from_text(&names, &text);
    assert_eq!(grid.places().len(), 7);
    assert_ne!(grid, PanelGrid::default_for(&names), "the saved layout is recalled, not the default");
    assert_fills(&grid);
    // The Panels that were not next to the hole stay where they were.
    let clean = grid.places()[0];
    assert_eq!((clean.column, clean.row, clean.columns, clean.rows), (0, 0, 3, 2));
}

// ------------------------------------------------------------------ inside a Panel

#[test]
fn a_panel_has_a_border_a_title_bar_at_its_top_and_the_display_under_it() {
    let p = PanelLayout::new([100.0, 50.0, 400.0, 300.0], 1.0);
    assert_eq!(p.frame, [100.0, 50.0, 400.0, 300.0]);
    assert_eq!(p.title, [101.0, 51.0, 398.0, 22.0]);
    // Under the title bar and the line beneath it, inside the border.
    assert_eq!(p.content, [101.0, 74.0, 398.0, 275.0]);
    // There is no label strip under the display: it reaches the bottom border.
    assert_eq!(p.content[1] + p.content[3], 50.0 + 300.0 - 1.0);
}

#[test]
fn the_border_is_thin_lines_round_the_panel_and_one_under_the_title_bar() {
    let p = PanelLayout::new([0.0, 0.0, 200.0, 100.0], 1.0);
    let lines = p.lines(1.0);
    assert_eq!(lines[0], [0.0, 0.0, 200.0, 1.0]);
    assert_eq!(lines[1], [0.0, 99.0, 200.0, 1.0]);
    assert_eq!(lines[2], [0.0, 0.0, 1.0, 100.0]);
    assert_eq!(lines[3], [199.0, 0.0, 1.0, 100.0]);
    assert_eq!(lines[4], [1.0, 23.0, 198.0, 1.0]);
}

#[test]
fn at_the_smallest_panel_size_the_display_and_title_bar_still_have_room() {
    for scale in [1.0, 2.0] {
        let min = min_panel_size(scale);
        let p = PanelLayout::new([0.0, 0.0, min[0], min[1]], scale);
        assert!(p.content[2] >= 100.0 * scale && p.content[3] >= 80.0 * scale - 0.01, "scale {scale}: {p:?}");
        assert!(p.title[3] >= 22.0 * scale - 0.01, "a whole line fits in the title bar");
        assert!(p.content[1] + p.content[3] <= min[1] + 0.01, "the display is inside the panel at scale {scale}");
    }
}

#[test]
fn at_a_scale_of_2_everything_is_twice_the_size() {
    let one = PanelLayout::new([0.0, 0.0, 400.0, 300.0], 1.0);
    let two = PanelLayout::new([0.0, 0.0, 800.0, 600.0], 2.0);
    assert_eq!(two.title[3], 2.0 * one.title[3]);
    assert_eq!(two.content[3], 2.0 * one.content[3]);
    assert_eq!(min_panel_size(2.0), min_panel_size(1.0).map(|v| v * 2.0));
}

#[test]
fn a_panel_smaller_than_its_title_bar_still_gives_the_display_a_pixel() {
    let p = PanelLayout::new([0.0, 0.0, 50.0, 10.0], 1.0);
    assert!(p.content[2] >= 1.0 && p.content[3] >= 1.0);
}

#[test]
fn the_vectorscope_is_two_cells_wide_and_borders_do_not_run_through_it() {
    let grid = default_grid();
    let one = cell_of(&grid, "Skin tone");
    let scope = cell_of(&grid, "Vectorscope");
    let luma = cell_of(&grid, "Luma waveform");
    assert!((scope[2] - (2.0 * one[2] + GAP)).abs() < 0.01, "two columns wide: {scope:?}");
    assert_eq!(scope[2], luma[2], "as wide as the luma waveform above it");
    assert_eq!(grid.border_at(AREA, GAP, [scope[0] + scope[2] / 2.0 - 0.0, scope[1] + 50.0], 4.0), None, "no border through its middle");
    assert_fills(&grid);
}

#[test]
fn a_layout_saved_with_the_colour_waveform_still_recalls_without_it() {
    // Saved while the Window had 9 Panels (the Colour Waveform in the 2 x 2 at the left): the others stay where
    // they were, and the cells the Colour Waveform leaves are closed up by a neighbour (the #98 rule).
    let nine = "layout 1\ncolumns 0.2 0.2 0.2 0.2 0.2\nrows 0.28 0.28 0.22 0.22\npanel 0 0 3 2 Clean picture\npanel 3 0 2 1 Luma waveform\npanel 3 1 2 1 Vectorscope\npanel 0 2 2 2 Colour waveform\npanel 2 2 1 1 Channel view\npanel 3 2 2 1 ARRI False Colour\npanel 2 3 1 1 Saturation heatmap\npanel 3 3 1 1 Skin tone\npanel 4 3 1 1 Stops False Colour\n";
    let grid = PanelGrid::from_text(&NAMES, nine);
    assert_ne!(grid, default_grid(), "the saved layout is recalled, not the default");
    assert_eq!(grid.places().len(), 8);
    assert_fills(&grid);
    let clean = grid.places()[0];
    assert_eq!((clean.column, clean.row, clean.columns, clean.rows), (0, 0, 3, 2));
}

#[test]
fn a_grid_saved_with_the_two_milestone_2_vectorscope_panels_is_ignored_and_the_default_used() {
    let old = "layout 1\ncolumns 0.2 0.2 0.2 0.2 0.2\nrows 0.28 0.28 0.22 0.22\npanel 0 0 3 2 Clean picture\npanel 3 0 2 1 Luma waveform\npanel 3 1 1 1 Vectorscope 100 %\npanel 4 1 1 1 Vectorscope 200 %\npanel 0 2 2 2 Colour waveform\npanel 2 2 1 1 Channel view\npanel 3 2 1 1 ARRI False Colour\npanel 4 2 1 1 Black and white\npanel 2 3 1 1 Saturation heatmap\npanel 3 3 1 1 Skin tone\npanel 4 3 1 1 Stops False Colour\n";
    assert_eq!(PanelGrid::from_text(&NAMES, old), default_grid());
}

// ------------------------------------------------------------------ swapping Panels (#97)

fn title_point(grid: &PanelGrid, name: &str) -> [f32; 2] {
    let t = PanelLayout::new(cell_of(grid, name), 1.0).title;
    [t[0] + t[2] / 2.0, t[1] + t[3] / 2.0]
}

#[test]
fn swapping_two_panels_gives_each_the_others_cell_and_span() {
    let mut grid = default_grid();
    let (hero, skin) = (cell_of(&grid, "Clean picture"), cell_of(&grid, "Skin tone"));
    let (a, b) = (0, NAMES.iter().position(|n| *n == "Skin tone").unwrap());
    grid.swap(a, b);
    assert_eq!(cell_of(&grid, "Clean picture"), skin);
    assert_eq!(cell_of(&grid, "Skin tone"), hero);
    assert_fills(&grid);
}

#[test]
fn swapping_a_panel_with_itself_changes_nothing() {
    let mut grid = default_grid();
    grid.swap(3, 3);
    assert_eq!(grid, default_grid());
}

#[test]
fn a_swap_is_remembered_between_sessions() {
    let mut grid = default_grid();
    grid.swap(0, 7);
    let back = PanelGrid::from_text(&NAMES, &grid.to_text(&NAMES));
    assert_eq!(back, grid);
    assert_ne!(back, default_grid());
}

#[test]
fn a_press_on_a_title_bar_starts_a_panel_drag() {
    let grid = default_grid();
    let skin = NAMES.iter().position(|n| *n == "Skin tone").unwrap();
    assert_eq!(grid.press_at(AREA, GAP, title_point(&grid, "Skin tone"), 3.0, 1.0), Some(Press::Panel(skin)));
}

#[test]
fn a_press_on_a_display_starts_nothing() {
    let grid = default_grid();
    let c = cell_of(&grid, "Skin tone");
    assert_eq!(grid.press_at(AREA, GAP, [c[0] + c[2] / 2.0, c[1] + c[3] - 10.0], 3.0, 1.0), None);
}

#[test]
fn a_press_in_a_gap_starts_a_border_drag_not_a_panel_drag() {
    let grid = default_grid();
    let c = cell_of(&grid, "ARRI False Colour");
    let level = c[1] + 10.0; // the height of ARRI False Colour's title bar
    assert!(matches!(grid.press_at(AREA, GAP, [c[0] - GAP / 2.0, level], 3.0, 1.0), Some(Press::Border(Border::Column { .. }))));
    assert!(matches!(grid.press_at(AREA, GAP, [c[0] + 20.0, level], 3.0, 1.0), Some(Press::Panel(_))));
}

#[test]
fn the_panel_under_a_point_is_found_for_a_drop() {
    let grid = default_grid();
    let c = cell_of(&grid, "Stops False Colour");
    let stops = NAMES.iter().position(|n| *n == "Stops False Colour").unwrap();
    assert_eq!(grid.panel_at(AREA, GAP, [c[0] + c[2] / 2.0, c[1] + c[3] / 2.0]), Some(stops));
    assert_eq!(grid.panel_at(AREA, GAP, [c[0] - GAP / 2.0, c[1] + 5.0]), None, "a gap is no Panel");
    assert_eq!(grid.panel_at(AREA, GAP, [0.0, 0.0]), None, "outside the area");
}
