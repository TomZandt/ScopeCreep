//! The Panel grid at the pure layout seam (#50): where each Panel goes for any number of displays, dragging a
//! border, remembering border positions, and what sits inside a Panel (the display, its label) at small sizes and
//! on a HiDPI screen. Rectangles are left, top, width, height in device pixels.
use scopecreep::panel_grid::{Border, PanelGrid, PanelLayout, min_panel_size};

const AREA: [f32; 4] = [12.0, 12.0, 1256.0, 700.0];
const GAP: f32 = 12.0;

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] - 0.01 && b[0] < a[0] + a[2] - 0.01 && a[1] < b[1] + b[3] - 0.01 && b[1] < a[1] + a[3] - 0.01
}

/// Every Panel is inside the area, none overlaps another, and together they fill it (less the gaps).
fn assert_fills(grid: &PanelGrid, n: usize) {
    let cells = grid.cells(AREA, GAP);
    assert_eq!(cells.len(), n);
    for (i, c) in cells.iter().enumerate() {
        assert!(c[2] >= 1.0 && c[3] >= 1.0, "panel {i} has a size: {c:?}");
        assert!(c[0] >= AREA[0] - 0.01 && c[1] >= AREA[1] - 0.01, "panel {i} inside: {c:?}");
        assert!(c[0] + c[2] <= AREA[0] + AREA[2] + 0.01 && c[1] + c[3] <= AREA[1] + AREA[3] + 0.01, "panel {i} inside: {c:?}");
        for (j, d) in cells.iter().enumerate().skip(i + 1) {
            assert!(!overlap(*c, *d), "panels {i} and {j} overlap: {c:?} {d:?}");
        }
    }
    let bottom = cells.iter().map(|c| c[1] + c[3]).fold(0.0, f32::max);
    assert!((bottom - (AREA[1] + AREA[3])).abs() < 0.01, "fills the height");
    // Each row fills the width.
    let mut start = 0;
    for &len in grid.rows() {
        let last = cells[start + len - 1];
        assert!((last[0] + last[2] - (AREA[0] + AREA[2])).abs() < 0.01, "row fills the width: {last:?}");
        assert!((cells[start][0] - AREA[0]).abs() < 0.01);
        start += len;
    }
}

#[test]
fn two_panels_sit_side_by_side_and_fill_the_window() {
    let grid = PanelGrid::new(2);
    assert_eq!(grid.rows(), [2]);
    assert_fills(&grid, 2);
    let c = grid.cells(AREA, GAP);
    assert_eq!(c[0], [12.0, 12.0, 622.0, 700.0]);
    assert_eq!(c[1], [646.0, 12.0, 622.0, 700.0]);
}

#[test]
fn fourteen_panels_fill_the_window_in_reading_order() {
    let grid = PanelGrid::new(14);
    assert_eq!(grid.rows(), [5, 5, 4]);
    assert_fills(&grid, 14);
    let c = grid.cells(AREA, GAP);
    // Reading order: left to right, then the next row down.
    for i in 1..14 {
        assert!(c[i][1] > c[i - 1][1] - 0.01, "never back up a row");
        if (c[i][1] - c[i - 1][1]).abs() < 0.01 {
            assert!(c[i][0] > c[i - 1][0]);
        }
    }
}

#[test]
fn any_number_of_displays_gets_a_grid_that_fills() {
    for n in 1..=20 {
        let grid = PanelGrid::new(n);
        assert_eq!(grid.rows().iter().sum::<usize>(), n);
        assert_fills(&grid, n);
    }
}

#[test]
fn dragging_a_border_resizes_the_panels_on_each_side() {
    let mut grid = PanelGrid::new(14);
    let before = grid.cells(AREA, GAP);
    // The border between panels 0 and 1 is in the gap right of panel 0.
    let x = before[0][0] + before[0][2] + GAP / 2.0;
    let y = before[0][1] + before[0][3] / 2.0;
    let border = grid.border_at(AREA, GAP, [x, y], 4.0).expect("a border is there");
    assert_eq!(border, Border::Column { row: 0, after: 0 });
    grid.drag(border, [x + 100.0, y], AREA, GAP, [60.0, 60.0]);
    let after = grid.cells(AREA, GAP);
    assert!((after[0][2] - (before[0][2] + 100.0)).abs() < 0.5, "left grows");
    assert!((after[1][2] - (before[1][2] - 100.0)).abs() < 0.5, "right shrinks");
    assert!((after[1][0] + after[1][2] - (before[1][0] + before[1][2])).abs() < 0.01, "the far edge stays");
    assert_eq!(after[5], before[5], "the next row is untouched");
    assert_fills(&grid, 14);
}

#[test]
fn dragging_a_row_border_resizes_the_rows_above_and_below() {
    let mut grid = PanelGrid::new(14);
    let before = grid.cells(AREA, GAP);
    let y = before[0][1] + before[0][3] + GAP / 2.0;
    let x = before[2][0] + before[2][2] / 2.0;
    let border = grid.border_at(AREA, GAP, [x, y], 4.0).unwrap();
    assert_eq!(border, Border::Row { after: 0 });
    grid.drag(border, [x, y - 50.0], AREA, GAP, [60.0, 60.0]);
    let after = grid.cells(AREA, GAP);
    assert!((after[0][3] - (before[0][3] - 50.0)).abs() < 0.5);
    assert!((after[5][3] - (before[5][3] + 50.0)).abs() < 0.5);
    assert_fills(&grid, 14);
}

#[test]
fn a_panel_cannot_be_dragged_smaller_than_its_minimum() {
    let mut grid = PanelGrid::new(2);
    let border = Border::Column { row: 0, after: 0 };
    grid.drag(border, [5000.0, 300.0], AREA, GAP, [80.0, 80.0]);
    let c = grid.cells(AREA, GAP);
    assert!(c[1][2] >= 79.99, "right panel keeps its minimum: {:?}", c[1]);
    grid.drag(border, [-5000.0, 300.0], AREA, GAP, [80.0, 80.0]);
    let c = grid.cells(AREA, GAP);
    assert!(c[0][2] >= 79.99, "left panel keeps its minimum: {:?}", c[0]);
    assert_fills(&grid, 2);
}

#[test]
fn a_point_inside_a_panel_is_not_on_a_border() {
    let grid = PanelGrid::new(2);
    assert_eq!(grid.border_at(AREA, GAP, [300.0, 300.0], 4.0), None);
}

#[test]
fn border_positions_survive_a_restart() {
    let mut grid = PanelGrid::new(14);
    grid.drag(Border::Column { row: 1, after: 2 }, [700.0, 300.0], AREA, GAP, [60.0, 60.0]);
    grid.drag(Border::Row { after: 1 }, [300.0, 500.0], AREA, GAP, [60.0, 60.0]);
    assert_ne!(grid.cells(AREA, GAP), PanelGrid::new(14).cells(AREA, GAP), "the drags moved something");
    let restored = PanelGrid::from_text(14, &grid.to_text());
    assert_eq!(restored.cells(AREA, GAP), grid.cells(AREA, GAP));
}

#[test]
fn saved_positions_for_another_number_of_displays_or_rubbish_are_ignored() {
    let mut grid = PanelGrid::new(14);
    grid.drag(Border::Row { after: 0 }, [300.0, 500.0], AREA, GAP, [60.0, 60.0]);
    let text = grid.to_text();
    assert_eq!(PanelGrid::from_text(13, &text).cells(AREA, GAP), PanelGrid::new(13).cells(AREA, GAP));
    assert_eq!(PanelGrid::from_text(14, "not a layout").cells(AREA, GAP), PanelGrid::new(14).cells(AREA, GAP));
    assert_eq!(PanelGrid::from_text(14, "").cells(AREA, GAP), PanelGrid::new(14).cells(AREA, GAP));
}

// ------------------------------------------------------------------ inside a Panel

#[test]
fn the_label_sits_under_the_display_inside_the_panel() {
    let p = PanelLayout::new([100.0, 50.0, 400.0, 300.0], 1.0, 1);
    assert_eq!(p.content, [100.0, 50.0, 400.0, 270.0]);
    assert_eq!(p.label, [100.0, 320.0, 400.0, 30.0]);
}

#[test]
fn a_wrapped_label_makes_room_for_its_extra_lines() {
    let one = PanelLayout::new([0.0, 0.0, 400.0, 300.0], 1.0, 1);
    let three = PanelLayout::new([0.0, 0.0, 400.0, 300.0], 1.0, 3);
    assert!((one.content[3] - three.content[3] - 2.0 * 12.5 * 1.4).abs() < 0.01);
}

#[test]
fn at_the_smallest_panel_size_the_display_and_label_still_have_room() {
    for scale in [1.0, 2.0] {
        let min = min_panel_size(scale);
        let p = PanelLayout::new([0.0, 0.0, min[0], min[1]], scale, 1);
        assert!(p.content[2] >= 40.0 * scale && p.content[3] >= 40.0 * scale, "scale {scale}: {p:?}");
        assert!(p.label[1] + p.label[3] <= min[1] + 0.01, "the label is inside the panel at scale {scale}");
        assert!(p.label[3] >= 30.0 * scale - 0.01, "a whole line fits");
    }
}

#[test]
fn at_a_scale_of_2_everything_is_twice_the_size() {
    let one = PanelLayout::new([0.0, 0.0, 400.0, 300.0], 1.0, 1);
    let two = PanelLayout::new([0.0, 0.0, 800.0, 600.0], 2.0, 1);
    assert_eq!(two.label[3], 2.0 * one.label[3]);
    assert_eq!(two.content[3], 600.0 - 60.0);
    assert_eq!(min_panel_size(2.0), min_panel_size(1.0).map(|v| v * 2.0));
}

#[test]
fn a_panel_smaller_than_its_label_still_gives_the_display_a_pixel() {
    let p = PanelLayout::new([0.0, 0.0, 50.0, 10.0], 1.0, 1);
    assert!(p.content[2] >= 1.0 && p.content[3] >= 1.0);
}

#[test]
fn the_14_panel_grid_at_default_window_size_keeps_every_display_usable() {
    for cell in PanelGrid::new(14).cells(AREA, GAP) {
        let p = PanelLayout::new(cell, 1.0, 1);
        assert!(p.content[2] >= 100.0 && p.content[3] >= 100.0, "{cell:?} -> {p:?}");
    }
}
