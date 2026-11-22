//! Docking by dragging a Panel's title (ADR 0038, #210): when a press becomes a drag, which part of the tab group
//! under the pointer a drop would use, and the rectangle the overlay shows for it. Pure: no GPU, no window.
//! Rectangles are left, top, width, height in device pixels.
use crate::layout_tree::Edge;
use crate::style;

/// Where in a tab group a drop lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// The outer quarter nearest an edge: the dropped Panel splits that edge and takes that half.
    Edge(Edge),
    /// Anywhere else: the dropped Panel becomes a tab of the tab group (#211).
    Middle,
}

impl Zone {
    /// The words the drop overlay carries (ADR 0044): what releasing here does, for the `cell` the pointer is over.
    /// `swap` is Shift held (the Panels trade places), `gap` the gap between Panels and `min` the least Panel size
    /// (device pixels): an edge with no room for the half says it will join as a tab instead.
    pub fn label(self, cell: [f32; 4], gap: f32, min: [f32; 2], swap: bool) -> &'static str {
        if swap {
            return "Swap places";
        }
        let (along, least, words) = match self {
            Zone::Middle => return "Join as a tab",
            Zone::Edge(Edge::Left) => (cell[2], min[0], "Split left"),
            Zone::Edge(Edge::Right) => (cell[2], min[0], "Split right"),
            Zone::Edge(Edge::Top) => (cell[3], min[1], "Split above"),
            Zone::Edge(Edge::Bottom) => (cell[3], min[1], "Split below"),
        };
        if (along - gap) / 2.0 < least - 0.001 {
            "No room to split: join as a tab"
        } else {
            words
        }
    }
}

/// Whether the pointer has moved far enough from where it was pressed to be a drag (`scale` device pixels per
/// logical pixel).
pub fn is_drag(pressed: [f32; 2], now: [f32; 2], scale: f32) -> bool {
    let (dx, dy) = (now[0] - pressed[0], now[1] - pressed[1]);
    let start = style::SPACE_DRAG_START * scale;
    dx * dx + dy * dy >= start * start
}

/// The zone of `cell` that `point` is in: within a quarter of the width or height of an edge, that edge (in a
/// corner the nearer one, by share of the side); else the middle.
pub fn zone_at(cell: [f32; 4], point: [f32; 2]) -> Zone {
    let [x, y, w, h] = cell;
    if w <= 0.0 || h <= 0.0 {
        return Zone::Middle;
    }
    let (fx, fy) = ((point[0] - x) / w, (point[1] - y) / h);
    let candidates = [
        (fx, Edge::Left),
        (1.0 - fx, Edge::Right),
        (fy, Edge::Top),
        (1.0 - fy, Edge::Bottom),
    ];
    candidates
        .into_iter()
        .filter(|(d, _)| *d < style::SPACE_DOCK_EDGE)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(Zone::Middle, |(_, e)| Zone::Edge(e))
}

/// The rectangle the overlay shows for a drop on `zone` of `cell`: exactly the half the dropped Panel would take
/// (net of `gap`); the whole cell for the middle.
pub fn overlay(cell: [f32; 4], zone: Zone, gap: f32) -> [f32; 4] {
    let [x, y, w, h] = cell;
    let (hw, hh) = (((w - gap) / 2.0).max(0.0), ((h - gap) / 2.0).max(0.0));
    match zone {
        Zone::Edge(Edge::Left) => [x, y, hw, h],
        Zone::Edge(Edge::Right) => [x + w - hw, y, hw, h],
        Zone::Edge(Edge::Top) => [x, y, w, hh],
        Zone::Edge(Edge::Bottom) => [x, y + h - hh, w, hh],
        Zone::Middle => cell,
    }
}
