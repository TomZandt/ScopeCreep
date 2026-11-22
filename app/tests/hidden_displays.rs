//! Gamut Warning, Neutrals and Chromaticity are back in the Window (#222, parked spec #121), at the pure layout
//! seam: ten displays registered, but the default layout, the page templates and every page saved before still give
//! the seven they always did, and the three are offered by Add panel, the edge + and Replace with, A to Z. No GPU.
use scopecreep::auto_resize::{Shape, shape_of};
use scopecreep::layout_tree::LayoutTree;
use scopecreep::page_templates::PageTemplate;
use scopecreep::pages::{self, Pages};
use scopecreep::settings::{PanelSettings, modern_key};

/// The ten displays as the Window registers them, in Panel order.
const TEN: [&str; 10] = [
    "Channel Viewer",
    "Waveform",
    "Gamut Warning",
    "Neutrals",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
    "Chromaticity",
];
/// The seven of the default layout, in Panel order.
const SEVEN: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];
const NEW: [&str; 3] = ["Gamut Warning", "Neutrals", "Chromaticity"];
const AREA: [f32; 4] = [8.0, 8.0, 1424.0, 738.0];
const GAP: f32 = 8.0;

fn at(names: &[&str], name: &str) -> usize {
    names.iter().position(|n| *n == name).unwrap()
}

#[test]
fn the_default_layout_is_the_exact_arrangement_of_the_seven_with_the_three_closed() {
    let seven = LayoutTree::default_for(&SEVEN).cells(AREA, GAP);
    let ten = LayoutTree::default_for(&TEN);
    let cells = ten.cells(AREA, GAP);
    for name in SEVEN {
        assert_eq!(
            cells[at(&TEN, name)],
            seven[at(&SEVEN, name)],
            "{name} is where it was"
        );
    }
    for name in NEW {
        assert!(!ten.contains(at(&TEN, name)), "{name} is closed");
    }
}

#[test]
fn the_page_templates_never_hold_the_three() {
    for template in PageTemplate::ALL {
        for name in template.displays() {
            assert!(!NEW.contains(name), "{} holds {name}", template.name());
        }
    }
}

#[test]
fn a_page_saved_with_the_seven_loads_with_the_three_closed() {
    let text = Pages::new(LayoutTree::default_for(&SEVEN)).to_text(&SEVEN);
    let read = Pages::from_text(&TEN, &text).expect("readable");
    assert_eq!(read.len(), 1);
    for name in NEW {
        assert!(!read.tree().contains(at(&TEN, name)), "{name} closed");
    }
    let seven = LayoutTree::default_for(&SEVEN).cells(AREA, GAP);
    let cells = read.tree().cells(AREA, GAP);
    for name in SEVEN {
        assert_eq!(cells[at(&TEN, name)], seven[at(&SEVEN, name)], "{name}");
    }
}

#[test]
fn a_layout_saved_with_the_seven_still_loads_with_the_three_closed() {
    let text = LayoutTree::default_for(&SEVEN).to_text(&SEVEN);
    assert_eq!(
        LayoutTree::from_text(&TEN, &text).to_text(&TEN),
        LayoutTree::default_for(&TEN).to_text(&TEN)
    );
}

#[test]
fn a_layout_saved_under_a_former_name_of_the_three_reads_as_the_new_name() {
    let saved = "layout 2\nsplit row 0.5\nleaf 0 Channel Viewer\nleaf 0 CIE chromaticity\n";
    let tree = LayoutTree::from_text(&TEN, saved);
    assert!(tree.contains(at(&TEN, "Chromaticity")));
    assert!(tree.contains(at(&TEN, "Channel Viewer")));
    assert!(!tree.contains(at(&TEN, "Neutrals")));
    for (former, now) in [
        ("Gamut/legal warning", "Gamut Warning"),
        ("Highlight neutrals", "Neutrals"),
        ("CIE chromaticity", "Chromaticity"),
    ] {
        assert_eq!(modern_key(former), now);
        assert_eq!(modern_key(&format!("{former} 3")), format!("{now} 3"));
    }
}

#[test]
fn settings_saved_under_the_former_names_of_the_three_are_read_under_the_new() {
    let old = "Gamut/legal warning\tlimits\tebu\nHighlight neutrals\ttolerance\t5\nCIE chromaticity\tplot\tx\n";
    let text = PanelSettings::from_text(old).to_text();
    for want in [
        "Gamut Warning\tlimits\tebu",
        "Neutrals\ttolerance\t5",
        "Chromaticity\tplot\tx",
    ] {
        assert!(text.contains(want), "{want} in {text}");
    }
}

#[test]
fn add_panel_offers_the_ten_a_to_z_by_shown_name() {
    let offers: Vec<&str> = pages::add_offers(&TEN)
        .into_iter()
        .map(|i| TEN[i])
        .collect();
    assert_eq!(
        offers,
        [
            "Channel Viewer",
            "Chromaticity",
            "False Colour - Arri",
            "False Colour - Stops",
            "Gamut Warning",
            "Neutrals",
            "Saturation",
            "Skin Tone",
            "Vectorscope",
            "Waveform",
        ]
    );
}

#[test]
fn the_shapes_of_the_three_are_known_to_auto_resize_and_auto_layout() {
    assert_eq!(shape_of("Gamut Warning"), Shape::Picture);
    assert_eq!(shape_of("Neutrals"), Shape::Picture);
    assert_eq!(shape_of("Chromaticity"), Shape::Other);
}
