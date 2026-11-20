//! Auto resize (#213, spec #196, ADR 0041): fit the split sizes of a page to the footage. Pure functions of the
//! footage's aspect, so Auto layout (#214) and the page templates (#215) can use them too.
//!
//! A cell is picture-shaped when its shown Panel is a Picture View or the luma waveform ([`Shape`]). Where a split
//! has exactly one picture-shaped side (a single tab group, so a Panel and not a pile of them) its ratio is set
//! so that side's cell leaves room for a picture of the footage's aspect once the title bar, the legend strip and
//! (for the waveform) the scale gutter are taken off. The tree walk is [`crate::layout_tree::LayoutTree::auto_resize`].

use crate::panel_grid::PanelLayout;
use crate::style;

/// The aspect with no Feed: 16:9.
pub const DEFAULT_ASPECT: f32 = 16.0 / 9.0;
/// The ratio of a fitted split stays inside these shares.
pub const MIN_SHARE: f32 = 0.15;
pub const MAX_SHARE: f32 = 0.85;

/// What a Panel's cell holds, as far as Auto resize and Auto layout care.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A Picture View: the picture fills the content area under the legend strip.
    Picture,
    /// The luma waveform: its plot has the frame's aspect, with a scale gutter on the left.
    Waveform,
    /// Anything else (the Vectorscope, the colour waveform, the CIE chart): not picture-shaped.
    Other,
}

impl Shape {
    pub fn is_picture(self) -> bool {
        self != Shape::Other
    }
}

/// The shape of the display called `name`.
pub fn shape_of(name: &str) -> Shape {
    match name {
        n if n == crate::waveform::NAME => Shape::Waveform,
        "Channel Viewer"
        | "False Colour - Arri"
        | "Channel view"
        | "Black and white"
        | "Neutrals"
        | "Gamut Warning"
        | "Saturation"
        | "Skin Tone"
        | crate::stops_false_colour::NAME => Shape::Picture,
        _ => Shape::Other,
    }
}

/// The footage's aspect from the Feed frame's size; 16:9 with no Feed (no frame, or one with no size).
pub fn aspect_of(frame: Option<[u32; 2]>) -> f32 {
    match frame {
        Some([w, h]) if w > 0 && h > 0 => w as f32 / h as f32,
        _ => DEFAULT_ASPECT,
    }
}

/// What a `shape` takes from its Panel's content area besides the picture, at `scale`: the width of the scale
/// gutter and the height of the legend strip.
pub fn legend_allowance(shape: Shape, scale: f32) -> (f32, f32) {
    let gutter = if shape == Shape::Waveform {
        style::SPACE_GUTTER * scale
    } else {
        0.0
    };
    (gutter, style::SPACE_LEGEND_HEIGHT * scale)
}

/// The width a cell `height` high needs for a picture of `aspect`, or the height a cell `width` wide needs:
/// `along_width` says which is wanted. Title bar, borders, legend strip and gutter are added on.
pub fn cell_extent(shape: Shape, aspect: f32, other: f32, along_width: bool, scale: f32) -> f32 {
    let (gutter, legend) = legend_allowance(shape, scale);
    // The chrome of a Panel: what its cell has beyond its content area.
    let chrome = {
        let c = PanelLayout::new([0.0, 0.0, 1000.0, 1000.0], scale).content;
        [1000.0 - c[2], 1000.0 - c[3]]
    };
    if along_width {
        (other - chrome[1] - legend) * aspect + gutter + chrome[0]
    } else {
        (other - chrome[0] - gutter) / aspect + legend + chrome[1]
    }
}

/// The ratio of a split with `total` room (net of the gap) in which the picture side needs `needed`, as the share
/// of the first side: `picture_first` says which side is the picture.
pub fn share(needed: f32, total: f32, picture_first: bool) -> f32 {
    let wanted = if total > 0.0 { needed / total } else { 0.5 };
    let wanted = if picture_first { wanted } else { 1.0 - wanted };
    wanted.clamp(MIN_SHARE, MAX_SHARE)
}
