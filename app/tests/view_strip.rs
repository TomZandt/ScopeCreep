//! The view strip at the pure UI seam (#205, spec #196, ADR 0040): its layout and hit areas at 1x and 2x, that Fit,
//! the readout and Link are in the same place whatever the layout, that it is disabled with no picture Panel, and
//! that the Window's minimum width counts it.
use scopecreep::components::{MenuBar, Prim};
use scopecreep::layout::min_window;
use scopecreep::menu_bar::{self, largest_menus};
use scopecreep::panel_grid::PanelGrid;
use scopecreep::style;
use scopecreep::view_strip::{StripPart, ViewStrip};

const SCALES: [f32; 2] = [1.0, 2.0];

fn strip(enabled: bool, readout: &str, link: bool) -> ViewStrip {
    ViewStrip {
        enabled,
        readout: readout.to_string(),
        link,
    }
}

fn centre(r: [f32; 4]) -> [f32; 2] {
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

fn fills(prims: &[Prim]) -> Vec<[f32; 4]> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Rect { colour, .. } => Some(*colour),
            _ => None,
        })
        .collect()
}

#[test]
fn the_strip_is_at_the_right_end_of_the_menu_bar_row_and_scales_with_the_display() {
    for scale in SCALES {
        let width = 1280.0 * scale;
        let l = ViewStrip::layout(width, scale);
        for r in [l.fit, l.readout, l.link] {
            assert!(
                r[1] >= 0.0 && r[1] + r[3] <= MenuBar::height(scale) + 0.01,
                "in the row"
            );
            assert!(r[0] + r[2] <= width, "inside the window");
        }
        assert!(l.fit[0] + l.fit[2] <= l.readout[0] && l.readout[0] + l.readout[2] <= l.link[0]);
        let right = l.link[0] + l.link[2];
        assert!(
            (width - right - style::SPACE_BAR_PAD * scale).abs() < 0.01,
            "at the right end"
        );
    }
    let one = ViewStrip::layout(1280.0, 1.0);
    let two = ViewStrip::layout(2560.0, 2.0);
    for (a, b) in [
        (one.fit, two.fit),
        (one.readout, two.readout),
        (one.link, two.link),
    ] {
        for k in 0..4 {
            assert!((a[k] * 2.0 - b[k]).abs() < 1e-3);
        }
    }
}

#[test]
fn fit_the_readout_and_link_are_in_the_same_place_whatever_the_readout_says() {
    for scale in SCALES {
        let width = 1000.0 * scale;
        let layout = ViewStrip::layout(width, scale);
        for s in [
            strip(true, "Fit", true),
            strip(true, "200 %", false),
            strip(true, "Mixed", false),
            strip(false, "", true),
        ] {
            assert_eq!(s.parts(width, scale), layout);
        }
        // The readout's slot holds the widest of its texts.
        for text in ["Fit", "800 %", "Mixed"] {
            assert!(
                layout.readout[2] >= text.chars().count() as f32 * style::SPACE_CHAR_WIDTH * scale
            );
        }
    }
}

#[test]
fn only_fit_and_link_are_hit_and_only_while_a_picture_panel_is_shown() {
    for scale in SCALES {
        let width = 1000.0 * scale;
        let l = ViewStrip::layout(width, scale);
        let on = strip(true, "Fit", true);
        assert_eq!(on.hit(width, centre(l.fit), scale), Some(StripPart::Fit));
        assert_eq!(on.hit(width, centre(l.link), scale), Some(StripPart::Link));
        assert_eq!(on.hit(width, centre(l.readout), scale), None, "read only");
        assert_eq!(
            on.hit(width, [l.fit[0] - 1.0, centre(l.fit)[1]], scale),
            None
        );
        let off = strip(false, "", true);
        assert_eq!(off.hit(width, centre(l.fit), scale), None, "disabled");
        assert_eq!(off.hit(width, centre(l.link), scale), None, "disabled");
    }
}

#[test]
fn the_link_chip_is_lit_while_link_is_on_and_a_disabled_strip_lights_nothing() {
    let lit = style::rgba(style::ACCENT);
    let on = fills(&strip(true, "Fit", true).draw(1000.0, None, 1.0));
    let off = fills(&strip(true, "Fit", false).draw(1000.0, None, 1.0));
    assert!(on.contains(&lit) && !off.contains(&lit));
    let hovered = fills(&strip(true, "Fit", false).draw(1000.0, Some(StripPart::Fit), 1.0));
    assert!(hovered.contains(&style::rgba(style::STATE_HOVER)) || hovered != off);
    let disabled = fills(&strip(false, "", true).draw(1000.0, Some(StripPart::Fit), 1.0));
    assert!(!disabled.contains(&lit));
    let texts: Vec<String> = strip(true, "Mixed", false)
        .draw(1000.0, None, 1.0)
        .iter()
        .filter_map(|p| match p {
            Prim::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["Fit", "Mixed", "Link"]);
}

#[test]
fn the_minimum_window_width_counts_the_strip_beside_the_menu_titles() {
    let names = ["Clean picture", "Luma waveform", "Vectorscope"];
    let grid = PanelGrid::default_for(&names);
    let [w, _] = min_window(&grid, &largest_menus());
    for scale in SCALES {
        let titles = menu_bar::bar().title_rects(0.0, 0.0, scale);
        let last = titles.last().unwrap();
        let l = ViewStrip::layout(w * scale, scale);
        assert!(
            l.fit[0] >= last[0] + last[2] - 0.01,
            "at the minimum width the strip clears the menu titles: {l:?} vs {last:?}"
        );
    }
    // With nothing else to set the width, the strip beside the titles does.
    let one = PanelGrid::default_for(&["Clean picture"]);
    let [narrow, _] = min_window(&one, &[]);
    let titles = menu_bar::bar().title_rects(0.0, 0.0, 1.0);
    let last = titles.last().unwrap();
    assert!(narrow >= last[0] + last[2] + ViewStrip::width(1.0) + style::SPACE_BAR_PAD - 1.0);
}
