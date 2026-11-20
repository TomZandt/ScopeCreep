//! Named layouts at the pure seam (#98): saving into slots, recalling, renaming, and a layout surviving Panels being added or removed.
use scopecreep::layouts::{Layouts, Rename, SLOTS};
use scopecreep::panel_grid::{Border, PanelGrid, min_panel_size};

const NAMES: [&str; 7] = [
    "Clean picture",
    "Luma waveform",
    "ARRI False Colour",
    "Vectorscope",
    "Saturation heatmap",
    "Skin tone",
    "Stops False Colour",
];
const AREA: [f32; 4] = [12.0, 12.0, 1256.0, 716.0];
const GAP: f32 = 12.0;

fn default_grid() -> PanelGrid {
    PanelGrid::default_for(&NAMES)
}

/// The default grid with the first two Panels swapped: an arrangement that is not the default.
fn arranged() -> PanelGrid {
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
    grid.drag(
        Border::Column { after: 2 },
        [800.0, 300.0],
        AREA,
        GAP,
        [160.0, 90.0],
    );
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
    grid.drag(
        Border::Row { after: 1 },
        [300.0, 5000.0],
        big,
        GAP,
        min_panel_size(1.0),
    );
    let mut layouts = Layouts::default();
    layouts.save(3, &grid, &NAMES);
    let recalled = Layouts::from_text(&layouts.to_text())
        .recall(3, &NAMES)
        .expect("the slot holds a layout");
    assert_ne!(recalled, default_grid(), "not the default layout");
    assert_eq!(recalled.places(), grid.places());
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
    let text = layouts.to_text() + "panel 0 0 1 1 Gamut/legal warning\n";
    assert_eq!(
        Layouts::from_text(&text).recall(0, &NAMES),
        Some(arranged())
    );
}

#[test]
fn a_panel_missing_from_a_layout_is_placed_in_the_first_free_cell() {
    // Saved without the heatmap (it was not there): it takes the one free cell.
    let mut layouts = Layouts::default();
    layouts.save(0, &default_grid(), &NAMES);
    let without = layouts
        .to_text()
        .replace("panel 2 3 1 1 Saturation heatmap\n", "");
    let grid = Layouts::from_text(&without).recall(0, &NAMES).unwrap();
    assert_eq!(grid, default_grid());
    // A Panel the Window gained since has no cell left: it gets a row of its own.
    let mut more = NAMES.to_vec();
    more.push("New display");
    let grid = layouts.recall(0, &more).unwrap();
    let new = grid.places()[7];
    assert_eq!((new.column, new.row, new.columns, new.rows), (0, 4, 5, 1));
    assert_eq!(grid.size(), (5, 5));
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
    // "Default layout" (the command) is PanelGrid::default_for, whatever has been saved.
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    assert_ne!(layouts.recall(0, &NAMES), Some(default_grid()));
    assert_eq!(PanelGrid::default_for(&NAMES), default_grid());
}

#[test]
fn a_layout_saved_with_the_colour_waveform_still_recalls_and_the_other_panels_keep_their_places() {
    // The 9-Panel Window as it was before the Colour Waveform left it (#145), saved to the layouts file and read back.
    let mut before = NAMES.to_vec();
    before.push("Channel view");
    let mut grid = PanelGrid::default_for(&before);
    grid.swap(0, 1);
    let mut layouts = Layouts::default();
    layouts.save(0, &grid, &before);
    let file = Layouts::from_text(&layouts.to_text());

    let recalled = file.recall(0, &NAMES).expect("the layout recalls");
    assert_eq!(
        recalled.places().len(),
        NAMES.len(),
        "the Panel that is gone is dropped"
    );
    // A neighbour may grow into the cell the gone Panel leaves, but each Panel stays where it was put.
    let corner = |places: &[scopecreep::panel_grid::Place]| {
        places.iter().map(|p| (p.column, p.row)).collect::<Vec<_>>()
    };
    assert_eq!(
        corner(recalled.places()),
        corner(&grid.places()[..NAMES.len()]),
        "the others keep their places"
    );
}
