//! The edge + at the pure UI seam (ADR 0038, #209): four hit areas on a Panel at 1x and 2x, shown only while the
//! pointer is over the Panel, inside it and clear of its title bar and the gaps. No GPU, no window.
use scopecreep::components::*;
use scopecreep::layout_tree::Edge;
use scopecreep::panel_grid::PanelLayout;
use scopecreep::style;

const SCALES: [f32; 2] = [1.0, 2.0];
const SIZES: [[f32; 2]; 3] = [[600.0, 400.0], [200.0, 300.0], [120.0, 105.0]];

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

#[test]
fn there_is_one_plus_on_each_side_inside_the_panel_and_clear_of_the_title_bar() {
    for scale in SCALES {
        for size in SIZES {
            let frame = [
                100.0 * scale,
                50.0 * scale,
                size[0] * scale,
                size[1] * scale,
            ];
            let layout = PanelLayout::new(frame, scale);
            let pluses = edge_plus_rects(frame, scale);
            let edges: Vec<Edge> = pluses.iter().map(|(e, _)| *e).collect();
            assert_eq!(
                edges,
                vec![Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]
            );
            let grab = style::SPACE_GRAB * scale;
            for (edge, r) in &pluses {
                // Inside the Panel, clear of the border and the divider's grab zone (the gaps).
                assert!(
                    r[0] >= frame[0] + grab && r[1] >= frame[1] + grab,
                    "{edge:?}"
                );
                assert!(r[0] + r[2] <= frame[0] + frame[2] - grab, "{edge:?}");
                assert!(r[1] + r[3] <= frame[1] + frame[3] - grab, "{edge:?}");
                // Clear of the title bar.
                assert!(r[1] >= layout.title[1] + layout.title[3], "{edge:?}");
                // A target a thumb-sized pointer can hit.
                assert!(r[2] >= 16.0 * scale && r[3] >= 16.0 * scale);
            }
            // Four separate targets.
            for i in 0..4 {
                for j in i + 1..4 {
                    assert!(!overlap(pluses[i].1, pluses[j].1), "{i} and {j}");
                }
            }
            // Each sits against its own side, in the middle of it.
            let content = layout.content;
            let at = |e: Edge| pluses.iter().find(|(x, _)| *x == e).unwrap().1;
            let mid = |r: [f32; 4]| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
            assert!(at(Edge::Left)[0] < content[0] + content[2] * 0.25);
            assert!(at(Edge::Right)[0] > content[0] + content[2] * 0.75 - at(Edge::Right)[2]);
            assert!(at(Edge::Top)[1] < content[1] + content[3] * 0.5);
            assert!(at(Edge::Bottom)[1] + at(Edge::Bottom)[3] > content[1] + content[3] * 0.5);
            let (cx, cy) = (content[0] + content[2] / 2.0, content[1] + content[3] / 2.0);
            assert!((mid(at(Edge::Left))[1] - cy).abs() < 1.0 * scale);
            assert!((mid(at(Edge::Right))[1] - cy).abs() < 1.0 * scale);
            assert!((mid(at(Edge::Top))[0] - cx).abs() < 1.0 * scale);
            assert!((mid(at(Edge::Bottom))[0] - cx).abs() < 1.0 * scale);
        }
    }
}

#[test]
fn a_press_on_a_plus_names_its_side_and_elsewhere_names_none() {
    for scale in SCALES {
        let frame = [100.0 * scale, 50.0 * scale, 600.0 * scale, 400.0 * scale];
        for (edge, r) in edge_plus_rects(frame, scale) {
            let centre = [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
            assert_eq!(edge_plus_at(frame, centre, scale), Some(edge));
            assert_eq!(edge_plus_at(frame, [r[0] - 1.0, centre[1]], scale), None);
        }
        let middle = [frame[0] + frame[2] / 2.0, frame[1] + frame[3] / 2.0];
        assert_eq!(edge_plus_at(frame, middle, scale), None);
        // The title bar is never a +.
        assert_eq!(
            edge_plus_at(frame, [middle[0], frame[1] + 4.0 * scale], scale),
            None
        );
    }
}

#[test]
fn the_plus_shows_only_while_the_pointer_is_over_the_panel() {
    let frame = [100.0, 50.0, 600.0, 400.0];
    assert!(edge_plus_shown(frame, [300.0, 200.0]));
    assert!(edge_plus_shown(frame, [101.0, 51.0]));
    assert!(!edge_plus_shown(frame, [99.0, 200.0]));
    assert!(!edge_plus_shown(frame, [300.0, 451.0]));
    assert!(!edge_plus_shown(frame, [-1.0, -1.0]));
}

#[test]
fn the_plus_draws_a_quiet_cross_and_lights_under_the_pointer() {
    for scale in SCALES {
        let frame = [100.0 * scale, 50.0 * scale, 600.0 * scale, 400.0 * scale];
        let quiet = edge_plus_draw(frame, None, scale);
        let lit = edge_plus_draw(frame, Some(Edge::Top), scale);
        assert!(!quiet.is_empty());
        assert_eq!(quiet.len(), lit.len());
        assert_ne!(format!("{quiet:?}"), format!("{lit:?}"));
        // Nothing drawn reaches outside the Panel.
        for p in &quiet {
            if let Prim::Rect { rect, .. } = p {
                assert!(rect[0] >= frame[0] && rect[1] >= frame[1]);
                assert!(rect[0] + rect[2] <= frame[0] + frame[2]);
                assert!(rect[1] + rect[3] <= frame[1] + frame[3]);
            }
        }
    }
}
