//! Fixes from the first look at the integrated tiling Window at the real app (spec #196), at the pure seams: where a
//! menu drops from the button that opened it, which Panels a page owns after a migration, what titles and the Add
//! list say, a clean profile's one page, the Colour Setup bar's text above its fill (#217) and the default layout.
//! No GPU, no window.
use scopecreep::components::contains;
use scopecreep::layout::min_window;
use scopecreep::layout_tree::{LayoutTree, default_names};
use scopecreep::layouts::Layouts;
use scopecreep::menu_bar::{AddEntry, LayoutMode, add_menu, largest_menus, layout_menu};
use scopecreep::page_bar::{BarPart, PageBar};
use scopecreep::pages::{self, Pages};
use scopecreep::window_text::colour_setup_line;

const SCALES: [f32; 3] = [1.0, 1.5, 2.0];

const NAMES: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];

fn bar() -> PageBar {
    PageBar {
        names: vec!["Main".into(), "Layout 1".into()],
        current: 0,
        rename: None,
    }
}

fn entries() -> Vec<AddEntry> {
    NAMES
        .iter()
        .enumerate()
        .map(|(panel, name)| AddEntry {
            panel,
            name: name.to_string(),
            enabled: true,
        })
        .collect()
}

#[test]
fn a_menu_opened_from_the_plus_or_add_panel_drops_under_that_button_at_1x_2x_and_uhd() {
    for scale in SCALES {
        for width in [1280.0 * scale, 3840.0] {
            let window = [width, 2160.0];
            let top = scopecreep::page_bar::top(scale);
            let b = bar();
            let menus = [
                (
                    BarPart::Plus,
                    layout_menu(&[], 0, &Default::default(), LayoutMode::NewFrom).drop,
                ),
                (BarPart::AddPanel, add_menu(&entries()).drop),
            ];
            for (part, drop) in menus {
                let button = b.anchor(part, top, width, scale).expect("a button");
                let l = b.layout(top, width, scale);
                assert_eq!(
                    button,
                    if part == BarPart::Plus {
                        l.plus
                    } else {
                        l.add_panel
                    }
                );
                let rect = drop.rect_near(button, window, scale);
                let [w, _] = drop.size(scale);
                assert_eq!(
                    rect[1],
                    button[1] + button[3],
                    "{part:?}: right under the button"
                );
                assert_eq!(
                    rect[0],
                    button[0].min(window[0] - w).max(0.0),
                    "{part:?}: its left edge"
                );
                assert!(
                    rect[0] > 0.0 || button[0] == 0.0,
                    "{part:?} is not at the Window's corner"
                );
                assert!(contains(
                    [rect[0], rect[1], rect[2], rect[3]],
                    [rect[0] + 1.0, rect[1] + 1.0]
                ));
            }
        }
    }
}

#[test]
fn a_menu_that_does_not_fit_below_its_button_opens_above_it() {
    let drop = add_menu(&entries()).drop;
    let [_, h] = drop.size(1.0);
    let button = [100.0, 700.0, 20.0, 20.0];
    let rect = drop.rect_near(button, [1280.0, 760.0], 1.0);
    assert_eq!(rect[1], button[1] - h);
    let narrow = drop.rect_near([1270.0, 100.0, 20.0, 20.0], [1280.0, 760.0], 1.0);
    assert!(narrow[0] + narrow[2] <= 1280.0, "kept inside the Window");
}

#[test]
fn only_the_plus_add_panel_and_the_tabs_have_an_anchor() {
    let b = bar();
    for part in [BarPart::AutoResize, BarPart::AutoLayout] {
        assert_eq!(b.anchor(part, 56.0, 1280.0, 1.0), None);
    }
    assert!(
        b.anchor(BarPart::Tab(0), 56.0, 1280.0, 1.0).is_some(),
        "a tab's right-click menu drops from it"
    );
}

#[test]
fn a_title_shows_a_number_only_where_the_page_holds_two_of_the_display() {
    let names = [
        "Channel Viewer",
        "Vectorscope",
        "Channel Viewer",
        "Channel Viewer",
    ];
    let keys: Vec<String> = [
        "Channel Viewer",
        "Vectorscope",
        "Channel Viewer 2",
        "Channel Viewer 3",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    // One of each on the page: no number, even for the third instance.
    assert_eq!(
        pages::title_names(&names, &keys, &[3, 1]),
        [
            "Channel Viewer",
            "Vectorscope",
            "Channel Viewer",
            "Channel Viewer"
        ]
    );
    // Two clean pictures: both say their number, the Vectorscope does not.
    let shown = pages::title_names(&names, &keys, &[0, 2, 1]);
    assert_eq!(shown[0], "Channel Viewer");
    assert_eq!(shown[2], "Channel Viewer 2");
    assert_eq!(shown[1], "Vectorscope");
}

#[test]
fn the_add_list_offers_each_display_once_whatever_instances_exist() {
    let names = [
        "Channel Viewer",
        "Vectorscope",
        "Channel Viewer",
        "Vectorscope",
        "Channel Viewer",
    ];
    assert_eq!(pages::add_offers(&names), [0, 1]);
    // Instance 0 is free: it is the one added. Held by a page: the next free; all held: none (one is made).
    assert_eq!(pages::instance_to_add(&names, 0, |_| false), Some(0));
    assert_eq!(pages::instance_to_add(&names, 0, |i| i == 0), Some(2));
    assert_eq!(pages::instance_to_add(&names, 0, |i| i % 2 == 0), None);
}

fn panels_of(pages: &mut Pages, page: usize) -> Vec<usize> {
    pages.go(page);
    let mut p = pages.tree().panels();
    p.sort();
    p
}

#[test]
fn migration_puts_main_on_the_first_instances_and_every_page_on_its_own() {
    // Main and two saved layouts all on the first instances, as a migration of old saved layouts leaves them.
    let main = LayoutTree::default_for(&NAMES);
    let mut layouts = Layouts::default();
    layouts.save(0, &LayoutTree::default_for(&NAMES), &NAMES);
    layouts.save(1, &LayoutTree::default_for(&NAMES), &NAMES);
    let mut pages = Pages::migrate(main, &layouts, &NAMES);
    assert_eq!(pages.len(), 3);
    let mut names: Vec<String> = NAMES.iter().map(|n| n.to_string()).collect();
    pages.own_instances(&NAMES, |name| {
        names.push(name.to_string());
        names.len() - 1
    });
    assert_eq!(
        panels_of(&mut pages, 0),
        (0..7).collect::<Vec<_>>(),
        "Main is on the first instances"
    );
    let second = panels_of(&mut pages, 1);
    let third = panels_of(&mut pages, 2);
    assert_eq!(second, (7..14).collect::<Vec<_>>());
    assert_eq!(third, (14..21).collect::<Vec<_>>());
    // Run again: nothing moves (a page already on its own Panels is left alone).
    let before: Vec<_> = (0..3).map(|p| panels_of(&mut pages, p)).collect();
    pages.own_instances(&NAMES, |_| unreachable!("every Panel is already its own"));
    let again: Vec<_> = (0..3).map(|p| panels_of(&mut pages, p)).collect();
    assert_eq!(before, again);
}

#[test]
fn a_page_saved_on_later_instances_comes_back_on_the_first_free_ones() {
    // The names list: 7 firsts, then "Channel Viewer" twice more; Main was saved on the third instance of it.
    let names: Vec<&str> = NAMES
        .iter()
        .copied()
        .chain(["Channel Viewer", "Channel Viewer"])
        .collect();
    let mut tree = LayoutTree::default_for(&NAMES);
    tree.remap(|p| if p == 0 { 8 } else { p });
    let mut pages = Pages::new(tree);
    pages.own_instances(&names, |_| unreachable!());
    assert!(pages.tree().panels().contains(&0) && !pages.tree().panels().contains(&8));
}

#[test]
fn a_clean_profile_has_exactly_one_page_and_only_saved_layouts_become_pages() {
    let main = LayoutTree::default_for(&NAMES);
    let clean = Pages::migrate(main.clone(), &Layouts::default(), &NAMES);
    assert_eq!(clean.len(), 1);
    assert_eq!(clean.names(), ["Main"]);
    assert_eq!(*clean.tree(), main);
    // Two saved slots, not the first two: those and only those become pages, in slot order, named as saved.
    let mut layouts = Layouts::default();
    layouts.save(2, &main, &NAMES);
    layouts.save(5, &main, &NAMES);
    layouts.rename(5, "Grade");
    let migrated = Pages::migrate(main, &layouts, &NAMES);
    assert_eq!(migrated.names(), ["Main", "Layout 3", "Grade"]);
}

#[test]
fn the_colour_setup_line_says_something_inside_the_menu_bar_row() {
    for scale in SCALES {
        let line = colour_setup_line("Timeline → Output  ·  Video levels", 1280.0 * scale, scale);
        assert!(!line.text.is_empty(), "the row says something");
        assert_eq!(line.rect[1], 0.0, "in the menu bar row");
        assert!(line.rect[0] > 0.0 && line.rect[0] + line.rect[2] <= 1280.0 * scale);
    }
}

#[test]
fn the_default_layout_is_the_owners_arrangement_and_every_minimum_holds() {
    let names = default_names();
    assert_eq!(names.len(), 7);
    let tree = LayoutTree::default_for(&names);
    for scale in SCALES {
        let window = min_window(&tree, &largest_menus());
        let area = [0.0, 0.0, window[0] * scale, window[1] * scale];
        let cells = tree.cells(area, 12.0 * scale);
        let by = |name: &str| cells[names.iter().position(|n| *n == name).unwrap()];
        let clean = by("Channel Viewer");
        let (vector, heat, luma) = (by("Vectorscope"), by("Saturation"), by("Waveform"));
        let (skin, stops, arri) = (
            by("Skin Tone"),
            by("False Colour - Stops"),
            by("False Colour - Arri"),
        );
        // Big clean picture at the left; the Vectorscope beside the heatmap with the waveform under them; a strip at
        // the bottom: Stops, Arri, Skin Tone.
        assert!(clean[2] > vector[2] + heat[2] && clean[3] > luma[3]);
        assert!(vector[0] > clean[0] + clean[2] && heat[0] > vector[0]);
        assert!(luma[1] > vector[1] && (luma[0] - vector[0]).abs() < 1.0);
        assert!(skin[1] > clean[1] + clean[3] && arri[0] > stops[0] && skin[0] > arri[0]);
        for c in cells {
            assert!(
                c[2] >= 120.0 * scale - 1.0 && c[3] >= 105.0 * scale - 1.0,
                "{c:?} at {scale}x"
            );
        }
    }
    // Any other set of displays is not this layout.
    assert_ne!(LayoutTree::default_for(&["a", "b"]), tree);
}

#[test]
fn the_displays_to_choose_from_are_listed_a_to_z_whatever_the_panel_order() {
    let offers = pages::add_offers(&NAMES);
    let listed: Vec<&str> = offers.iter().map(|i| NAMES[*i]).collect();
    assert_eq!(
        listed,
        [
            "Channel Viewer",
            "False Colour - Arri",
            "False Colour - Stops",
            "Saturation",
            "Skin Tone",
            "Vectorscope",
            "Waveform"
        ]
    );
    // Case does not matter.
    let mixed = ["b display", "A display", "C display"];
    assert_eq!(pages::add_offers(&mixed), [1, 0, 2]);
}

#[test]
fn the_windows_least_size_follows_the_default_layout() {
    let tree = LayoutTree::default_for(&default_names());
    assert_eq!(min_window(&tree, &largest_menus()), [1015.0, 669.0]);
}
