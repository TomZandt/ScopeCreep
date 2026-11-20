//! The displays were renamed (Clean picture is now Channel Viewer, and so on: see the Displays table in GLOSSARY.md).
//! A file saved under a former name is still read, to the same settings and arrangement, and is written with the new
//! names from then on: one test per kind of file. Pure: no GPU, no window.
use scopecreep::layout_tree::LayoutTree;
use scopecreep::layouts::Layouts;
use scopecreep::pages::{self, Pages};
use scopecreep::settings::{FORMER_NAMES, PanelSettings, modern_key};

/// The seven displays in the Window now, in Panel order.
const NAMES: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];

/// `text` as an older version wrote it: the new names turned back into the old ones.
fn as_saved_before(text: &str) -> String {
    let mut old = text.to_string();
    for (former, now) in FORMER_NAMES {
        old = old.replace(now, former);
    }
    old
}

#[test]
fn every_former_name_and_its_numbered_instances_map_to_the_new_name() {
    for (former, now) in FORMER_NAMES {
        assert_eq!(modern_key(former), now);
        assert_eq!(modern_key(&format!("{former} 2")), format!("{now} 2"));
        assert_eq!(modern_key(&format!("{former} 12")), format!("{now} 12"));
        assert_eq!(modern_key(now), now, "a new name is left alone");
    }
    // Not a former name, and not an instance number.
    for other in [
        "Vectorscope",
        "Vectorscope 3",
        "Skin tone line",
        "Clean picture x",
        "Channel view",
    ] {
        assert_eq!(modern_key(other), other);
    }
}

#[test]
fn settings_saved_under_the_former_names_are_read_and_written_under_the_new() {
    let old = "Clean picture\tchannel\tred\nClean picture 2\tstate:loupe_zoom\t300\nLuma waveform\tscale\t10\nARRI False Colour\tbw\ton\nStops False Colour\tgrey\ton\nSkin tone\tbw\ton\nSaturation heatmap\tscale\t120\nWindow\tstate:link\toff\n";
    let settings = PanelSettings::from_text(old);
    let text = settings.to_text();
    for want in [
        "Channel Viewer\tchannel\tred",
        "Channel Viewer 2\tstate:loupe_zoom\t300",
        "Waveform\tscale\t10",
        "False Colour - Arri\tbw\ton",
        "False Colour - Stops\tgrey\ton",
        "Skin Tone\tbw\ton",
        "Saturation\tscale\t120",
        "Window\tstate:link\toff",
    ] {
        assert!(text.contains(want), "{want} in {text}");
    }
    for (former, _) in FORMER_NAMES {
        assert!(!text.contains(former), "{former} is not written again");
    }
    assert_eq!(
        PanelSettings::from_text(&text).to_text(),
        text,
        "and the new text reads back the same"
    );
}

fn default_tree() -> LayoutTree {
    LayoutTree::default_for(&NAMES)
}

#[test]
fn a_layout_saved_under_the_former_names_is_the_same_arrangement() {
    let text = default_tree().to_text(&NAMES);
    let old = as_saved_before(&text);
    assert!(old.contains("Clean picture") && !old.contains("Channel Viewer"));
    assert_eq!(LayoutTree::from_text(&NAMES, &old), default_tree());
    assert!(
        !LayoutTree::from_text(&NAMES, &old)
            .to_text(&NAMES)
            .contains("Clean picture"),
        "written with the new names"
    );
}

#[test]
fn named_layouts_saved_under_the_former_names_recall_the_same_arrangement() {
    let mut layouts = Layouts::default();
    layouts.save(0, &default_tree(), &NAMES);
    layouts.rename(0, "Grade");
    let old = as_saved_before(&layouts.to_text());
    // The layout's own name could hold a display word: only the arrangement's names were changed back here.
    let back = Layouts::from_text(&old);
    assert_eq!(back.recall(0, &NAMES), Some(default_tree()));
}

#[test]
fn pages_saved_under_the_former_names_come_back_with_their_panels() {
    let mut pages = Pages::new(default_tree());
    pages.new_with(default_tree());
    let text = pages.to_text(&NAMES);
    let old = as_saved_before(&text);
    assert!(old.contains("Clean picture"));
    let read = Pages::from_text(&NAMES, &old).expect("readable");
    assert_eq!(read.len(), 2);
    assert_eq!(*read.tree(), default_tree());
}

#[test]
fn saved_copies_by_their_former_key_are_made_again_under_the_new_key() {
    // recreate_saved_copies reads a page that holds "Clean picture 2": the copy is asked for as "Channel Viewer 2".
    let old = "pages 1\ncurrent 1\npage Main\nlayout 2\nsplit row 0.5\nleaf 0 Clean picture|Clean picture 2\nleaf 0 Vectorscope\n";
    let keys_wanted = pages::wanted_copies(&[old]);
    assert_eq!(
        keys_wanted,
        [(
            "Channel Viewer".to_string(),
            "Channel Viewer 2".to_string(),
            2
        )]
    );
}
