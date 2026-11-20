//! The waveform's scale numbers at small sizes: which stay when they would overlap (the Panel grid itself is
//! tested in `panel_grid.rs`).
use scopecreep::waveform::{PlotLayout, marks_that_fit};

const GUTTER_PX: f32 = scopecreep::layout::GUTTER;

/// The scale's marks top to bottom as the app names them: "> 1023", 940, 512, 64, "< 0", each with how much it
/// matters (lower first): black and white, then the edge rows, then the middle.
fn scale_marks(plot: &PlotLayout) -> Vec<(f32, u32)> {
    let edge = |r: [f32; 4]| r[1] + r[3] / 2.0;
    vec![
        (edge(plot.above()), 1),
        (plot.y_of_code(940), 0),
        (plot.y_of_code(512), 2),
        (plot.y_of_code(64), 0),
        (edge(plot.below()), 1),
    ]
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
    assert_eq!(
        marks_that_fit(&scale_marks(&plot), 14.3),
        vec![false, true, false, false, true]
    );
}

/// The plot's width and height as a ratio, to compare with the frame's.
fn ratio(plot: [f32; 4]) -> f32 {
    plot[2] / plot[3]
}

#[test]
fn the_plot_has_the_frames_aspect_for_16_9_2_39_4_3_and_9_16() {
    for (frame, name) in [
        ([1920, 1080], "16:9"),
        ([2048, 858], "2.39:1"),
        ([1440, 1080], "4:3"),
        ([1080, 1920], "9:16"),
    ] {
        for area in [
            [0.0, 0.0, 800.0, 500.0],
            [0.0, 0.0, 300.0, 700.0],
            [10.0, 20.0, 1000.0, 1000.0],
        ] {
            let plot = PlotLayout::fitted(area, 1.0, frame).plot();
            let want = frame[0] as f32 / frame[1] as f32;
            // Within a device pixel in each side: the sizes are rounded to whole pixels.
            assert!(
                (plot[2] - plot[3] * want).abs() <= 1.0 + want,
                "{name} in {area:?}: plot {plot:?} ratio {}",
                ratio(plot)
            );
        }
    }
}

#[test]
fn the_plot_is_centred_with_the_gutter_and_edge_rows_inside_the_panel() {
    let layout = PlotLayout::fitted([0.0, 0.0, 800.0, 500.0], 1.0, [1920, 1080]);
    let [l, t, w, h] = layout.plot();
    assert!(l >= GUTTER_PX && l + w <= 800.0 && t >= 0.0 && t + h <= 500.0);
    // The gutter and plot are centred as one block.
    assert!(((l - GUTTER_PX) - (800.0 - (GUTTER_PX + w)) / 2.0).abs() <= 1.0);
    // The plot and its two edge rows are centred as one stack.
    let (above, below) = (layout.above(), layout.below());
    assert!(above[1] >= 0.0 && below[1] + below[3] <= 500.0);
    assert!(
        (above[1] - (500.0 - (below[1] + below[3]))).abs() <= 1.0,
        "equal space above the top row and below the bottom row"
    );
    assert_eq!(layout.gutter_left(), l - GUTTER_PX);
}

#[test]
fn a_tall_frame_in_a_wide_panel_is_narrow_and_a_wide_frame_in_a_tall_panel_is_short() {
    let tall = PlotLayout::fitted([0.0, 0.0, 800.0, 500.0], 1.0, [1080, 1920]).plot();
    assert!(tall[2] < tall[3] && tall[3] > 400.0, "{tall:?}");
    let wide = PlotLayout::fitted([0.0, 0.0, 300.0, 700.0], 1.0, [2048, 858]).plot();
    assert!(wide[2] > wide[3] * 2.0 && wide[3] < 200.0, "{wide:?}");
}

#[test]
fn a_new_resolution_gives_a_new_plot() {
    let area = [0.0, 0.0, 800.0, 500.0];
    assert_ne!(
        PlotLayout::fitted(area, 1.0, [1920, 1080]),
        PlotLayout::fitted(area, 1.0, [3840, 1600])
    );
}

#[test]
fn black_and_white_keep_their_numbers_on_a_short_fitted_plot() {
    let plot = PlotLayout::fitted([0.0, 0.0, 600.0, 50.0], 1.0, [1920, 1080]);
    let shown = marks_that_fit(&scale_marks(&plot), 14.3);
    assert!(
        shown[1] || shown[3],
        "at least white or black stays: {shown:?}"
    );
}
