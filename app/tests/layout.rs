//! Where the window's parts go: the Picture View and the waveform take the Feed frame's shape and the same size,
//! each letterboxed in its half of the window, with room kept for the scale's numbers and the labels.
use scopecreep::layout::{Chrome, WindowLayout};
use scopecreep::waveform::{PlotLayout, marks_that_fit};

/// The chrome at 1x: a one-line label strip (30) and a one-row status line (20).
const ONE_LINE: Chrome = Chrome { label_strip: 30.0, status: 20.0 };

fn layout(window: [f32; 2], frame: [u32; 2]) -> WindowLayout {
    WindowLayout::new(window, 1.0, Some(frame), ONE_LINE)
}

#[test]
fn a_16_9_frame_in_the_default_window() {
    // Worked by hand: halves of 640; the right half loses the 44 px gutter, so it sets the width (578 px); a
    // 16:9 picture 578 wide is 325 high; the panels and their label strip are centred in the height left above
    // the status line.
    let l = layout([1280.0, 760.0], [1920, 1080]);
    assert_eq!(l.picture, [34.0, 199.0, 578.0, 325.0]);
    assert_eq!(l.waveform, [690.0, 199.0, 578.0, 325.0]);
    assert_eq!(l.gutter, [646.0, 199.0, 44.0, 325.0]);
}

/// The two panels are the same size, level with each other, in the frame's shape (to the pixel), and each inside
/// its own half of the window.
fn assert_panels_match(l: &WindowLayout, window: [f32; 2], frame: [u32; 2]) {
    let [px, py, pw, ph] = l.picture;
    let [wx, wy, ww, wh] = l.waveform;
    assert_eq!((pw, ph, py), (ww, wh, wy), "same size and level: {l:?}");
    let shape = frame[0] as f32 / frame[1] as f32;
    assert!((pw - ph * shape).abs() <= shape.max(1.0), "the frame's shape {shape}: {pw}x{ph}");
    let half = window[0] / 2.0;
    assert!(px >= 0.0 && px + pw <= half, "the picture is in the left half: {l:?}");
    assert!(l.gutter[0] >= half && wx + ww <= window[0], "the waveform and its gutter are in the right half: {l:?}");
    assert_eq!(l.gutter[0] + l.gutter[2], wx, "the gutter is directly left of the waveform");
    assert!(py >= 0.0 && l.label_top + ONE_LINE.label_strip + ONE_LINE.status <= window[1], "the labels and status fit: {l:?}");
}

#[test]
fn a_scope_wide_frame_fills_the_width_and_is_letterboxed_top_and_bottom() {
    let l = layout([1280.0, 760.0], [4096, 1716]);
    assert_panels_match(&l, [1280.0, 760.0], [4096, 1716]);
    assert_eq!((l.picture[2], l.picture[3]), (578.0, 242.0), "2.39:1 at 578 wide");
}

#[test]
fn a_4_3_frame() {
    let l = layout([1280.0, 760.0], [1440, 1080]);
    assert_panels_match(&l, [1280.0, 760.0], [1440, 1080]);
    assert_eq!((l.picture[2], l.picture[3]), (578.0, 433.0), "4:3 at 578 wide");
}

#[test]
fn a_portrait_frame_is_set_by_the_height_and_centred_in_each_half() {
    let l = layout([1280.0, 760.0], [1080, 1920]);
    assert_panels_match(&l, [1280.0, 760.0], [1080, 1920]);
    // The full height above the label strip and status line: 760 - 12 - 30 - 20 = 698; 9:16 of that is 392 wide.
    assert_eq!((l.picture[1], l.picture[3], l.picture[2]), (12.0, 698.0, 392.0));
    assert_eq!(l.picture[0], 12.0 + ((622.0f32 - 392.0) / 2.0).round(), "centred in the left half");
    assert_eq!(l.waveform[0], 690.0 + ((578.0f32 - 392.0) / 2.0).round(), "centred in the right half, after the gutter");
}

#[test]
fn a_tall_window_letterboxes_a_16_9_frame_top_and_bottom() {
    let l = layout([1000.0, 1400.0], [1920, 1080]);
    assert_panels_match(&l, [1000.0, 1400.0], [1920, 1080]);
}

#[test]
fn a_tiny_window_keeps_the_whole_gutter_for_the_scale_numbers() {
    for window in [[320.0, 200.0], [200.0, 120.0], [120.0, 80.0]] {
        let l = layout(window, [1920, 1080]);
        assert_eq!(l.gutter[2], 44.0, "the scale numbers keep their room at {window:?}");
        assert!(l.picture[2] >= 1.0 && l.picture[3] >= 1.0 && l.waveform[2] == l.picture[2], "{window:?}: {l:?}");
        assert!(l.picture[0] >= 0.0 && l.waveform[0] + l.waveform[2] <= window[0].max(l.waveform[0] + 1.0), "{window:?}: {l:?}");
    }
    let l = layout([320.0, 200.0], [1920, 1080]);
    assert_panels_match(&l, [320.0, 200.0], [1920, 1080]);
}

#[test]
fn the_gutter_and_margins_follow_the_display_scale() {
    let l = WindowLayout::new([2560.0, 1520.0], 2.0, Some([1920, 1080]), Chrome { label_strip: 60.0, status: 40.0 });
    assert_eq!(l.gutter[2], 88.0);
    assert_eq!(l.picture, [68.0, 397.0, 1156.0, 650.0], "24 + (1520 - 24 - 40 - 60 - 650) / 2 = 397");
}

#[test]
fn a_new_frame_size_lays_the_panels_out_again() {
    let wide = layout([1280.0, 760.0], [1920, 1080]);
    let scope = layout([1280.0, 760.0], [4096, 1716]);
    assert_ne!(wide.picture, scope.picture);
    assert_eq!(scope.waveform[3], scope.picture[3], "the waveform follows the new shape too");
}

#[test]
fn before_the_first_frame_the_panels_are_laid_out_for_16_9() {
    let none = WindowLayout::new([1280.0, 760.0], 1.0, None, ONE_LINE);
    assert_eq!(none, layout([1280.0, 760.0], [1920, 1080]));
}

#[test]
fn the_labels_sit_under_the_panels_and_run_to_the_end_of_their_half() {
    let l = layout([1280.0, 760.0], [1080, 1920]);
    assert_eq!(l.label_top, l.picture[1] + l.picture[3]);
    assert_eq!(l.picture[0] + l.picture_label_width, 634.0, "to the left half's inner edge (640 - 6)");
    assert_eq!(l.waveform[0] + l.waveform_label_width, 1268.0, "to the window's right margin");
}

#[test]
fn a_wrapped_label_strip_shrinks_the_panels_to_keep_it_on_screen() {
    let one = layout([800.0, 400.0], [1080, 1920]);
    let three = WindowLayout::new([800.0, 400.0], 1.0, Some([1080, 1920]), Chrome { label_strip: 65.0, status: 20.0 });
    assert!(three.picture[3] < one.picture[3]);
    assert!(three.label_top + 65.0 + 20.0 <= 400.0);
}

#[test]
fn the_waveform_plot_lies_exactly_on_its_panel_so_columns_line_up_with_the_picture() {
    for (window, frame) in [([1280.0, 760.0], [1920, 1080]), ([320.0, 200.0], [4096, 1716]), ([200.0, 400.0], [1080, 1920])] {
        let l = layout(window, frame);
        let plot = PlotLayout::new(l.waveform_area(), 1.0);
        assert_eq!((plot.plot()[0], plot.plot()[2]), (l.waveform[0], l.waveform[2]), "{window:?} {frame:?}");
        assert_eq!((plot.above()[1], plot.below()[1] + plot.below()[3]), (l.waveform[1], l.waveform[1] + l.waveform[3]), "edge rows inside the panel");
        // Picture column 100 of the frame is at the same distance from each panel's left edge.
        let in_picture = l.picture[0] + (100.0 + 0.5) / frame[0] as f32 * l.picture[2];
        assert!((plot.x_of_column(100, frame[0]) - l.waveform[0] - (in_picture - l.picture[0])).abs() < 0.01);
    }
}

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
