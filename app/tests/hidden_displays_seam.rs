//! Gamut Warning, Neutrals and Chromaticity as Panel instances (#222, ADR 0039), proven at the Feed seam with the
//! real display set: each can be added to a page, duplicated, closed and replaced; a page without one has it closed
//! and does no work for it; and the default page is still the seven. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::DisplaySet;
use scopecreep::layout_tree::{Dir, LayoutTree};
use scopecreep::page_edit::{self, Room};
use scopecreep::pages;
use scopecreep::settings::PanelSettings;

const NEW: [&str; 3] = ["Gamut Warning", "Neutrals", "Chromaticity"];
const ROOM: Room = Room {
    area: [0.0, 0.0, 1424.0, 738.0],
    gap: 12.0,
    min: [120.0, 105.0],
};

struct Bench {
    set: DisplaySet,
    tree: LayoutTree,
    settings: PanelSettings,
    _rig: Rig,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let set = DisplaySet::standard(&rig.device, &rig.queue, TARGET, false);
        let tree = pages::default_tree(&set, |_| false);
        Bench {
            set,
            tree,
            settings: PanelSettings::default(),
            _rig: rig,
        }
    }

    fn index(&self, key: &str) -> usize {
        self.set.keys().iter().position(|k| k == key).unwrap()
    }
}

#[test]
fn the_window_registers_ten_displays_and_the_default_page_holds_the_seven() {
    let b = Bench::new("hidden-ten");
    let names = b.set.names();
    assert_eq!(names.len(), 10, "{names:?}");
    for name in NEW {
        assert!(names.contains(&name), "{name} is registered: {names:?}");
        assert!(
            !b.tree.contains(b.index(name)),
            "{name} is not on the default page"
        );
    }
    let on_page = (0..names.len()).filter(|i| b.tree.contains(*i)).count();
    assert_eq!(on_page, 7);
}

#[test]
fn a_display_not_on_the_page_is_not_shown_so_it_costs_nothing() {
    let b = Bench::new("hidden-cost");
    b.set.set_shown(&b.tree.shown_panels());
    for name in NEW {
        assert!(!b.set.is_shown(b.index(name)), "{name} is hidden");
    }
}

#[test]
fn each_can_be_added_shown_duplicated_closed_and_replaced() {
    for name in NEW {
        let mut b = Bench::new("hidden-lifecycle");
        let panel = b.index(name);
        // Added: the page now holds it, and it is shown.
        b.tree.add(panel, ROOM.area, ROOM.gap, ROOM.min);
        b.set.set_shown(&b.tree.shown_panels());
        assert!(b.tree.contains(panel) && b.set.is_shown(panel), "{name}");
        // Duplicated: its own key, with a copy of its settings.
        b.settings.set(name, "brightness", "150");
        let copy =
            page_edit::duplicate(&b.set, &mut b.tree, &mut b.settings, panel, Dir::Row, ROOM)
                .unwrap_or_else(|e| panic!("{name} duplicates: {e:?}"));
        assert_eq!(b.set.key(copy), format!("{name} 2"));
        assert_eq!(b.set.name(copy), name);
        assert!(b.settings.to_text().contains(&format!("{name} 2\t")));
        // Closed: the copy goes (its number is free again), the first instance stays for Add.
        page_edit::close(&b.set, &mut b.tree, &mut b.settings, copy, |_| false);
        assert_eq!(
            page_edit::sweep(&b.set, &mut b.tree, &mut b.settings),
            vec![copy]
        );
        assert!(!b.settings.to_text().contains(&format!("{name} 2\t")));
        // Replaced: a Panel of the page becomes it, in the place and size of the old one.
        page_edit::close(&b.set, &mut b.tree, &mut b.settings, panel, |_| false);
        assert!(!b.tree.contains(panel));
        let skin = b.index("Skin Tone");
        let place = b.tree.cells(ROOM.area, ROOM.gap)[skin];
        let new = page_edit::replace(&b.set, &mut b.tree, &mut b.settings, skin, name, |_| false)
            .unwrap_or_else(|| panic!("{name} replaces"));
        assert_eq!(new, panel, "the closed first instance comes back");
        assert_eq!(b.tree.cells(ROOM.area, ROOM.gap)[new], place);
        assert!(!b.tree.contains(skin));
    }
}

#[test]
fn replace_with_lists_the_three_with_the_others_a_to_z() {
    let b = Bench::new("hidden-replace-list");
    let from = b.index("Skin Tone");
    let list = page_edit::replace_choices(&b.set, from);
    assert_eq!(
        list,
        [
            "Channel Viewer",
            "Chromaticity",
            "False Colour - Arri",
            "False Colour - Stops",
            "Gamut Warning",
            "Neutrals",
            "Saturation",
            "Vectorscope",
            "Waveform",
        ]
    );
}
