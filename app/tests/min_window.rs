//! The Window's minimum size (#172) at the pure seam: at it, every Panel of the default layout is at least its
//! minimum and every menu fits under the menu bar, at 1x and 2x.
use scopecreep::components::{BarKind, MenuBar};
use scopecreep::layout::{MARGIN, min_window};
use scopecreep::menu_bar::{self, largest_menus};
use scopecreep::panel_grid::{PanelGrid, PanelLayout, min_panel_size};
use scopecreep::style;

const NAMES: [&str; 7] = [
    "Clean picture",
    "Luma waveform",
    "Vectorscope",
    "ARRI False Colour",
    "Stops False Colour",
    "Saturation heatmap",
    "Skin tone",
];

fn minimum() -> [f32; 2] {
    min_window(&PanelGrid::default_for(&NAMES), &largest_menus())
}

/// The Panel area the window draws in a window of `size` device pixels, with the status row at its tallest (a
/// notice above the status line).
fn area(size: [f32; 2], scale: f32) -> [f32; 4] {
    let top = MenuBar::height(scale) + BarKind::Top.height(scale) + MARGIN * scale;
    [
        MARGIN * scale,
        top,
        size[0] - 2.0 * MARGIN * scale,
        size[1] - 2.0 * style::SPACE_STATUS_ROW * scale - top,
    ]
}

#[test]
fn the_minimum_is_smaller_than_the_default_window() {
    let [w, h] = minimum();
    assert!(w < 1280.0 && h < 760.0, "{w} x {h}");
    assert!(w > 0.0 && h > 0.0);
}

#[test]
fn at_the_minimum_every_panel_of_the_default_layout_is_at_least_its_minimum() {
    let grid = PanelGrid::default_for(&NAMES);
    for scale in [1.0, 1.5, 2.0] {
        let size = minimum().map(|v| v * scale);
        let min = min_panel_size(scale);
        for (name, cell) in NAMES
            .iter()
            .zip(grid.cells(area(size, scale), MARGIN * scale))
        {
            assert!(
                cell[2] >= min[0] - 0.01 && cell[3] >= min[1] - 0.01,
                "{name} at {scale}x is {cell:?}, under {min:?}"
            );
            let p = PanelLayout::new(cell, scale);
            assert!(
                p.content[3] >= style::SPACE_MIN_DISPLAY * scale - 0.01,
                "{name}: {p:?}"
            );
        }
    }
}

#[test]
fn the_minimum_is_tight_so_it_is_no_bigger_than_it_needs_to_be() {
    // Shrink the Panel area by a pixel either way and some Panel is under its minimum (unless a menu set it).
    let grid = PanelGrid::default_for(&NAMES);
    let need = grid.min_area(MARGIN, min_panel_size(1.0));
    let min = min_panel_size(1.0);
    let under = |a: [f32; 4]| {
        grid.cells(a, MARGIN)
            .iter()
            .any(|c| c[2] < min[0] - 0.01 || c[3] < min[1] - 0.01)
    };
    assert!(!under([0.0, 0.0, need[0], need[1]]));
    assert!(under([0.0, 0.0, need[0] - 1.0, need[1]]));
    assert!(under([0.0, 0.0, need[0], need[1] - 1.0]));
}

#[test]
fn at_the_minimum_every_menu_fits_under_the_bar() {
    for scale in [1.0, 2.0] {
        let window = minimum().map(|v| v * scale);
        for title in menu_bar::bar().title_rects(0.0, 0.0, scale) {
            for menu in largest_menus() {
                let r = menu.rect(title, window, scale);
                assert!(
                    r[0] >= 0.0 && r[0] + r[2] <= window[0] + 0.01,
                    "across: {r:?} in {window:?}"
                );
                assert!(
                    r[1] >= MenuBar::height(scale) - 0.01 && r[1] + r[3] <= window[1] + 0.01,
                    "down: {r:?} in {window:?}"
                );
            }
        }
    }
}

#[test]
fn the_largest_menus_cover_every_menu_and_the_hotkeys_panel_with_a_conflict() {
    let menus = largest_menus();
    assert!(
        menus.len() >= 5,
        "three Layout modes, Settings, the Hotkeys panel"
    );
    let tallest = menus.iter().map(|m| m.size(1.0)[1]).fold(0.0, f32::max);
    let [_, h] = minimum();
    assert!(MenuBar::height(1.0) + tallest <= h);
}
