//! The waveform's scale numbers at small sizes: which stay when they would overlap (the Panel grid itself is
//! tested in `panel_grid.rs`).
use scopecreep::waveform::{PlotLayout, marks_that_fit};

/// The scale's marks top to bottom as the app names them: "> 1023", 940, 512, 64, "< 0", each with how much it
/// matters (lower first): black and white, then the edge rows, then the middle.
fn scale_marks(plot: &PlotLayout) -> Vec<(f32, u32)> {
    let edge = |r: [f32; 4]| r[1] + r[3] / 2.0;
    vec![(edge(plot.above()), 1), (plot.y_of_code(940), 0), (plot.y_of_code(512), 2), (plot.y_of_code(64), 0), (edge(plot.below()), 1)]
}

#[test]
fn every_scale_number_shows_on_a_tall_waveform() {
    let plot = PlotLayout::new([0.0, 0.0, 600.0, 400.0], 1.0);
    assert_eq!(marks_that_fit(&scale_marks(&plot), 14.3), vec![true; 5]);
}

#[test]
fn on_a_short_waveform_the_scale_numbers_that_would_overlap_are_left_out_black_and_white_last() {
    // 50 px high: the plot is 28 px (from y 11); 940 is at y 13.3, 512 at 25.0 and 64 at 37.2, the edge rows at 2.5 and 47.5.
    let plot = PlotLayout::new([0.0, 0.0, 300.0, 50.0], 1.0);
    let shown = marks_that_fit(&scale_marks(&plot), 14.3);
    assert_eq!(shown, vec![false, true, false, true, false]);
    // Tiny (30 px): 940 and 64 are 7 px apart, so white keeps its number and black loses it; the "< 0" row, 16 px
    // below white, keeps its own.
    let plot = PlotLayout::new([0.0, 0.0, 100.0, 30.0], 1.0);
    assert_eq!(marks_that_fit(&scale_marks(&plot), 14.3), vec![false, true, false, false, true]);
}
