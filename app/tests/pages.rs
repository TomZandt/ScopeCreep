//! Pages at the pure layout seam (#212, spec #196, ADR 0041): go, new (empty, default, copy), rename, delete,
//! the current page remembered, each page with its own tree, focus and undo, and the migration of the nine named
//! layouts.
use scopecreep::display::{Display, DisplayContext, DisplaySet};
use scopecreep::gpu_frame::GpuFrame;
use scopecreep::layout_tree::LayoutTree;
use scopecreep::layouts::Layouts;
use scopecreep::pages::{MAX_PAGES, PageRename, Pages};

const NAMES: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];
const AREA: [f32; 4] = [8.0, 8.0, 1424.0, 738.0];
const GAP: f32 = 8.0;

fn default_tree() -> LayoutTree {
    LayoutTree::default_for(&NAMES)
}

/// The default layout with two Panels swapped: an arrangement that is not the default.
fn arranged() -> LayoutTree {
    let mut tree = default_tree();
    tree.swap(0, 1);
    tree
}

fn names(pages: &Pages) -> Vec<String> {
    (0..pages.len())
        .map(|i| pages.name(i).unwrap().to_string())
        .collect()
}

#[test]
fn a_new_set_of_pages_is_one_page_called_main_holding_the_arrangement() {
    let pages = Pages::new(arranged());
    assert_eq!(names(&pages), ["Main"]);
    assert_eq!(pages.current(), 0);
    assert_eq!(*pages.tree(), arranged());
}

#[test]
fn going_to_a_page_makes_it_current_and_a_page_that_is_not_there_changes_nothing() {
    let mut pages = Pages::new(default_tree());
    pages.new_empty(7);
    pages.new_empty(7);
    assert_eq!(pages.current(), 2, "a new page is shown");
    assert!(pages.go(0));
    assert_eq!(pages.current(), 0);
    assert!(!pages.go(3));
    assert_eq!(pages.current(), 0);
}

#[test]
fn a_new_empty_page_has_no_panel_and_a_default_page_has_all_seven() {
    let mut pages = Pages::new(arranged());
    pages.new_empty(7);
    assert!(pages.tree().is_empty());
    pages.new_with(default_tree());
    assert_eq!(*pages.tree(), default_tree());
    assert_eq!(pages.len(), 3);
    // The first page is as it was left.
    pages.go(0);
    assert_eq!(*pages.tree(), arranged());
}

#[test]
fn a_copy_of_the_page_copies_the_tree_and_asks_for_each_panel_a_copy_to_hold() {
    let mut pages = Pages::new(arranged());
    let mut asked = Vec::new();
    pages.copy_current(|panel| {
        asked.push(panel);
        panel + 10
    });
    asked.sort_unstable();
    assert_eq!(asked, (0..7).collect::<Vec<_>>(), "each Panel once");
    assert_eq!(pages.current(), 1);
    assert_eq!(pages.name(1), Some("Copy of Main"));
    let mut panels = pages.tree().panels();
    panels.sort_unstable();
    assert_eq!(
        panels,
        (10..17).collect::<Vec<_>>(),
        "the copy holds the copies"
    );
    // The same arrangement: the cells are the same, Panel for Panel.
    let copy_cells = pages.tree().cells(AREA, GAP);
    pages.go(0);
    let own = pages.tree().cells(AREA, GAP);
    assert_eq!(copy_cells[10], own[0]);
    assert_eq!(copy_cells[11], own[1]);
    assert_eq!(
        *pages.tree(),
        arranged(),
        "the page copied from is untouched"
    );
}

#[test]
fn a_copy_keeps_the_focus_on_the_same_panel_and_starts_with_nothing_to_undo() {
    let mut pages = Pages::new(default_tree());
    pages.tree_mut().set_focus(3);
    pages.change(|t| t.swap(0, 1));
    assert!(pages.can_undo());
    pages.copy_current(|p| p + 10);
    assert_eq!(pages.tree().focused(), Some(13));
    assert!(!pages.can_undo(), "undo is the page's own");
}

#[test]
fn each_page_keeps_its_own_tree_focus_and_undo() {
    let mut pages = Pages::new(default_tree());
    pages.tree_mut().set_focus(4);
    pages.change(|t| t.swap(0, 1));
    pages.new_with(default_tree());
    assert_ne!(
        pages.tree().focused(),
        Some(4),
        "the new page has its own focus"
    );
    assert!(!pages.can_undo(), "and nothing to undo");
    pages.change(|t| t.close(2));
    pages.go(0);
    assert_eq!(pages.tree().focused(), Some(4));
    assert!(pages.undo());
    assert_eq!(
        *pages.tree(),
        default_tree(),
        "undo on page 1 undid page 1's change"
    );
    assert!(!pages.can_undo());
    pages.go(1);
    assert!(pages.can_undo(), "page 2 still has its own step");
    assert!(pages.undo());
    assert_eq!(*pages.tree(), default_tree());
}

#[test]
fn rename_changes_the_name_and_an_empty_name_keeps_the_old_one() {
    let mut pages = Pages::new(default_tree());
    assert!(pages.rename(0, "  Grade  "));
    assert_eq!(pages.name(0), Some("Grade"));
    assert!(!pages.rename(0, "   "));
    assert!(!pages.rename(0, "Grade"), "the same name is no change");
    assert!(!pages.rename(5, "x"));
    let long = "W".repeat(60);
    pages.rename(0, &long);
    assert_eq!(pages.name(0).unwrap().chars().count(), 24);
}

#[test]
fn a_name_is_typed_in_place_and_accepted() {
    let mut pages = Pages::new(default_tree());
    let mut typing = PageRename::start(&pages, 0).unwrap();
    assert_eq!(typing.text, "Main", "the number is not part of the name");
    typing.backspace();
    typing.type_text("k\n");
    assert_eq!(typing.text, "Maik");
    assert!(typing.accept(&mut pages));
    assert_eq!(pages.name(0), Some("Maik"));
    assert!(PageRename::start(&pages, 4).is_none());
}

#[test]
fn delete_always_leaves_one_page_and_keeps_the_current_page_where_it_was() {
    let mut pages = Pages::new(default_tree());
    pages.new_empty(7);
    pages.new_empty(7);
    pages.rename(0, "A");
    pages.rename(1, "B");
    pages.rename(2, "C");
    pages.go(2);
    assert!(pages.delete(0));
    assert_eq!(names(&pages), ["B", "C"]);
    assert_eq!(pages.name(pages.current()), Some("C"), "still on C");
    assert!(pages.delete(1), "deleting the current page");
    assert_eq!(names(&pages), ["B"]);
    assert_eq!(pages.current(), 0);
    assert!(!pages.delete(0), "the last page cannot go");
    assert_eq!(pages.len(), 1);
    assert!(!pages.delete(9));
}

#[test]
fn there_is_a_limit_to_how_many_pages() {
    let mut pages = Pages::new(default_tree());
    for _ in 0..MAX_PAGES + 5 {
        pages.new_empty(7);
    }
    assert_eq!(pages.len(), MAX_PAGES);
}

#[test]
fn the_window_can_ask_whether_another_page_fits_before_making_anything() {
    let mut pages = Pages::new(default_tree());
    while !pages.is_full() {
        assert!(pages.new_empty(7));
    }
    assert_eq!(pages.len(), MAX_PAGES);
    assert!(!pages.new_empty(7));
}

#[test]
fn the_pages_and_the_current_page_are_remembered() {
    let mut pages = Pages::new(arranged());
    pages.new_empty(7);
    pages.new_with(default_tree());
    pages.rename(1, "Pictures");
    pages.go(1);
    let text = pages.to_text(&NAMES);
    let back = Pages::from_text(&NAMES, &text).expect("readable");
    assert_eq!(names(&back), ["Main", "Pictures", "Page 3"]);
    assert_eq!(back.current(), 1);
    assert!(back.tree().is_empty());
    let mut back = back;
    back.go(0);
    assert_eq!(*back.tree(), arranged());
    back.go(2);
    assert_eq!(*back.tree(), default_tree());
}

#[test]
fn rubbish_or_a_file_of_another_version_is_not_pages() {
    assert!(Pages::from_text(&NAMES, "").is_none());
    assert!(Pages::from_text(&NAMES, "pages 9\ncurrent 1\n").is_none());
    assert!(
        Pages::from_text(&NAMES, "pages 1\ncurrent 1\n").is_none(),
        "no page"
    );
}

#[test]
fn a_page_that_cannot_be_read_is_dropped_and_the_current_page_stays_valid() {
    let good = arranged().to_text(&NAMES);
    let text = format!("pages 1\ncurrent 3\npage One\n{good}page Two\nrubbish\npage Three\n{good}");
    let pages = Pages::from_text(&NAMES, &text).expect("two pages are readable");
    assert_eq!(names(&pages), ["One", "Three"]);
    let text = format!("pages 1\ncurrent 99\npage One\n{good}");
    assert_eq!(Pages::from_text(&NAMES, &text).unwrap().current(), 0);
}

fn layouts_with(slots: &[(usize, &str, &LayoutTree)]) -> Layouts {
    let mut layouts = Layouts::default();
    for (slot, name, tree) in slots {
        layouts.save(*slot, tree, &NAMES);
        layouts.rename(*slot, name);
    }
    layouts
}

#[test]
fn nine_named_layouts_become_pages_in_slot_order_after_main() {
    let tree = arranged();
    let slots: Vec<(usize, String)> = (0..9).map(|i| (i, format!("Look {}", 9 - i))).collect();
    let refs: Vec<(usize, &str, &LayoutTree)> =
        slots.iter().map(|(i, n)| (*i, n.as_str(), &tree)).collect();
    let pages = Pages::migrate(default_tree(), &layouts_with(&refs), &NAMES);
    let mut expected = vec!["Main".to_string()];
    expected.extend((0..9).map(|i| format!("Look {}", 9 - i)));
    assert_eq!(names(&pages), expected);
    assert_eq!(pages.current(), 0, "Main is shown");
    assert_eq!(
        *pages.tree(),
        default_tree(),
        "Main holds the arrangement the user had"
    );
    let mut pages = pages;
    pages.go(4);
    assert_eq!(*pages.tree(), tree);
}

#[test]
fn with_no_saved_layouts_there_is_one_page_main() {
    let pages = Pages::migrate(arranged(), &Layouts::default(), &NAMES);
    assert_eq!(names(&pages), ["Main"]);
    assert_eq!(*pages.tree(), arranged());
}

#[test]
fn empty_slots_are_skipped_and_the_others_keep_their_order() {
    let tree = arranged();
    let layouts = layouts_with(&[(1, "Second", &tree), (6, "Seventh", &tree)]);
    let pages = Pages::migrate(default_tree(), &layouts, &NAMES);
    assert_eq!(names(&pages), ["Main", "Second", "Seventh"]);
}

#[test]
fn a_saved_layout_that_cannot_be_read_is_dropped_by_the_migration() {
    let tree = arranged();
    let layouts = layouts_with(&[(0, "Good", &tree)]);
    let text = layouts.to_text()
        + "slot 3 Broken\nnot a layout\n"
        + "slot 4 Fine\n"
        + &tree.to_text(&NAMES);
    let layouts = Layouts::from_text(&text);
    let pages = Pages::migrate(default_tree(), &layouts, &NAMES);
    assert_eq!(names(&pages), ["Main", "Good", "Fine"]);
}

/// A display with a name, that does nothing: enough to be a Panel that can be copied.
struct Stub(&'static str);

impl Display for Stub {
    fn name(&self) -> &'static str {
        self.0
    }
    fn on_frame(&self, _: &mut wgpu::CommandEncoder, _: &GpuFrame) {}
    fn configure(&self, _: &DisplayContext) {}
    fn label(&self) -> String {
        self.0.into()
    }
    fn draw(&self, _: &mut wgpu::RenderPass<'_>, _: [f32; 4], _: [f32; 2], _: f32) {}
}

fn stub_set() -> DisplaySet {
    let mut set = DisplaySet::default();
    for name in NAMES {
        set.register_with(
            std::sync::Arc::new(Stub(name)),
            std::sync::Arc::new(move || std::sync::Arc::new(Stub(name))),
        );
    }
    set
}

#[test]
fn a_default_page_shares_no_panel_with_another_page() {
    let set = stub_set();
    let free = scopecreep::pages::default_tree(&set, |_| false);
    assert_eq!(free, default_tree(), "nothing claimed: the first instances");
    let tree = scopecreep::pages::default_tree(&set, |p| p == 0 || p == 3);
    assert_eq!(set.len(), 9, "two new instances were made");
    assert_eq!(set.keys()[7], "Channel Viewer 2");
    assert_eq!(set.keys()[8], "Vectorscope 2");
    let mut panels = tree.panels();
    panels.sort_unstable();
    assert_eq!(panels, [1, 2, 4, 5, 6, 7, 8]);
    // Same arrangement as the default, Panel for Panel.
    let (a, b) = (tree.cells(AREA, GAP), default_tree().cells(AREA, GAP));
    assert_eq!((a[7], a[1], a[8]), (b[0], b[1], b[3]));
}

#[test]
fn copies_a_saved_page_names_are_made_again_before_it_is_read() {
    let set = stub_set();
    let text =
        "layout 2\nsplit row 0.5\nleaf 0 Clean picture|Clean picture 2\nleaf 0 Vectorscope 3\n";
    scopecreep::pages::recreate_saved_copies(&set, &[text, "leaf 0 Nonsense 99\n"]);
    let keys = set.keys();
    assert!(keys.contains(&"Channel Viewer 2".to_string()), "{keys:?}");
    assert!(keys.contains(&"Vectorscope 3".to_string()), "{keys:?}");
    assert!(!keys.iter().any(|k| k.starts_with("Nonsense")));
}

#[test]
fn a_panel_only_a_deleted_page_held_is_removed_and_the_pages_left_keep_their_panels() {
    let set = stub_set();
    let mut settings = scopecreep::settings::PanelSettings::default();
    let mut pages = Pages::new(default_tree());
    // Page 2 and 3 are copies: seven new Panels each (indices 7..14 and 14..21).
    for _ in 0..2 {
        pages.copy_current(|p| set.duplicate(p).unwrap_or(p));
    }
    assert_eq!(set.len(), 21);
    assert!(
        pages.sweep(&set, &mut settings).is_empty(),
        "every Panel is on a page"
    );
    pages.go(1);
    pages.delete(1);
    let gone = pages.sweep(&set, &mut settings);
    assert_eq!(gone.len(), 7, "the deleted page's copies went");
    assert_eq!(set.len(), 14);
    // The page that was third now holds the Panels that moved up to 7..14.
    pages.go(1);
    let mut panels = pages.tree().panels();
    panels.sort_unstable();
    assert_eq!(panels, (7..14).collect::<Vec<_>>());
    assert_eq!(
        set.keys()[7],
        "Channel Viewer 3",
        "a Panel keeps its key when others go"
    );
}

#[test]
fn only_the_shown_pages_shown_panels_are_live_and_every_other_panel_is_hidden() {
    let set = stub_set();
    let mut pages = Pages::new(default_tree());
    pages.copy_current(|p| set.duplicate(p).unwrap_or(p));
    // Page 2 holds Panels 7 to 13; page 1 holds 0 to 6.
    assert_eq!(pages.live(set.len()).iter().filter(|l| **l).count(), 7);
    assert!(pages.live(14)[7..].iter().all(|l| *l));
    assert!(pages.live(14)[..7].iter().all(|l| !*l));
    pages.go(0);
    pages.show_on(&set);
    assert!((0..7).all(|i| set.is_shown(i)) && (7..14).all(|i| !set.is_shown(i)));
    pages.go(1);
    pages.show_on(&set);
    assert!((0..7).all(|i| !set.is_shown(i)) && (7..14).all(|i| set.is_shown(i)));
}
