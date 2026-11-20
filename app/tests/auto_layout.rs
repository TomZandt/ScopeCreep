//! Auto layout at the pure layout seam (#214, spec #196, ADR 0041): the result keeps reading order and tab groups,
//! fits every minimum, is deterministic, gives the clean picture the most room and differs with the footage and
//! the Window; with no fit the tree is unchanged and the notice names the minimum; above 9 Panels a greedy
//! halving runs; 7 Panels are done within the budget; one undo step, and the focus stays.
use scopecreep::auto_layout::{self, Item};
use scopecreep::layout_tree::{LayoutTree, NoFit};
use scopecreep::pages::Pages;
use scopecreep::panel_grid::{MIN_PANEL, min_panel_size, shown};
use std::time::{Duration, Instant};

const NAMES: [&str; 7] = [
    "Clean picture",
    "Luma waveform",
    "ARRI False Colour",
    "Vectorscope",
    "Saturation heatmap",
    "Skin tone",
    "Stops False Colour",
];
const GAP: f32 = 12.0;
/// How many variations a page steps through before it wraps.
const REROLLS: usize = 8;
const WINDOW: [f32; 4] = [0.0, 0.0, 1256.0, 716.0];

fn items(names: &[&str]) -> Vec<Item> {
    names.iter().map(|n| auto_layout::item_of(n)).collect()
}

fn default_tree() -> LayoutTree {
    LayoutTree::default_for(&NAMES)
}

fn laid(aspect: f32, area: [f32; 4], scale: f32) -> LayoutTree {
    let mut t = default_tree();
    t.auto_layout(&items(&NAMES), aspect, area, GAP * scale, scale)
        .expect("fits");
    t
}

fn assert_minimums(t: &LayoutTree, area: [f32; 4], scale: f32) {
    let min = min_panel_size(scale);
    for (i, c) in t.cells(area, GAP * scale).iter().enumerate() {
        assert!(
            c[2] >= min[0] - 0.5 && c[3] >= min[1] - 0.5,
            "{}: {c:?} under {min:?}",
            NAMES[i]
        );
    }
}

#[test]
fn the_result_fits_every_minimum_at_1x_and_2x() {
    for scale in [1.0, 2.0] {
        for aspect in [16.0 / 9.0, 2.39, 9.0 / 16.0] {
            let area = WINDOW.map(|v| v * scale);
            assert_minimums(&laid(aspect, area, scale), area, scale);
        }
    }
}

#[test]
fn reading_order_is_kept() {
    let before = default_tree().panels();
    for aspect in [16.0 / 9.0, 2.39, 9.0 / 16.0] {
        assert_eq!(laid(aspect, WINDOW, 1.0).panels(), before);
    }
}

#[test]
fn a_tab_group_stays_one_item_with_its_shown_tab() {
    let names = ["Clean picture", "Vectorscope", "Skin tone", "Luma waveform"];
    let text = "layout 2\nsplit row 0.5\nleaf 0 Clean picture\nsplit col 0.5\nleaf 1 Vectorscope|Skin tone\nleaf 0 Luma waveform\n";
    let mut t = LayoutTree::parse_text(&names, text).expect("a layout");
    t.auto_layout(&items(&names), 16.0 / 9.0, WINDOW, GAP, 1.0)
        .expect("fits");
    let group = t.tab_group(1).expect("a group");
    assert_eq!(group.tabs, vec![1, 2]);
    assert_eq!(group.active, 1, "the shown tab is still shown");
    assert_eq!(t.tab_group(2), Some(group));
    let cells = t.cells(WINDOW, GAP);
    assert!(
        shown(cells[2]) && !shown(cells[1]),
        "the other tab is off screen"
    );
    assert_eq!(t.panels(), vec![0, 1, 2, 3]);
}

#[test]
fn it_is_deterministic() {
    let a = laid(2.39, WINDOW, 1.0).to_text(&NAMES);
    assert_eq!(a, laid(2.39, WINDOW, 1.0).to_text(&NAMES));
}

const ASPECT: f32 = 16.0 / 9.0;

fn press(t: &mut LayoutTree) {
    t.auto_layout(&items(&NAMES), ASPECT, WINDOW, GAP, 1.0)
        .expect("fits");
}

/// The arrangements a run of `n` presses on a fresh default page gives, as text.
fn presses(n: usize) -> Vec<String> {
    let mut t = default_tree();
    (0..n)
        .map(|_| {
            press(&mut t);
            t.to_text(&NAMES)
        })
        .collect()
}

#[test]
fn successive_presses_give_distinct_arrangements_in_a_fixed_order() {
    let run = presses(REROLLS);
    for (i, a) in run.iter().enumerate() {
        for b in &run[i + 1..] {
            assert_ne!(a, b, "two presses gave the same arrangement");
        }
    }
    assert_eq!(
        run[0],
        laid(ASPECT, WINDOW, 1.0).to_text(&NAMES),
        "first is the best"
    );
    assert_eq!(
        run,
        presses(REROLLS),
        "the order is the same on another run"
    );
}

#[test]
fn every_variation_keeps_minimums_reading_order_and_tab_groups() {
    let names = ["Clean picture", "Vectorscope", "Skin tone", "Luma waveform"];
    let text = "layout 2\nsplit row 0.5\nleaf 0 Clean picture\nsplit col 0.5\nleaf 1 Vectorscope|Skin tone\nleaf 0 Luma waveform\n";
    let mut t = LayoutTree::parse_text(&names, text).expect("a layout");
    let mut seen = Vec::new();
    for _ in 0..REROLLS {
        t.auto_layout(&items(&names), ASPECT, WINDOW, GAP, 1.0)
            .expect("fits");
        assert_eq!(t.panels(), vec![0, 1, 2, 3]);
        assert_eq!(t.tab_group(1).expect("a group").tabs, vec![1, 2]);
        for c in t.cells(WINDOW, GAP).into_iter().filter(|c| shown(*c)) {
            assert!(
                c[2] >= MIN_PANEL[0] - 0.5 && c[3] >= MIN_PANEL[1] - 0.5,
                "{c:?} in {}",
                t.to_text(&names)
            );
        }
        seen.push(t.to_text(&names));
    }
    seen.dedup();
    assert!(seen.len() > 1, "four items have more than one arrangement");

    let mut seven = default_tree();
    for _ in 0..REROLLS {
        press(&mut seven);
        assert_eq!(seven.panels(), default_tree().panels());
        assert_minimums(&seven, WINDOW, 1.0);
    }
}

#[test]
fn the_press_after_the_last_variation_returns_to_the_best() {
    let run = presses(REROLLS + 2);
    assert_eq!(run[REROLLS], run[0]);
    assert_eq!(run[REROLLS + 1], run[1]);
}

#[test]
fn any_other_change_restarts_at_the_best() {
    let mut t = default_tree();
    press(&mut t);
    press(&mut t);
    // A Panel closed and added back is another arrangement, so the next press is a first press of that tree.
    t.close(5);
    t.add(5, WINDOW, GAP, min_panel_size(1.0));
    // The same arrangement in a tree that has never been pressed: both give the same first press.
    let mut reference = LayoutTree::parse_text(&NAMES, &t.to_text(&NAMES)).expect("a layout");
    press(&mut t);
    press(&mut reference);
    assert_eq!(t.to_text(&NAMES), reference.to_text(&NAMES));
}

#[test]
fn a_press_after_a_hand_change_is_the_best_again() {
    let mut t = default_tree();
    for _ in 0..3 {
        press(&mut t);
    }
    let result = t.to_text(&NAMES);
    let mut moved = LayoutTree::parse_text(&NAMES, &result).expect("a layout");
    moved.swap(0, 1);
    moved.swap(0, 1);
    assert_eq!(moved.to_text(&NAMES), result);
    // Same arrangement, parsed fresh: a first press, so it gives the best.
    press(&mut moved);
    assert_eq!(moved.to_text(&NAMES), presses(1).remove(0));
}

#[test]
fn a_change_of_footage_aspect_restarts_at_the_best() {
    let mut t = default_tree();
    press(&mut t);
    press(&mut t);
    t.auto_layout(&items(&NAMES), 2.39, WINDOW, GAP, 1.0)
        .expect("fits");
    assert_eq!(t.to_text(&NAMES), laid(2.39, WINDOW, 1.0).to_text(&NAMES));
}

#[test]
fn undo_steps_back_one_variation_and_each_press_is_one_step() {
    let mut pages = Pages::new(default_tree());
    let mut texts = vec![pages.tree().to_text(&NAMES)];
    for _ in 0..3 {
        pages.change(press);
        texts.push(pages.tree().to_text(&NAMES));
    }
    for want in texts.iter().rev().skip(1) {
        assert!(pages.undo());
        assert_eq!(&pages.tree().to_text(&NAMES), want);
    }
    assert!(!pages.can_undo());
}

#[test]
fn a_press_after_an_undo_shows_the_variation_it_undid() {
    let mut pages = Pages::new(default_tree());
    for _ in 0..3 {
        pages.change(press);
    }
    let third = pages.tree().to_text(&NAMES);
    pages.undo();
    pages.change(press);
    assert_eq!(pages.tree().to_text(&NAMES), third);
}

#[test]
fn above_nine_panels_a_repeat_press_changes_nothing() {
    let refs: Vec<&str> = std::iter::once("Clean picture")
        .chain(std::iter::repeat_n("Skin tone", 11))
        .collect();
    let area = [0.0, 0.0, 3840.0, 2160.0];
    let mut pages = Pages::new(LayoutTree::automatic(12));
    pages.change(|t| {
        t.auto_layout(&items(&refs), ASPECT, area, GAP, 1.0)
            .expect("fits")
    });
    let once = pages.tree().to_text(&refs);
    pages.change(|t| {
        t.auto_layout(&items(&refs), ASPECT, area, GAP, 1.0)
            .expect("fits")
    });
    assert_eq!(pages.tree().to_text(&refs), once);
    assert!(pages.undo());
    assert!(!pages.can_undo(), "the repeat press was no undo step");
}

#[test]
fn the_clean_picture_gets_the_most_room() {
    for (aspect, area) in [
        (16.0 / 9.0, WINDOW),
        (2.39, WINDOW),
        (9.0 / 16.0, [0.0, 0.0, 716.0, 1256.0]),
        (16.0 / 9.0, [0.0, 0.0, 2512.0, 1432.0]),
    ] {
        let t = laid(aspect, area, 1.0);
        let c = t.cells(area, GAP);
        let size = |i: usize| c[i][2] * c[i][3];
        for i in 1..NAMES.len() {
            assert!(
                size(0) >= size(i),
                "aspect {aspect}: {} bigger than the clean picture",
                NAMES[i]
            );
        }
    }
}

#[test]
fn it_differs_for_16_9_2_39_9_16_footage_and_a_portrait_window() {
    let wide = laid(16.0 / 9.0, WINDOW, 1.0).to_text(&NAMES);
    let cinema = laid(2.39, WINDOW, 1.0).to_text(&NAMES);
    let tall = laid(9.0 / 16.0, WINDOW, 1.0).to_text(&NAMES);
    let portrait_window = [0.0, 0.0, 716.0, 1256.0];
    let mut t = default_tree();
    t.auto_layout(&items(&NAMES), 16.0 / 9.0, portrait_window, GAP, 1.0)
        .expect("fits");
    let portrait = t.to_text(&NAMES);
    assert_ne!(wide, cinema);
    assert_ne!(wide, tall);
    assert_ne!(cinema, tall);
    assert_ne!(wide, portrait);
    // The clean picture's cell follows the footage: wide for 2.39, taller for 9:16.
    let shape = |tree: &LayoutTree, area: [f32; 4]| {
        let c = tree.cells(area, GAP)[0];
        c[2] / c[3]
    };
    assert!(
        shape(&laid(2.39, WINDOW, 1.0), WINDOW) > shape(&laid(9.0 / 16.0, WINDOW, 1.0), WINDOW)
    );
}

#[test]
fn with_no_fit_the_tree_is_unchanged_and_the_notice_names_the_minimum() {
    let tiny = [0.0, 0.0, MIN_PANEL[0] * 2.0, MIN_PANEL[1] * 2.0];
    let mut t = default_tree();
    let before = t.to_text(&NAMES);
    let result = t.auto_layout(&items(&NAMES), 16.0 / 9.0, tiny, GAP, 1.0);
    assert_eq!(result, Err(NoFit));
    assert_eq!(t.to_text(&NAMES), before);
    let notice = NoFit.notice();
    assert!(notice.contains(&format!("{} x {}", MIN_PANEL[0], MIN_PANEL[1])));
}

#[test]
fn a_cut_that_breaks_a_minimum_is_dropped_but_another_is_kept() {
    // Room for two Panels stacked or side by side, not three in a row: the result still fits.
    let area = [
        0.0,
        0.0,
        MIN_PANEL[0] * 2.0 + GAP + 4.0,
        MIN_PANEL[1] * 2.0 + GAP + 4.0,
    ];
    let names = &NAMES[..4];
    let mut t = LayoutTree::automatic(4);
    t.auto_layout(&items(names), 16.0 / 9.0, area, GAP, 1.0)
        .expect("a 2 x 2 fits");
    for c in t.cells(area, GAP) {
        assert!(
            c[2] >= MIN_PANEL[0] - 0.5 && c[3] >= MIN_PANEL[1] - 0.5,
            "{c:?}"
        );
    }
}

#[test]
fn above_nine_panels_a_greedy_halving_runs_and_is_quick() {
    let refs: Vec<&str> = std::iter::once("Clean picture")
        .chain(std::iter::repeat_n("Skin tone", 11))
        .collect();
    let area = [0.0, 0.0, 3840.0, 2160.0];
    let mut t = LayoutTree::automatic(12);
    let start = Instant::now();
    t.auto_layout(&items(&refs), 16.0 / 9.0, area, GAP, 1.0)
        .expect("fits at 4K");
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(t.panels(), (0..12).collect::<Vec<_>>());
    let min = min_panel_size(1.0);
    for c in t.cells(area, GAP) {
        assert!(c[2] >= min[0] - 0.5 && c[3] >= min[1] - 0.5, "{c:?}");
    }
    // Halves of the ordered list, so the first cut is between the sixth and seventh Panel.
    let c = t.cells(area, GAP);
    let left: f32 = c[..6].iter().map(|r| r[0] + r[2]).fold(0.0, f32::max);
    let right: f32 = c[6..].iter().map(|r| r[0]).fold(f32::MAX, f32::min);
    let top: f32 = c[..6].iter().map(|r| r[1] + r[3]).fold(0.0, f32::max);
    let below: f32 = c[6..].iter().map(|r| r[1]).fold(f32::MAX, f32::min);
    assert!(left <= right || top <= below, "the first six are one half");
    // Too many for a small Window: nothing changes.
    let mut small = LayoutTree::automatic(12);
    let before = small.to_text(&refs);
    assert_eq!(
        small.auto_layout(
            &items(&refs),
            16.0 / 9.0,
            [0.0, 0.0, 600.0, 400.0],
            GAP,
            1.0
        ),
        Err(NoFit)
    );
    assert_eq!(small.to_text(&refs), before);
}

#[test]
fn seven_panels_take_well_under_the_budget() {
    // About 100 ms at 7 Panels is the budget (spec #196, Size of the search).
    let mut worst = Duration::ZERO;
    for _ in 0..5 {
        let mut t = default_tree();
        let start = Instant::now();
        t.auto_layout(&items(&NAMES), 16.0 / 9.0, WINDOW, GAP, 1.0)
            .expect("fits");
        worst = worst.max(start.elapsed());
    }
    eprintln!("auto layout, 7 Panels: worst of 5 runs {worst:?}");
    assert!(worst < Duration::from_millis(100), "{worst:?}");
}

#[test]
fn nine_panels_are_still_searched_within_a_second() {
    let nine = [
        "Clean picture",
        "Luma waveform",
        "Vectorscope",
        "ARRI False Colour",
        "Saturation heatmap",
        "Skin tone",
        "Stops False Colour",
        "Channel view",
        "Black and white",
    ];
    let mut t = LayoutTree::automatic(9);
    let start = Instant::now();
    t.auto_layout(
        &items(&nine),
        16.0 / 9.0,
        [0.0, 0.0, 2512.0, 1432.0],
        GAP,
        1.0,
    )
    .expect("fits");
    eprintln!("auto layout, 9 Panels: {:?}", start.elapsed());
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn it_is_one_undo_step_and_the_focus_stays() {
    let mut start = default_tree();
    start.set_focus(3);
    let mut pages = Pages::new(start);
    let before = pages.tree().to_text(&NAMES);
    let mut outcome = Ok(());
    pages.change(|t| outcome = t.auto_layout(&items(&NAMES), 2.39, WINDOW, GAP, 1.0));
    assert_eq!(outcome, Ok(()));
    assert_ne!(pages.tree().to_text(&NAMES), before);
    assert_eq!(pages.tree().focused(), Some(3), "the focused Panel is kept");
    assert!(pages.undo());
    assert_eq!(pages.tree().to_text(&NAMES), before);
    assert_eq!(pages.tree().focused(), Some(3));
    assert!(!pages.can_undo(), "one step");
}

#[test]
fn a_press_that_finds_no_fit_is_no_undo_step() {
    let tiny = [0.0, 0.0, MIN_PANEL[0] * 2.0, MIN_PANEL[1] * 2.0];
    let mut pages = Pages::new(default_tree());
    pages.change(|t| {
        let _ = t.auto_layout(&items(&NAMES), 16.0 / 9.0, tiny, GAP, 1.0);
    });
    assert!(!pages.can_undo());
}
