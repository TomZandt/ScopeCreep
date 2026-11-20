//! Auto layout (#214, spec #196, ADR 0041): what the search for the best arrangement of a page weighs. Pure
//! functions of the footage's aspect; the search itself is [`crate::layout_tree::LayoutTree::auto_layout`].
//!
//! The score of an arrangement is the sum over its tab groups of `weight * ln(1 + useful area)`. The weight says
//! how much the colourist wants that display big (the clean picture most), the logarithm keeps one Panel from
//! being fed at the cost of the rest, and the useful area is what of the cell the display really draws with.

use crate::auto_resize::{Shape, legend_allowance, shape_of};
use crate::panel_grid::PanelLayout;

/// Weights (ADR 0041): the clean picture, the luma waveform, the Vectorscope, and every other display.
pub const WEIGHT_CLEAN: f32 = 3.0;
pub const WEIGHT_WAVEFORM: f32 = 1.6;
pub const WEIGHT_VECTORSCOPE: f32 = 1.2;
pub const WEIGHT_OTHER: f32 = 1.0;

/// Up to this many Panels (items, a tab group being one) every way of cutting is tried; above it, and up to
/// [`SEARCH_MAX`], only the first cuts near the middle are.
pub const FULL_SEARCH_MAX: usize = 6;
/// Above this many items the search is replaced by a greedy halving (no search at all).
pub const SEARCH_MAX: usize = 9;

/// What the search needs to know about the display a Panel shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub shape: Shape,
    pub weight: f32,
    /// A round plot (the Vectorscope): its useful area is the inscribed circle.
    pub round: bool,
}

/// The item for the display called `name`.
pub fn item_of(name: &str) -> Item {
    let (weight, round) = match name {
        "Channel Viewer" => (WEIGHT_CLEAN, false),
        n if n == crate::waveform::NAME => (WEIGHT_WAVEFORM, false),
        "Vectorscope" => (WEIGHT_VECTORSCOPE, true),
        _ => (WEIGHT_OTHER, false),
    };
    Item {
        shape: shape_of(name),
        weight,
        round,
    }
}

/// The area of `cell` (a Panel's whole cell) that its display really uses, in square device pixels: the fitted
/// frame less the legend strip for a Picture View, the aspect-true plot less the scale gutter for the luma
/// waveform, the inscribed circle for the Vectorscope, and for any other display its content area.
pub fn useful_area(item: Item, cell: [f32; 4], aspect: f32, scale: f32) -> f32 {
    let content = PanelLayout::new(cell, scale).content;
    let (gutter, legend) = legend_allowance(item.shape, scale);
    if item.round {
        let d = content[2].min(content[3]).max(0.0);
        return std::f32::consts::PI * d * d / 4.0;
    }
    match item.shape {
        Shape::Other => content[2].max(0.0) * content[3].max(0.0),
        Shape::Picture | Shape::Waveform => {
            let (w, h) = (content[2] - gutter, content[3] - legend);
            if w <= 0.0 || h <= 0.0 {
                return 0.0;
            }
            let width = w.min(h * aspect);
            width * width / aspect
        }
    }
}

/// One tab group's share of the score.
pub fn score(item: Item, cell: [f32; 4], aspect: f32, scale: f32) -> f32 {
    item.weight * (1.0 + useful_area(item, cell, aspect, scale)).ln()
}
