//! Docking at the pure UI seam (#210, ADR 0038): the drag threshold, the zone under the pointer and the overlay
//! rectangle for each zone, at 1x and 2x. No GPU, no window.
use scopecreep::dock::{Zone, is_drag, overlay, zone_at};
use scopecreep::layout_tree::Edge;

const GAP: f32 = 8.0;

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01)
}

#[test]
fn a_press_is_a_drag_after_four_logical_pixels() {
    for scale in [1.0, 2.0] {
        let p = [100.0, 100.0];
        assert!(!is_drag(p, [100.0 + 3.0 * scale, 100.0], scale));
        assert!(!is_drag(p, p, scale));
        assert!(is_drag(p, [100.0 + 4.0 * scale, 100.0], scale));
        assert!(is_drag(p, [100.0, 100.0 - 5.0 * scale], scale));
    }
}

#[test]
fn zones_and_overlays_at_both_scales() {
    for s in [1.0f32, 2.0] {
        let cell = [10.0 * s, 20.0 * s, 400.0 * s, 200.0 * s];
        let gap = GAP * s;
        let [x, y, w, h] = cell;
        let hw = (w - gap) / 2.0;
        let hh = (h - gap) / 2.0;
        let cases = [
            (
                [x + w * 0.1, y + h * 0.5],
                Zone::Edge(Edge::Left),
                [x, y, hw, h],
            ),
            (
                [x + w * 0.9, y + h * 0.5],
                Zone::Edge(Edge::Right),
                [x + w - hw, y, hw, h],
            ),
            (
                [x + w * 0.5, y + h * 0.1],
                Zone::Edge(Edge::Top),
                [x, y, w, hh],
            ),
            (
                [x + w * 0.5, y + h * 0.9],
                Zone::Edge(Edge::Bottom),
                [x, y + h - hh, w, hh],
            ),
            ([x + w * 0.5, y + h * 0.5], Zone::Middle, cell),
        ];
        for (point, zone, rect) in cases {
            assert_eq!(zone_at(cell, point), zone);
            assert!(near(overlay(cell, zone, gap), rect), "{zone:?} at {s}x");
        }
    }
}

#[test]
fn the_quarter_ends_at_a_quarter_and_a_corner_takes_the_nearer_edge() {
    let cell = [0.0, 0.0, 400.0, 200.0];
    assert_eq!(zone_at(cell, [99.0, 100.0]), Zone::Edge(Edge::Left));
    assert_eq!(zone_at(cell, [101.0, 100.0]), Zone::Middle);
    assert_eq!(zone_at(cell, [100.0, 49.0]), Zone::Edge(Edge::Top));
    assert_eq!(zone_at(cell, [100.0, 51.0]), Zone::Middle);
    // 10 px in and 10 px down: 2.5% of the width against 5% of the height, so the left.
    assert_eq!(zone_at(cell, [10.0, 10.0]), Zone::Edge(Edge::Left));
    assert_eq!(zone_at(cell, [30.0, 5.0]), Zone::Edge(Edge::Top));
}
