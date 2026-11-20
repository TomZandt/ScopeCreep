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
    /// Anywhere else (a tab, #211): nothing is docked yet.
    Middle,
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
