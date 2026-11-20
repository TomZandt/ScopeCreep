//! Named layouts at the pure seam (#98): saving into slots, recalling, renaming, and a layout surviving Panels being added or removed.
use scopecreep::layout_tree::LayoutTree;
use scopecreep::layouts::{Layouts, Rename, SLOTS};
use scopecreep::panel_grid::min_panel_size;

const NAMES: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];
const AREA: [f32; 4] = [12.0, 12.0, 1256.0, 716.0];
const GAP: f32 = 12.0;

fn default_grid() -> LayoutTree {
    LayoutTree::default_for(&NAMES)
}

/// The default grid with the first two Panels swapped: an arrangement that is not the default.
fn arranged() -> LayoutTree {
    let mut grid = default_grid();
    grid.swap(0, 1);
    grid
}

#[test]
fn a_saved_layout_is_recalled_as_it_was() {
    let mut layouts = Layouts::default();
    layouts.save(2, &arranged(), &NAMES);
    assert_eq!(layouts.recall(2, &NAMES), Some(arranged()));
}

#[test]
fn an_empty_slot_recalls_nothing() {
    let layouts = Layouts::default();
    for slot in 0..SLOTS {
        assert_eq!(layouts.recall(slot, &NAMES), None);
        assert_eq!(layouts.name(slot), None);
    }
}

#[test]
fn saving_into_a_slot_replaces_it_and_keeps_its_name() {
    let mut layouts = Layouts::default();
    layouts.save(0, &default_grid(), &NAMES);
    assert_eq!(layouts.name(0), Some("Layout 1"));
    layouts.rename(0, "Grade");
    layouts.save(0, &arranged(), &NAMES);
    assert_eq!(layouts.name(0), Some("Grade"));
    assert_eq!(layouts.recall(0, &NAMES), Some(arranged()));
}

#[test]
fn a_layout_keeps_the_border_positions_it_was_saved_with() {
    let mut grid = default_grid();
    let clean = grid.cells(AREA, GAP)[0];
    let border = grid
        .border_at(
            AREA,
            GAP,
            [clean[0] + clean[2] + GAP / 2.0, clean[1] + 40.0],
            2.0,
        )
        .expect("a border right of the clean picture");
    grid.drag(&border, [800.0, 300.0], AREA, GAP, [160.0, 90.0]);
    let mut layouts = Layouts::default();
    layouts.save(4, &grid, &NAMES);
    assert_ne!(grid, default_grid());
    assert_eq!(layouts.recall(4, &NAMES), Some(grid));
}

#[test]
fn renaming_changes_only_the_name_and_ignores_empty_names_and_empty_slots() {
    let mut layouts = Layouts::default();
    layouts.save(1, &arranged(), &NAMES);
    assert!(layouts.rename(1, "  Colour pass  "));
    assert_eq!(layouts.name(1), Some("Colour pass"));
    assert!(!layouts.rename(1, "   "), "an empty name keeps the old one");
    assert!(
        !layouts.rename(5, "Nothing here"),
        "an empty slot cannot be named"
    );
    assert_eq!(layouts.name(5), None);
    assert_eq!(layouts.recall(1, &NAMES), Some(arranged()));
}

#[test]
fn a_name_is_limited_and_has_no_control_characters() {
    let mut layouts = Layouts::default();
    layouts.save(0, &default_grid(), &NAMES);
    layouts.rename(0, &format!("a\nb{}", "x".repeat(60)));
    let name = layouts.name(0).unwrap();
    assert!(
        name.chars().count() <= 24 && !name.contains('\n'),
        "{name:?}"
    );
}

#[test]
fn layouts_are_remembered_between_sessions() {
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    layouts.rename(0, "Grade: wide");
    layouts.save(8, &default_grid(), &NAMES);
    let back = Layouts::from_text(&layouts.to_text());
    assert_eq!(back, layouts);
    assert_eq!(back.name(0), Some("Grade: wide"));
    assert_eq!(back.recall(0, &NAMES), Some(arranged()));
    assert_eq!(back.recall(8, &NAMES), Some(default_grid()));
    assert_eq!(back.name(3), None);
}

#[test]
fn rubbish_or_another_versions_file_gives_no_layouts() {
    assert_eq!(Layouts::from_text(""), Layouts::default());
    assert_eq!(
        Layouts::from_text("not layouts\nslot 1 x\n"),
        Layouts::default()
    );
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    let broken = layouts.to_text().replace("slot 1", "slot 12");
    assert_eq!(
        Layouts::from_text(&broken),
        Layouts::default(),
        "a slot that does not exist is dropped"
    );
}

#[test]
fn a_saved_arrangement_that_is_rubbish_recalls_the_default() {
    let text = "layouts 1\nslot 1 Broken\nlayout 1\ncolumns 0.5 0.5\nrows nonsense\n";
    assert_eq!(
        Layouts::from_text(text).recall(0, &NAMES),
        Some(default_grid())
    );
}

#[test]
fn a_layout_saved_with_a_border_at_the_minimum_in_a_large_window_recalls_it() {
    // A 2560 x 1440 window's Panel area: the row under the picture dragged down to the minimum there (#179).
    let big = [12.0, 12.0, 2536.0, 1400.0];
    let mut grid = arranged();
    let top = grid.cells(big, GAP)[0];
    let border = grid
        .border_at(
            big,
            GAP,
            [top[0] + top[2] / 2.0, top[1] + top[3] + GAP / 2.0],
            2.0,
        )
        .expect("a border under the first panel");
    grid.drag(&border, [300.0, 5000.0], big, GAP, min_panel_size(1.0));
    let mut layouts = Layouts::default();
    layouts.save(3, &grid, &NAMES);
    let recalled = Layouts::from_text(&layouts.to_text())
        .recall(3, &NAMES)
        .expect("the slot holds a layout");
    assert_ne!(recalled, default_grid(), "not the default layout");
    // Raised to the minimum in the default window, so about where it was left in the large one, give or take.
    for (a, b) in recalled.cells(big, GAP).iter().zip(grid.cells(big, GAP)) {
        assert!(
            (a[2] - b[2]).abs() < 0.2 * big[2] && (a[3] - b[3]).abs() < 0.3 * big[3],
            "{a:?} vs {b:?}"
        );
    }
    let min = min_panel_size(1.0);
    for c in recalled.cells(AREA, GAP) {
        assert!(c[2] >= min[0] - 0.01 && c[3] >= min[1] - 0.01, "{c:?}");
    }
}

#[test]
fn a_layout_naming_a_panel_that_no_longer_exists_drops_it() {
    // Saved while the Window also had a "Gamut/legal warning" Panel in one cell; it is gone now, and the Panel
    // that took its place elsewhere still goes where the layout put it.
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    let text = layouts.to_text().replacen(
        "leaf 0 Luma waveform\n",
        "split row 0.5\nleaf 0 Luma waveform\nleaf 0 Gamut/legal warning\n",
        1,
    );
    assert_eq!(
        Layouts::from_text(&text).recall(0, &NAMES),
        Some(arranged())
    );
}

#[test]
fn a_layout_missing_a_display_the_window_has_keeps_that_display_closed() {
    // Saved while the Window had 7 displays; it has 8 now. The saved arrangement comes back and the new display is
    // not on the page (Add panel brings it on).
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    let mut more = NAMES.to_vec();
    more.push("New display");
    let grid = layouts.recall(0, &more).unwrap();
    assert_eq!(grid, arranged());
    assert_eq!(grid.cells(AREA, GAP).len(), 8);
    assert!(!grid.contains(7));
    assert_eq!(grid.len(), 7);
}

#[test]
fn renaming_types_a_name_and_accepting_keeps_it() {
    let mut layouts = Layouts::default();
    layouts.save(0, &default_grid(), &NAMES);
    let mut rename = Rename::start(&layouts, 0).unwrap();
    assert_eq!(rename.text, "Layout 1");
    for _ in 0..8 {
        rename.backspace();
    }
    rename.type_text("Grade\n");
    assert_eq!(rename.text, "Grade");
    assert_eq!(
        layouts.name(0),
        Some("Layout 1"),
        "nothing changes until it is accepted"
    );
    assert!(rename.accept(&mut layouts));
    assert_eq!(layouts.name(0), Some("Grade"));
}

#[test]
fn renaming_an_empty_slot_does_not_start() {
    assert_eq!(Rename::start(&Layouts::default(), 0), None);
}

#[test]
fn a_typed_name_stops_at_its_limit() {
    let mut layouts = Layouts::default();
    layouts.save(0, &default_grid(), &NAMES);
    let mut rename = Rename {
        slot: 0,
        text: String::new(),
    };
    rename.type_text(&"x".repeat(100));
    assert_eq!(rename.text.chars().count(), 24);
    assert!(rename.accept(&mut layouts));
}

#[test]
fn default_layout_is_the_default_grid_for_the_displays() {
    // "Default layout" (the command) is LayoutTree::default_for, whatever has been saved.
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    assert_ne!(layouts.recall(0, &NAMES), Some(default_grid()));
    assert_eq!(LayoutTree::default_for(&NAMES), default_grid());
}

#[test]
fn a_layout_saved_as_a_grid_before_the_tree_still_recalls_as_the_same_arrangement() {
    // The default grid as saved before the tree: a sequence of cuts, so it converts to the same arrangement.
    let old = "layouts 1\nslot 1 Old grade\nlayout 1\ncolumns 0.2 0.2 0.2 0.2 0.2\nrows 0.28 0.28 0.22 0.22\npanel 0 0 3 2 Clean picture\npanel 3 0 2 1 Luma waveform\npanel 3 1 2 1 Vectorscope\npanel 0 2 3 1 ARRI False Colour\npanel 3 2 2 1 Stops False Colour\npanel 0 3 3 1 Saturation heatmap\npanel 3 3 2 1 Skin tone\n";
    let layouts = Layouts::from_text(old);
    assert_eq!(layouts.name(0), Some("Old grade"));
    let recalled = layouts.recall(0, &NAMES).unwrap();
    // The old grid's arrangement, not the new default: the clean picture spans 3 of 5 equal columns and 2 of 4
    // rows (weights 14 14 11 11).
    let clean = recalled.cells(AREA, GAP)[0];
    let (w, h) = (AREA[2] - 4.0 * GAP, AREA[3] - 3.0 * GAP);
    assert!(
        (clean[2] - (3.0 * w / 5.0 + 2.0 * GAP)).abs() < 0.5,
        "{clean:?}"
    );
    assert!((clean[3] - (h * 0.56 + GAP)).abs() < 0.5, "{clean:?}");
    assert_ne!(recalled, default_grid());
}
