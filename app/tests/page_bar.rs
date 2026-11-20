//! The Page bar at the pure UI seam (#212, spec #196, ADR 0041): its layout and hit areas at 1x and 2x, "[n] Name"
//! labels for pages 1 to 9 and none beyond, a name edited in place without its number, and the Window's minimum
//! counting the bar.
use scopecreep::components::{MenuBar, Prim};
use scopecreep::layout::min_window;
use scopecreep::layout_tree::LayoutTree;
use scopecreep::menu_bar::largest_menus;
use scopecreep::page_bar::{BarPart, PageBar, tab_label};
use scopecreep::pages::PageRename;
use scopecreep::style;

const SCALES: [f32; 2] = [1.0, 2.0];

fn bar(names: &[&str], current: usize) -> PageBar {
    PageBar {
        names: names.iter().map(|n| n.to_string()).collect(),
        current,
        rename: None,
    }
}

fn centre(r: [f32; 4]) -> [f32; 2] {
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

fn texts(prims: &[Prim]) -> Vec<String> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_first_nine_tabs_show_their_number_and_a_tenth_onward_none() {
    let names: Vec<String> = (1..=12).map(|i| format!("P{i}")).collect();
    for (i, name) in names.iter().enumerate() {
        let label = tab_label(i, name);
        if i < 9 {
            assert_eq!(label, format!("[{}] P{}", i + 1, i + 1));
        } else {
            assert_eq!(label, format!("P{}", i + 1), "no number from the tenth");
        }
    }
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let b = bar(&refs, 0);
    assert_eq!(b.label(0), "[1] P1");
    assert_eq!(b.label(9), "P10");
}

#[test]
fn the_bar_lays_out_tabs_then_plus_with_add_panel_at_the_right_end_at_1x_and_2x() {
    for scale in SCALES {
        let width = 1280.0 * scale;
        let top = 56.0 * scale;
        let b = bar(&["Colour", "Pictures", "Scopes"], 1);
        let l = b.layout(top, width, scale);
        assert_eq!(l.bar, [0.0, top, width, PageBar::height(scale)]);
        assert_eq!(
            l.tabs.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        let mut x = 0.0;
        for (_, r) in &l.tabs {
            assert!(r[0] >= x, "left to right, no overlap");
            x = r[0] + r[2];
            assert!(
                r[1] >= top && r[1] + r[3] <= top + l.bar[3] + 0.01,
                "in the bar"
            );
        }
        assert!(l.plus[0] >= x, "the + comes after the last tab");
        assert!(
            l.plus[0] + l.plus[2] <= l.auto_layout[0],
            "and before Auto layout"
        );
        assert!(
            l.auto_layout[0] + l.auto_layout[2] <= l.auto_resize[0],
            "which is beside Auto resize"
        );
        assert!(
            l.auto_resize[0] + l.auto_resize[2] <= l.add_panel[0],
            "which is beside Add panel"
        );
        assert_eq!(l.auto_resize[1], l.add_panel[1]);
        assert_eq!(l.auto_layout[1], l.add_panel[1]);
        let right = l.add_panel[0] + l.add_panel[2];
        assert!(
            (width - right - style::SPACE_BAR_PAD * scale).abs() < 0.01,
            "Add panel is at the right end"
        );
    }
    // The same in logical pixels at both scales.
    let b = bar(&["Colour", "Pictures"], 0);
    let one = b.layout(56.0, 1280.0, 1.0);
    let two = b.layout(112.0, 2560.0, 2.0);
    for (a, c) in one.tabs.iter().zip(&two.tabs) {
        for k in 0..4 {
            assert!((a.1[k] * 2.0 - c.1[k]).abs() < 0.01);
        }
    }
}

#[test]
fn a_click_finds_a_tab_the_plus_or_add_panel() {
    for scale in SCALES {
        let width = 1280.0 * scale;
        let top = 56.0 * scale;
        let b = bar(&["Colour", "Pictures"], 0);
        let l = b.layout(top, width, scale);
        assert_eq!(
            b.hit(top, width, centre(l.tabs[1].1), scale),
            Some(BarPart::Tab(1))
        );
        assert_eq!(
            b.hit(top, width, centre(l.plus), scale),
            Some(BarPart::Plus)
        );
        assert_eq!(
            b.hit(top, width, centre(l.add_panel), scale),
            Some(BarPart::AddPanel)
        );
        assert_eq!(
            b.hit(top, width, centre(l.auto_resize), scale),
            Some(BarPart::AutoResize)
        );
        assert_eq!(
            b.hit(top, width, centre(l.auto_layout), scale),
            Some(BarPart::AutoLayout)
        );
        assert_eq!(b.hit(top, width, [width / 2.0, top + 1.0], scale), None);
    }
}

#[test]
fn the_current_tab_is_lit_and_drawn_with_its_label() {
    let b = bar(&["Colour", "Pictures"], 1);
    let prims = b.draw(56.0, 1280.0, None, 1.0);
    let drawn = texts(&prims);
    assert_eq!(
        drawn,
        [
            "[1] Colour",
            "[2] Pictures",
            "+",
            "Auto layout",
            "Auto resize",
            "Add panel"
        ]
    );
}

#[test]
fn a_name_edited_in_place_shows_without_its_number_and_the_edit_text_is_the_name_alone() {
    let mut pages = scopecreep::pages::Pages::new(LayoutTree::automatic(2));
    pages.rename(0, "Colour");
    let mut b = PageBar {
        names: pages.names(),
        current: 0,
        rename: PageRename::start(&pages, 0),
    };
    assert_eq!(b.rename.as_ref().unwrap().text, "Colour");
    assert_eq!(b.label(0), "Colour|", "no [1] while it is edited");
    b.rename.as_mut().unwrap().type_text("s");
    assert_eq!(b.label(0), "Colours|");
    b.rename = None;
    assert_eq!(b.label(0), "[1] Colour");
}

#[test]
fn with_more_tabs_than_room_the_current_one_is_always_shown() {
    let names: Vec<String> = (1..=20).map(|i| format!("Page number {i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    for scale in SCALES {
        let width = 700.0 * scale;
        for current in [0, 7, 19] {
            let b = bar(&refs, current);
            let l = b.layout(0.0, width, scale);
            assert!(
                l.tabs.iter().any(|(i, _)| *i == current),
                "current {current}"
            );
            for (_, r) in &l.tabs {
                assert!(r[0] + r[2] <= l.plus[0] && r[0] >= 0.0);
            }
            assert!(
                l.plus[0] + l.plus[2] <= l.auto_layout[0],
                "the + still fits"
            );
        }
    }
}

#[test]
fn the_windows_minimum_counts_the_bar() {
    let tree = LayoutTree::automatic(1);
    let menus = largest_menus();
    let [w, h] = min_window(&tree, &menus);
    assert!(w >= PageBar::min_width(1.0));
    // The Panels' room is the same as before; the bar is one row more of chrome.
    let area = tree.min_area(
        style::SPACE_MARGIN,
        scopecreep::panel_grid::min_panel_size(1.0),
    );
    let chrome = MenuBar::height(1.0)
        + scopecreep::components::BarKind::Top.height(1.0)
        + PageBar::height(1.0)
        + style::SPACE_MARGIN
        + 2.0 * style::SPACE_STATUS_ROW;
    assert!(h >= (area[1] + chrome).ceil());
}
