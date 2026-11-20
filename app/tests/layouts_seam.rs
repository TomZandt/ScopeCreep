//! Named layouts at the pure seam (#98): saving into slots, recalling, renaming, the menu, and a
//! layout surviving Panels being added or removed.
use scopecreep::layouts::{Action, Click, Layouts, Rename, SLOTS, action, menu};
use scopecreep::panel_grid::{Border, PanelGrid};
use scopecreep::settings::MenuRow;

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
fn a_menu_click_recalls_saves_renames_or_restores_the_default() {
    let mut layouts = Layouts::default();
    layouts.save(0, &arranged(), &NAMES);
    assert_eq!(
        action(&layouts, "default", Click::Plain),
        Some(Action::Default)
    );
    assert_eq!(action(&layouts, "default", Click::Right), None);
    assert_eq!(
        action(&layouts, "1", Click::Plain),
        Some(Action::Recall(0)),
        "a saved layout is recalled"
    );
    assert_eq!(
        action(&layouts, "1", Click::Shift),
        Some(Action::Save(0)),
        "Shift saves over it"
    );
    assert_eq!(action(&layouts, "1", Click::Right), Some(Action::Rename(0)));
    assert_eq!(
        action(&layouts, "2", Click::Plain),
        Some(Action::Save(1)),
        "an empty slot is filled by a click"
    );
    assert_eq!(
        action(&layouts, "2", Click::Right),
        None,
        "there is nothing to rename in it"
    );
    assert_eq!(action(&layouts, "10", Click::Plain), None);
}

fn texts(menu: &scopecreep::settings::Menu) -> Vec<String> {
    menu.rows
        .iter()
        .map(|r| match r {
            MenuRow::Heading(t) => t.clone(),
            MenuRow::Choice { text, .. } | MenuRow::Reset { text, .. } => text.clone(),
            MenuRow::Slider { amount, value, .. } => amount.text(*value),
        })
        .collect()
}

#[test]
fn the_menu_lists_default_layout_and_the_nine_slots_by_name() {
    let mut layouts = Layouts::default();
    layouts.save(1, &arranged(), &NAMES);
    layouts.rename(1, "Colour pass");
    let rows = texts(&menu(&layouts, None));
    assert_eq!(rows[1], "Default layout");
    assert_eq!(rows[2], "1  empty");
    assert_eq!(rows[3], "2  Colour pass");
    assert_eq!(rows[10], "9  empty");
    assert_eq!(
        rows.len(),
        12,
        "a heading, the default, nine slots, and how to use it"
    );
}

#[test]
fn the_menu_rows_hit_test_like_the_settings_menu() {
    let layouts = Layouts::default();
    let m = menu(&layouts, None);
    let rect = m.rect([100.0, 100.0], [1280.0, 760.0], 1.0);
    // Row 1 is "Default layout": its centre is on the choice, the heading's is not.
    let row_centre = |i: usize| {
        [
            rect[0] + 10.0,
            scopecreep::settings::Menu::row_top(rect, i, 1.0) + 2.0,
        ]
    };
    assert_eq!(
        m.choice_at(rect, row_centre(1), 1.0),
        Some(("layout", "default"))
    );
    assert_eq!(m.choice_at(rect, row_centre(0), 1.0), None);
    assert_eq!(m.choice_at(rect, row_centre(4), 1.0), Some(("layout", "3")));
}

#[test]
fn renaming_types_into_the_menu_row_and_accepting_keeps_the_name() {
    let mut layouts = Layouts::default();
    layouts.save(0, &default_grid(), &NAMES);
    let mut rename = Rename::start(&layouts, 0).unwrap();
    assert_eq!(rename.text, "Layout 1");
    for _ in 0..8 {
        rename.backspace();
    }
    rename.type_text("Grade\n");
    assert_eq!(texts(&menu(&layouts, Some(&rename)))[2], "1  Grade|");
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
    // "Default layout" in the menu is PanelGrid::default_for, whatever has been saved.
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
