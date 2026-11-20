//! What sits inside one Panel at the pure seam (#96): its border, title bar and display at small sizes and on a
//! HiDPI screen, and the cells while one Panel fills the Window (Ctrl+F, #180). Where the Panels go is
//! `layout_tree.rs`. Rectangles are left, top, width, height in device pixels.
use scopecreep::layout_tree::LayoutTree;
use scopecreep::panel_grid::{PanelLayout, cells_filled, min_panel_size, shown};

const AREA: [f32; 4] = [12.0, 12.0, 1256.0, 716.0];
const GAP: f32 = 12.0;

const NAMES: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];

fn default_grid() -> LayoutTree {
    LayoutTree::default_for(&NAMES)
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
        assert!(
            p.content[2] >= 100.0 * scale && p.content[3] >= 80.0 * scale - 0.01,
            "scale {scale}: {p:?}"
        );
        assert!(
            p.title[3] >= 22.0 * scale - 0.01,
            "a whole line fits in the title bar"
        );
        assert!(
            p.content[1] + p.content[3] <= min[1] + 0.01,
            "the display is inside the panel at scale {scale}"
        );
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

// ---- A Panel filling the Window (Ctrl+F, #180)

#[test]
fn a_filling_panel_takes_the_whole_area_and_the_others_none() {
    let grid = default_grid();
    for full in 0..NAMES.len() {
        let cells = cells_filled(&grid, AREA, GAP, Some(full));
        assert_eq!(cells.len(), NAMES.len());
        for (i, cell) in cells.iter().enumerate() {
            if i == full {
                assert_eq!(*cell, AREA);
                assert!(shown(*cell));
            } else {
                assert!(!shown(*cell), "Panel {i} is hidden");
                let layout = PanelLayout::new(*cell, 1.0);
                for rect in [layout.title, layout.content] {
                    assert!(
                        rect[0] + rect[2] < 0.0 && rect[1] + rect[3] < 0.0,
                        "Panel {i} can never be hit inside the window"
                    );
                }
            }
        }
    }
}

#[test]
fn with_no_panel_filling_the_window_the_grid_is_as_it_was() {
    let grid = default_grid();
    assert_eq!(cells_filled(&grid, AREA, GAP, None), grid.cells(AREA, GAP));
    assert!(grid.cells(AREA, GAP).iter().all(|c| shown(*c)));
    assert_eq!(
        cells_filled(&grid, AREA, GAP, Some(99)),
        grid.cells(AREA, GAP),
        "a Panel that is not there fills nothing"
    );
}
