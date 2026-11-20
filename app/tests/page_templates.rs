//! Page templates at the pure layout seam (#215, spec #196, ADR 0041): each template makes the listed Panels,
//! arranged by Auto layout for the footage's aspect and fitting every minimum; a page from a template is named
//! after it, renamable, independent of the other pages and remembered like any page.
use scopecreep::auto_layout::{self, Item};
use scopecreep::layout_tree::LayoutTree;
use scopecreep::page_templates::PageTemplate;
use scopecreep::pages::Pages;
use scopecreep::panel_grid::{min_panel_size, shown};

const NAMES: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];
const GAP: f32 = 12.0;
const WINDOW: [f32; 4] = [0.0, 0.0, 1256.0, 716.0];

fn items() -> Vec<Item> {
    NAMES.iter().map(|n| auto_layout::item_of(n)).collect()
}

/// The Panels (indices into `NAMES`) the template names.
fn panels(template: PageTemplate) -> Vec<usize> {
    template
        .displays()
        .iter()
        .map(|d| NAMES.iter().position(|n| n == d).expect("a known display"))
        .collect()
}

fn built(template: PageTemplate, aspect: f32, area: [f32; 4], scale: f32) -> LayoutTree {
    template.tree(
        &panels(template),
        &items(),
        aspect,
        area,
        GAP * scale,
        scale,
    )
}

#[test]
fn the_templates_list_the_panels_the_ticket_names() {
    let list: Vec<(&str, Vec<&str>)> = PageTemplate::ALL
        .iter()
        .map(|t| (t.name(), t.displays().to_vec()))
        .collect();
    assert_eq!(
        list,
        [
            ("All 7 Panels", NAMES.to_vec()),
            (
                "Picture views",
                vec![
                    "Channel Viewer",
                    "False Colour - Arri",
                    "False Colour - Stops",
                    "Saturation",
                    "Skin Tone"
                ]
            ),
            ("Scopes", vec!["Channel Viewer", "Waveform", "Vectorscope"]),
            ("Picture only", vec!["Channel Viewer"]),
        ]
    );
}

#[test]
fn each_template_holds_exactly_its_panels_and_every_one_fits_its_minimum() {
    for scale in [1.0, 2.0] {
        let area = [0.0, 0.0, WINDOW[2] * scale, WINDOW[3] * scale];
        for aspect in [16.0 / 9.0, 2.39, 9.0 / 16.0] {
            for template in PageTemplate::ALL {
                let tree = built(template, aspect, area, scale);
                let mut held = tree.panels();
                held.sort_unstable();
                let mut want = panels(template);
                want.sort_unstable();
                assert_eq!(held, want, "{} holds its Panels", template.name());
                let min = min_panel_size(scale);
                let cells = tree.cells(area, GAP * scale);
                for p in want {
                    assert!(shown(cells[p]));
                    assert!(
                        cells[p][2] >= min[0] - 0.01 && cells[p][3] >= min[1] - 0.01,
                        "{} at {aspect}, {scale}x: Panel {p} fits",
                        template.name()
                    );
                }
            }
        }
    }
}

#[test]
fn a_template_is_arranged_by_auto_layout_for_the_aspect() {
    let template = PageTemplate::Scopes;
    let wide = built(template, 16.0 / 9.0, WINDOW, 1.0);
    let tall = built(template, 9.0 / 16.0, WINDOW, 1.0);
    assert_ne!(wide, tall, "the arrangement follows the footage");
    // The clean picture gets the most room.
    let cells = wide.cells(WINDOW, GAP);
    let area = |p: usize| cells[p][2] * cells[p][3];
    assert!(area(0) > area(1) && area(0) > area(3));
}

#[test]
fn the_picture_only_template_fills_the_window_with_the_clean_picture() {
    let tree = built(PageTemplate::PictureOnly, 16.0 / 9.0, WINDOW, 1.0);
    assert_eq!(tree.panels(), [0]);
    let c = tree.cells(WINDOW, GAP)[0];
    assert!((c[2] - WINDOW[2]).abs() < 0.01 && (c[3] - WINDOW[3]).abs() < 0.01);
}

#[test]
fn a_window_too_small_for_the_template_still_gets_a_page_holding_every_panel() {
    let tiny = [0.0, 0.0, 300.0, 200.0];
    let tree = built(PageTemplate::AllSeven, 16.0 / 9.0, tiny, 1.0);
    assert_eq!(tree.len(), 7);
}

#[test]
fn a_template_page_is_named_after_it_renamable_and_independent() {
    let mut pages = Pages::new(LayoutTree::default_for(&NAMES));
    let tree = built(PageTemplate::Scopes, 16.0 / 9.0, WINDOW, 1.0);
    assert!(pages.new_named(PageTemplate::Scopes.name(), tree.clone()));
    assert_eq!(pages.name(1), Some("Scopes"));
    assert_eq!(pages.current(), 1);
    assert!(pages.rename(1, "Scopes for grade"));
    assert_eq!(pages.name(1), Some("Scopes for grade"));
    // Its own tree: changing it leaves Main alone, and the other way round.
    pages.change(|g| g.swap(0, 1));
    let changed = pages.tree().clone();
    assert_ne!(changed, tree);
    pages.go(0);
    assert_eq!(*pages.tree(), LayoutTree::default_for(&NAMES));
    // Remembered like any page.
    let text = pages.to_text(&NAMES);
    let mut again = Pages::from_text(&NAMES, &text).expect("reads");
    assert_eq!(again.name(1), Some("Scopes for grade"));
    again.go(1);
    assert_eq!(*again.tree(), changed);
}
