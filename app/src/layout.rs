//! Sizes shared by the Window's parts. The Panels' grid is in [`crate::panel_grid`].

/// Logical pixels: around the window's edge and between the Panels.
pub const MARGIN: f32 = crate::style::SPACE_MARGIN;
/// The gutter for the waveform scale's numbers ([`crate::waveform::PlotLayout`] uses the same width).
pub const GUTTER: f32 = crate::style::SPACE_GUTTER;

use crate::components::{DropMenu, MenuBar};
use crate::panel_grid::{PanelGrid, min_panel_size};
use crate::style;
use crate::view_strip::ViewStrip;

/// The smallest the Window can be made (#172), in logical pixels: the default layout of `grid` with every Panel at
/// least its minimum size, under the menu bar, the Colour Setup line and a margin and over a two-line status row
/// (a notice can take the second); room for the menu bar row's titles and the view strip (ADR 0040); and room under the menu bar for the largest of `menus`, so no menu is cut off.
/// A layout the user dragged is not part of it: making the Window smaller than that layout was arranged at shrinks
/// its Panels with it.
pub fn min_window(grid: &PanelGrid, menus: &[DropMenu]) -> [f32; 2] {
    let area = grid.min_area(MARGIN, min_panel_size(1.0));
    let chrome = MenuBar::height(1.0)
        + crate::components::BarKind::Top.height(1.0)
        + MARGIN
        + 2.0 * style::SPACE_STATUS_ROW;
    let (menu_width, menu_height) = menus
        .iter()
        .map(|m| m.size(1.0))
        .fold((0.0, 0.0), |(w, h), [mw, mh]| {
            (f32::max(w, mw), f32::max(h, mh))
        });
    // The menu bar row holds the menu titles and, at its right end, the view strip (ADR 0040).
    let titles_right = crate::menu_bar::bar()
        .title_rects(0.0, 0.0, 1.0)
        .last()
        .map_or(0.0, |r| r[0] + r[2]);
    let bar_width = titles_right + ViewStrip::width(1.0) + style::SPACE_BAR_PAD;
    [
        (area[0] + 2.0 * MARGIN)
            .max(menu_width)
            .max(bar_width)
            .ceil(),
        (area[1] + chrome)
            .max(MenuBar::height(1.0) + menu_height)
            .ceil(),
    ]
}
