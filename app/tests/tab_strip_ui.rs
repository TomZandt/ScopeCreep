//! The tab strip in a tab group's title bar at the pure UI seam (ADR 0038, #211): the active highlight, the hit
//! areas, the overflow at the minimum Panel width, at 1x and 2x, and the drop overlay's "Tab here". No GPU, no
//! window.
use scopecreep::components::{Prim, TitleBar};
use scopecreep::dock::Zone;
use scopecreep::layout_tree::Edge;
use scopecreep::panel_grid::{MIN_PANEL, PanelLayout};
use scopecreep::style;
use scopecreep::tab_strip::{TabHit, TabStrip, room};

const SCALES: [f32; 2] = [1.0, 2.0];

fn strip(labels: &[&str], active: usize) -> TabStrip {
    TabStrip {
        labels: labels.iter().map(|l| l.to_string()).collect(),
        active,
    }
}

fn inside(inner: [f32; 4], outer: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.01
        && inner[1] >= outer[1] - 0.01
        && inner[0] + inner[2] <= outer[0] + outer[2] + 0.01
        && inner[1] + inner[3] <= outer[1] + outer[3] + 0.01
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// A Panel's title bar with its settings button and a chip, at `width` logical pixels.
fn bar_room(width: f32, scale: f32) -> [f32; 4] {
    let frame = [0.0, 0.0, width * scale, 200.0 * scale];
    let title = PanelLayout::new(frame, scale).title;
    let layout = TitleBar {
        name: String::new(),
        tags: String::new(),
        chips: vec![scopecreep::components::Chip::new("Y")],
        groups: Vec::new(),
        settings_button: true,
        view: false,
    }
    .layout(title, scale);
    room(&layout, scale)
}

#[test]
fn the_active_tab_is_highlighted_and_the_others_are_not() {
    for scale in SCALES {
        let area = bar_room(400.0, scale);
        let s = strip(&["Channel Viewer", "Waveform", "Vectorscope"], 1);
        let l = s.layout(area, scale);
        assert_eq!(l.tabs.len(), 3);
        assert!(l.overflow.is_none() && l.hidden.is_empty());
        let prims = s.draw(area, None, scale);
        let fills: Vec<[f32; 4]> = prims
            .iter()
            .filter_map(|p| match p {
                Prim::Rect { rect, colour } if *colour == style::rgba(style::SURFACE_CONTROL) => {
                    Some(*rect)
                }
                _ => None,
            })
            .collect();
        // One fill, on the active tab only; an accent underline sits at its foot.
        assert_eq!(fills, vec![l.tabs[1].1]);
        let underline = prims.iter().any(
            |p| matches!(p, Prim::Rect { colour, .. } if *colour == style::rgba(style::ACCENT)),
        );
        assert!(underline);
        for ((_, r), next) in l.tabs.iter().zip(l.tabs.iter().skip(1)) {
            assert!(!overlap(*r, next.1));
            assert!(inside(*r, area));
        }
    }
}

#[test]
fn each_tab_has_a_hit_area_and_the_title_bar_beside_them_has_none() {
    for scale in SCALES {
        let area = bar_room(400.0, scale);
        let s = strip(&["One", "Two"], 0);
        let l = s.layout(area, scale);
        for (i, r) in &l.tabs {
            let middle = [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
            assert_eq!(s.hit(area, middle, scale), Some(TabHit::Tab(*i)));
        }
        let past = [l.tabs[1].1[0] + l.tabs[1].1[2] + 2.0, area[1] + 2.0];
        assert_eq!(s.hit(area, past, scale), None);
        // The tabs stay inside the room, so the chips and the settings button keep their own presses.
        assert!(l.tabs.iter().all(|(_, r)| inside(*r, area)));
    }
}

#[test]
fn at_the_minimum_panel_width_the_shown_tab_stays_and_an_overflow_chip_counts_the_rest() {
    for scale in SCALES {
        let area = bar_room(MIN_PANEL[0], scale);
        let labels = [
            "Channel Viewer",
            "Waveform",
            "Vectorscope",
            "Skin Tone",
            "ARRI",
        ];
        for active in [0, 2, 4] {
            let s = strip(&labels, active);
            let l = s.layout(area, scale);
            assert!(l.tabs.iter().any(|(i, _)| *i == active), "{active}");
            assert!(!l.hidden.is_empty());
            assert_eq!(l.tabs.len() + l.hidden.len(), labels.len());
            let chip = l.overflow.expect("overflow chip");
            assert!(inside(chip, area));
            for (_, r) in &l.tabs {
                assert!(inside(*r, area) && !overlap(*r, chip));
            }
            assert_eq!(
                s.hit(area, [chip[0] + 1.0, chip[1] + 1.0], scale),
                Some(TabHit::Overflow)
            );
            // The chip says how many are out of sight.
            let says = s.draw(area, None, scale).iter().any(
                |p| matches!(p, Prim::Text { text, .. } if *text == format!("+{}", l.hidden.len())),
            );
            assert!(says);
            // It shows the next hidden tab after the shown one, wrapping to the first.
            let next = s.next_hidden(&l.hidden).unwrap();
            assert!(l.hidden.contains(&next));
            assert_eq!(
                next,
                l.hidden
                    .iter()
                    .copied()
                    .find(|i| *i > active)
                    .unwrap_or(l.hidden[0])
            );
        }
    }
}

#[test]
fn tabs_squeeze_to_share_the_room_before_any_overflow() {
    for scale in SCALES {
        let area = bar_room(220.0, scale);
        let s = strip(&["Channel Viewer", "Waveform", "Vectorscope"], 0);
        let l = s.layout(area, scale);
        assert_eq!(l.tabs.len(), 3);
        assert!(l.overflow.is_none());
        assert!(l.tabs.iter().all(|(_, r)| inside(*r, area)));
        let min = style::SPACE_TAB_MIN_WIDTH * scale;
        assert!(l.tabs.iter().all(|(_, r)| r[2] >= min - 0.01));
    }
}

#[test]
fn the_tab_layout_at_2x_is_the_1x_layout_doubled() {
    let labels = ["Channel Viewer", "Waveform", "Vectorscope"];
    let a = strip(&labels, 1).layout(bar_room(300.0, 1.0), 1.0);
    let b = strip(&labels, 1).layout(bar_room(300.0, 2.0), 2.0);
    for ((i, r), (j, q)) in a.tabs.iter().zip(&b.tabs) {
        assert_eq!(i, j);
        for k in 0..4 {
            assert!((r[k] * 2.0 - q[k]).abs() < 0.1, "{r:?} {q:?}");
        }
    }
}

#[test]
fn the_middle_of_a_tab_group_is_labelled_tab_here_and_an_edge_is_not() {
    assert_eq!(Zone::Middle.label(), Some("Tab here"));
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        assert_eq!(Zone::Edge(edge).label(), None);
    }
}
