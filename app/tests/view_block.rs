//! The view block on every Panel's title bar, the Colour Setup line in the menu bar row and the page tab's
//! right-click at the pure UI seam (#224, spec #196, ADR 0042): layout and hit areas at 1x and 2x, the title bar's
//! zones and divider, that the block is in the same place whatever the Panel's chips or readout, that it is
//! disabled where nothing zooms, that the Colour Setup line is cut with an ellipsis before it meets the menus, and
//! that a right-click on a page tab opens the Layout menu's entries.
use scopecreep::commands::Hotkeys;
use scopecreep::components::{Chip, MenuBar, MenuEntry, Prim, TitleBar};
use scopecreep::layout::min_window;
use scopecreep::layout_tree::LayoutTree;
use scopecreep::menu_bar::{self, LayoutMode, largest_menus};
use scopecreep::page_bar::{BarPart, PageBar};
use scopecreep::panel_grid::PanelLayout;
use scopecreep::settings::PanelSettings;
use scopecreep::style;
use scopecreep::view_block::{BlockPart, ViewBlock};
use scopecreep::window_text::colour_setup_line;

const SCALES: [f32; 2] = [1.0, 2.0];

fn block(enabled: bool, readout: &str, link: bool) -> ViewBlock {
    ViewBlock {
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

/// A title bar of a Panel `width` logical pixels wide with `chips` chips, a settings button and the view block.
fn bar(chips: usize, width: f32, scale: f32) -> scopecreep::components::TitleLayout {
    let all = ["100 %", "200 %", "Both", "Mono"].map(Chip::new);
    TitleBar {
        name: "Vectorscope".into(),
        tags: "Rec.709".into(),
        chips: all[..chips].to_vec(),
        groups: vec![0, 0, 0, 1][..chips].to_vec(),
        settings_button: true,
        view: true,
    }
    .layout(
        PanelLayout::new([0.0, 0.0, width * scale, 300.0 * scale], scale).title,
        scale,
    )
}

#[test]
fn the_title_bar_is_name_tags_chips_divider_view_block_then_the_settings_button() {
    for scale in SCALES {
        let l = bar(4, 700.0, scale);
        let tags = l.tags.expect("room for the tags");
        let view = l.view.expect("every Panel has the block");
        let divider = l.divider.expect("a divider between the two groups");
        let settings = l.settings.unwrap();
        assert!(tags[0] + tags[2] <= l.chips[0][0], "tags, then the chips");
        let last = l.chips.last().unwrap();
        assert!(last[0] + last[2] < divider[0], "chips, then the divider");
        assert!(
            divider[0] + divider[2] < view.fit[0],
            "the divider, then Fit"
        );
        assert!(view.fit[0] + view.fit[2] <= view.readout[0]);
        assert!(view.readout[0] + view.readout[2] <= view.link[0]);
        assert!(
            view.link[0] + view.link[2] <= settings[0],
            "Link, then the settings button"
        );
        assert!(
            (l.bar[0] + l.bar[2] - style::SPACE_BAR_PAD * scale - (settings[0] + settings[2]))
                .abs()
                < 0.01,
            "the settings button is fixed at the right edge"
        );
        // The divider is clear: at least the gap of a chip group on each side, as tall as a chip.
        assert!(
            view.fit[0] - (divider[0] + divider[2]) >= style::SPACE_CHIP_GROUP_GAP * scale * 0.6
        );
        assert!(divider[0] - (last[0] + last[2]) >= style::SPACE_CHIP_GROUP_GAP * scale * 0.6);
        assert!((divider[3] - style::SPACE_CHIP_HEIGHT * scale).abs() < 0.01);
        for r in [view.fit, view.readout, view.link, divider] {
            assert!(r[1] >= l.bar[1] && r[1] + r[3] <= l.bar[1] + l.bar[3] + 0.01);
        }
    }
}

#[test]
fn the_block_is_in_the_same_place_whatever_the_panels_chips_and_has_no_divider_without_any() {
    for scale in SCALES {
        let with_chips = bar(4, 600.0, scale).view.unwrap();
        let few = bar(1, 600.0, scale).view.unwrap();
        let none = bar(0, 600.0, scale);
        assert_eq!(with_chips, few);
        assert_eq!(Some(with_chips), none.view);
        assert!(none.divider.is_none(), "no chips, nothing to divide");
        assert!(bar(2, 600.0, scale).divider.is_some());
    }
}

#[test]
fn the_title_bar_layout_at_2x_is_the_1x_layout_doubled() {
    let one = bar(3, 500.0, 1.0);
    let two = bar(3, 500.0, 2.0);
    let (a, b) = (one.view.unwrap(), two.view.unwrap());
    for (x, y) in [(a.fit, b.fit), (a.readout, b.readout), (a.link, b.link)] {
        for k in 0..4 {
            assert!((x[k] * 2.0 - y[k]).abs() < 1e-3);
        }
    }
    let (d1, d2) = (one.divider.unwrap(), two.divider.unwrap());
    for k in 0..4 {
        assert!((d1[k] * 2.0 - d2[k]).abs() < 1e-3);
    }
}

#[test]
fn a_narrow_panel_gives_up_chips_to_the_popover_before_the_block_and_the_name_before_both() {
    for scale in SCALES {
        let defs = &scopecreep::vectorscope::TOOLBAR;
        let whole = scopecreep::toolbar::Toolbar::new(defs, &Default::default());
        for logical in [330.0, 280.0, 240.0] {
            let title = PanelLayout::new([0.0, 0.0, logical * scale, 200.0 * scale], scale).title;
            let mut toolbar = whole.clone();
            toolbar.fit_to_reserving(title[2], true, TitleBar::view_reserve(scale), scale);
            let l = TitleBar {
                name: "Vectorscope".into(),
                tags: String::new(),
                chips: toolbar.chips.clone(),
                groups: toolbar.groups.clone(),
                settings_button: true,
                view: true,
            }
            .layout(title, scale);
            let view = l.view.expect("the block never moves into the popover");
            assert!(view.link[0] + view.link[2] <= l.settings.unwrap()[0]);
            if let Some(divider) = l.divider {
                let last = l.chips.last().unwrap();
                assert!(
                    last[0] + last[2] < divider[0],
                    "{logical}: chips clear the divider"
                );
                assert!(
                    l.chips[0][0] >= title[0],
                    "{logical}: chips stay inside the bar"
                );
            }
            if logical < 300.0 {
                assert!(
                    toolbar.chips.len() < whole.chips.len() && !toolbar.collapsed.is_empty(),
                    "{logical}: the least used chips went to the popover"
                );
            }
        }
    }
}

#[test]
fn only_fit_and_link_are_hit_and_only_where_the_panel_zooms() {
    for scale in SCALES {
        let view = bar(2, 600.0, scale).view.unwrap();
        let on = block(true, "62 %", true);
        assert_eq!(on.hit(&view, centre(view.fit)), Some(BlockPart::Fit));
        assert_eq!(on.hit(&view, centre(view.link)), Some(BlockPart::Link));
        assert_eq!(on.hit(&view, centre(view.readout)), None, "read only");
        let off = ViewBlock::disabled(true);
        assert_eq!(off.hit(&view, centre(view.fit)), None, "disabled");
        assert_eq!(off.hit(&view, centre(view.link)), None, "disabled");
    }
}

#[test]
fn the_link_chip_is_lit_while_link_is_on_and_a_disabled_block_is_greyed_and_lights_nothing() {
    let view = bar(2, 600.0, 1.0).view.unwrap();
    let lit = style::rgba(style::accent());
    let on = fills(&block(true, "62 %", true).draw(&view, None, 1.0));
    let off = fills(&block(true, "62 %", false).draw(&view, None, 1.0));
    assert!(on.contains(&lit) && !off.contains(&lit));
    let hovered = fills(&block(true, "62 %", false).draw(&view, Some(BlockPart::Fit), 1.0));
    assert_ne!(hovered, off, "the hovered button lights");
    let disabled = ViewBlock::disabled(true);
    let greyed = disabled.draw(&view, Some(BlockPart::Fit), 1.0);
    assert!(!fills(&greyed).contains(&lit), "nothing lit while disabled");
    let texts = |b: &ViewBlock| -> Vec<String> {
        b.draw(&view, None, 1.0)
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(texts(&block(true, "62 %", false)), ["Fit", "62 %", "Link"]);
    assert_eq!(
        texts(&disabled),
        ["Fit", "", "Link"],
        "a Scope's block is the same shape with no readout"
    );
}

// ---- the Colour Setup line in the menu bar row

const LINE: &str =
    "Rec.709 Gamma 2.4 → Rec.709 Gamma 2.4  ·  Video levels  ·  from project  ·  exact";

#[test]
fn the_colour_setup_line_is_at_the_right_end_of_the_menu_bar_row() {
    for scale in SCALES {
        let width = 1600.0 * scale;
        let l = colour_setup_line(LINE, width, scale);
        assert!(!l.cut && l.text == LINE);
        assert_eq!(l.rect[1], 0.0);
        assert_eq!(l.rect[3], MenuBar::height(scale), "the row's height");
        assert!(
            (width - style::SPACE_BAR_PAD * scale - (l.rect[0] + l.rect[2])).abs() < 0.01,
            "at the right end"
        );
    }
}

#[test]
fn a_narrow_row_cuts_the_colour_setup_line_with_an_ellipsis_and_never_meets_the_menus() {
    for scale in SCALES {
        let titles = menu_bar::bar().title_rects(0.0, 0.0, scale);
        let titles_right = titles.last().map(|r| r[0] + r[2]).unwrap();
        for logical in [300.0, 420.0, 600.0, 713.0, 900.0] {
            let l = colour_setup_line(LINE, logical * scale, scale);
            if l.text.is_empty() {
                continue;
            }
            assert!(
                l.rect[0] >= titles_right,
                "{logical}: the text starts right of the menus"
            );
            assert!(l.rect[0] + l.rect[2] <= logical * scale);
            if l.cut {
                assert!(l.text.ends_with('…'), "{logical}: {}", l.text);
                assert!(
                    scopecreep::components::text_width(&l.text, scale) <= l.rect[2] + 0.01,
                    "{logical}: the cut text fits its room"
                );
                assert!(LINE.starts_with(l.text.trim_end_matches('…')));
            }
        }
        // Narrower than the menus leave room for: nothing is drawn (the hover has it all).
        assert!(colour_setup_line(LINE, titles_right, scale).text.is_empty());
    }
}

#[test]
fn the_page_below_the_menu_bar_row_starts_at_its_bottom_and_the_least_size_drops_the_colour_setup_row()
 {
    for scale in SCALES {
        assert_eq!(
            scopecreep::page_bar::top(scale),
            MenuBar::height(scale),
            "no full-width Colour Setup row any more"
        );
    }
    let names = ["Channel Viewer", "Waveform", "Vectorscope"];
    let grid = LayoutTree::default_for(&names);
    let [w, h] = min_window(&grid, &largest_menus());
    let area = grid.min_area(
        scopecreep::layout::MARGIN,
        scopecreep::panel_grid::min_panel_size(1.0),
    );
    let chrome = MenuBar::height(1.0)
        + PageBar::height(1.0)
        + scopecreep::layout::MARGIN
        + 2.0 * style::SPACE_STATUS_ROW;
    // The Panels need `area + chrome` and the tallest menu needs the menu bar row and its height: the larger is
    // the least height. The Colour Setup row (32 px) is no longer in the first.
    let menu_h = largest_menus()
        .iter()
        .map(|m| m.size(1.0)[1])
        .fold(0.0, f32::max);
    assert_eq!(
        h,
        (area[1] + chrome).max(MenuBar::height(1.0) + menu_h).ceil()
    );
    assert!(
        area[1] + chrome + style::SPACE_TOP_BAR <= h + 0.01 || h > area[1] + chrome,
        "the Panels have the row's height back"
    );
    assert!(w >= area[0] + 2.0 * scopecreep::layout::MARGIN - 1.0);
    // With nothing else to set the width, only the menu titles do (the line is cut to the room left).
    let one = LayoutTree::default_for(&["Channel Viewer"]);
    let [narrow, _] = min_window(&one, &[]);
    let titles = menu_bar::bar().title_rects(0.0, 0.0, 1.0);
    let last = titles.last().unwrap();
    assert!(narrow >= last[0] + last[2] + style::SPACE_BAR_PAD - 1.0);
}

// ---- right-click on a page tab

fn page_bar() -> PageBar {
    PageBar {
        names: vec!["Main".into(), "Grade".into(), "Noise".into()],
        current: 0,
        rename: None,
    }
}

#[test]
fn a_right_click_on_a_page_tab_gives_that_page_and_the_tab_to_drop_the_menu_from() {
    for scale in SCALES {
        let bar = page_bar();
        let (top, width) = (scopecreep::page_bar::top(scale), 1200.0 * scale);
        let layout = bar.layout(top, width, scale);
        for (page, rect) in &layout.tabs {
            assert_eq!(
                bar.right_click(top, width, centre(*rect), scale),
                Some((*page, *rect)),
                "page {page}"
            );
        }
        // Not on a tab: the buttons and empty bar have no menu of their own.
        for part in [layout.plus, layout.add_panel, layout.auto_layout] {
            assert_eq!(bar.right_click(top, width, centre(part), scale), None);
        }
        assert_eq!(
            bar.right_click(top, width, [width / 2.0, top + 1.0], scale),
            None
        );
        assert_eq!(
            bar.hit(top, width, centre(layout.tabs[1].1), scale),
            Some(BarPart::Tab(1))
        );
    }
}

#[test]
fn the_tabs_menu_is_the_layout_menu_and_the_menu_bar_still_has_it() {
    let hotkeys = Hotkeys::default();
    let names: Vec<String> = page_bar().names;
    let menu = menu_bar::layout_menu_with_undo(&names, 1, &hotkeys, LayoutMode::Show, true);
    let labels: Vec<String> = menu
        .drop
        .entries
        .iter()
        .filter_map(|e| match e {
            MenuEntry::Item(i) => Some(i.label.clone()),
            _ => None,
        })
        .collect();
    for want in [
        "New page from…",
        "Duplicate page",
        "Rename page",
        "Delete page",
        "Add panel…",
        "Auto layout",
        "Auto resize",
        "Undo layout change",
        "Default layout",
    ] {
        assert!(labels.iter().any(|l| l == want), "{want} in {labels:?}");
    }
    assert!(labels.iter().any(|l| l.ends_with("Grade")), "go to a page");
    assert_eq!(menu_bar::bar().titles[menu_bar::LAYOUT], "Layout");
}

// ---- Vectorscope defaults

#[test]
fn a_fresh_vectorscope_shows_both_plots_and_the_all_tonal_range_and_a_saved_choice_wins() {
    let defs = &scopecreep::vectorscope::SETTINGS;
    let fresh = PanelSettings::default().resolve(scopecreep::vectorscope::NAME, defs);
    assert_eq!(fresh.get("plots").map(String::as_str), Some("both"));
    assert_eq!(fresh.get("tonal").map(String::as_str), Some("all"));
    let mut saved = PanelSettings::default();
    saved.set(scopecreep::vectorscope::NAME, "plots", "200");
    saved.set(scopecreep::vectorscope::NAME, "tonal", "mids");
    let kept = saved.resolve(scopecreep::vectorscope::NAME, defs);
    assert_eq!(kept.get("plots").map(String::as_str), Some("200"));
    assert_eq!(kept.get("tonal").map(String::as_str), Some("mids"));
}

#[test]
fn the_least_panel_width_holds_the_block_and_the_settings_button_inside_the_title_bar() {
    for scale in SCALES {
        let min = scopecreep::panel_grid::min_panel_size(scale);
        let title = PanelLayout::new([0.0, 0.0, min[0], min[1]], scale).title;
        let l = TitleBar {
            name: "Skin Tone".into(),
            tags: String::new(),
            chips: vec![],
            groups: vec![],
            settings_button: true,
            view: true,
        }
        .layout(title, scale);
        let view = l.view.unwrap();
        let settings = l.settings.unwrap();
        assert!(
            view.fit[0] >= title[0] && settings[0] + settings[2] <= title[0] + title[2],
            "the block is whole inside a Panel of the least width"
        );
    }
}
