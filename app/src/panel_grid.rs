//! What sits inside one Panel: a thin border round it, a title bar at its top holding the label, and the display
//! filling the rest ([`PanelLayout`]); and the cells while one Panel fills the Window. Where the Panels go is the
//! layout tree ([`crate::layout_tree`], ADR 0038). Pure: no GPU, no window, no fonts. Sizes are in device pixels;
//! rectangles are left, top, width, height.

use crate::layout_tree::LayoutTree;
use crate::style;

/// Logical pixels: the label's text size, and the title bar's height.
pub const LABEL_SIZE: f32 = style::TYPE_LABEL;
pub const TITLE_BAR: f32 = style::SPACE_TITLE_BAR;
/// The least height a display keeps, in logical pixels.
pub const MIN_DISPLAY: f32 = style::SPACE_MIN_DISPLAY;
/// The smallest a Panel can be dragged to, in logical pixels (width, height): the display keeps [`MIN_DISPLAY`]
/// under a title bar, inside the border.
pub const MIN_PANEL: [f32; 2] = [style::SPACE_MIN_PANEL_WIDTH, style::SPACE_MIN_PANEL_HEIGHT];

/// The smallest a Panel can be at `scale` device pixels per logical pixel.
pub fn min_panel_size(scale: f32) -> [f32; 2] {
    MIN_PANEL.map(|v| v * scale)
}

/// What sits inside one Panel: a thin border round it, a title bar at its top holding the label, and the display
/// filling the rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelLayout {
    /// The Panel's whole cell: the border is drawn round its edge.
    pub frame: [f32; 4],
    /// The title bar, inside the border.
    pub title: [f32; 4],
    /// The display, under the title bar and inside the border (at least a pixel).
    pub content: [f32; 4],
}

/// The cells while Panel `full` fills the Window (Ctrl+F, #180): it has the whole `area`, and every other Panel an
/// empty cell far off screen, so it is never drawn or hit. `None` gives the grid's own cells.
pub fn cells_filled(
    grid: &LayoutTree,
    area: [f32; 4],
    gap: f32,
    full: Option<usize>,
) -> Vec<[f32; 4]> {
    let cells = grid.cells(area, gap);
    match full.filter(|f| *f < cells.len()) {
        None => cells,
        Some(full) => (0..cells.len())
            .map(|i| if i == full { area } else { off_screen() })
            .collect(),
    }
}

/// Where a Panel that is not shown is: no size, far outside any window.
pub(crate) fn off_screen() -> [f32; 4] {
    let far = -f32::from(u16::MAX);
    [far, far, 0.0, 0.0]
}

/// Whether a cell from [`cells_filled`] is shown.
pub fn shown(cell: [f32; 4]) -> bool {
    cell[2] > 0.0 && cell[3] > 0.0
}

impl PanelLayout {
    /// The layout inside `cell` at `scale` device pixels per logical pixel. The title bar is one line high and the
    /// label is clipped to it, so a long label never takes room from the display.
    pub fn new(cell: [f32; 4], scale: f32) -> PanelLayout {
        let [left, top, width, height] = cell;
        let border = style::LINE_WEIGHT * scale;
        let bar = TITLE_BAR * scale;
        let inner_width = (width - 2.0 * border).max(1.0);
        PanelLayout {
            frame: cell,
            title: [left + border, top + border, inner_width, bar],
            content: [
                left + border,
                top + border + bar + border,
                inner_width,
                (height - 3.0 * border - bar).max(1.0),
            ],
        }
    }

    /// The border as four thin rectangles (top, bottom, left, right) of `weight` device pixels, and the line under
    /// the title bar: the rectangles the window draws for this Panel.
    pub fn lines(&self, weight: f32) -> [[f32; 4]; 5] {
        let [left, top, width, height] = self.frame;
        let [title_left, title_top, title_width, title_height] = self.title;
        [
            [left, top, width, weight],
            [left, top + height - weight, width, weight],
            [left, top, weight, height],
            [left + width - weight, top, weight, height],
            [title_left, title_top + title_height, title_width, weight],
        ]
    }
}
