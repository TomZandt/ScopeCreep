//! The luma waveform, driven at the Feed (spec #2, "Testing Decisions"): synthetic frames go in through the
//! synthetic writer, the app's Feed monitor hands them to the GPU exactly as the app does, and the tests look at
//! what a colourist would see: how many pixels landed at each code value of each column, the label, and the
//! waveform as drawn. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::colour_setup::{Levels, Provenance};
use scopecreep::waveform::{Brightness, Look, LumaWeights, MeasuredSpace, Measurement, PlotLayout, Waveform, WaveformCounts};

/// A stand-in Resolve (the synthetic writer), the app's Feed monitor, and the app's GPU side of the Feed: the
/// frame upload and the waveform.
struct Bench {
    rig: Rig,
    waveform: Waveform,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let mut b = Bench { waveform: Waveform::new(&rig.device, &rig.queue, TARGET), rig };
        // Full levels unless a test says otherwise: a value's code is then simply value × 1023.
        b.waveform.set_measurement(measuring(MeasuredSpace::Rec709, LumaWeights::FromPrimaries));
        b.poll();
        b
    }

    fn poll(&mut self) {
        let waveform = &self.waveform;
        self.rig.poll(|encoder, gpu_frame| waveform.measure(encoder, gpu_frame));
    }

    /// Resolve plays one frame (RGBA, bottom row first) and the app has a moment to take it.
    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) -> WaveformCounts {
        let waveform = &self.waveform;
        self.rig.play(width, height, pixels, |rig| rig.poll(|encoder, gpu_frame| waveform.measure(encoder, gpu_frame)));
        self.waveform.counts()
    }
}

/// A picture `width` × `height` where each pixel's red, green and blue are given by `rgb(x, row from the top)`.
fn picture(width: u32, height: u32, rgb: impl Fn(u32, u32) -> [f32; 3]) -> Vec<f32> {
    let mut out = Vec::with_capacity(width as usize * height as usize * 4);
    for row in 0..height {
        let from_top = height - 1 - row; // the Feed stores rows bottom first
        for x in 0..width {
            let [r, g, b] = rgb(x, from_top);
            out.extend_from_slice(&[r, g, b, 1.0]);
        }
    }
    out
}

/// The grey whose luma is exactly 10-bit code `code`.
fn grey(code: u32) -> [f32; 3] {
    let v = code as f32 / 1023.0;
    [v, v, v]
}

// ------------------------------------------------------------------ ramps

#[test]
fn a_horizontal_ramp_puts_every_pixel_of_column_x_at_code_x() {
    let mut b = Bench::new("hramp");
    let counts = b.play(1024, 9, &picture(1024, 9, |x, _| grey(x)));
    assert_eq!(counts.columns(), 1024, "one waveform column per picture column");
    for x in 0..1024 {
        for code in 0..1024 {
            assert_eq!(counts.at(x, code), if code == x { 9 } else { 0 }, "column {x}, code {code}");
        }
        assert_eq!((counts.below(x), counts.above(x)), (0, 0), "column {x}: nothing out of range");
    }
}

#[test]
fn a_vertical_ramp_puts_one_pixel_of_every_column_at_every_code() {
    let mut b = Bench::new("vramp");
    // Three columns: an odd width, so the rows of the frame are not a convenient size for the GPU.
    let counts = b.play(3, 1024, &picture(3, 1024, |_, from_top| grey(1023 - from_top)));
    assert_eq!(counts.columns(), 3);
    for x in 0..3 {
        for code in 0..1024 {
            assert_eq!(counts.at(x, code), 1, "column {x}, code {code}");
        }
    }
    assert_eq!(counts.total(), 3 * 1024, "every pixel counted once");
}

// ------------------------------------------------------------------ luma weights

/// Plays three columns of half-strength red, green and blue, and returns the code each one lands on.
fn half_primaries_codes(b: &mut Bench) -> [u32; 3] {
    let counts = b.play(3, 4, &picture(3, 4, |x, _| {
        let mut rgb = [0.0; 3];
        rgb[x as usize] = 0.5;
        rgb
    }));
    [0, 1, 2].map(|x| (0..1024).find(|&code| counts.at(x, code) == 4).unwrap_or_else(|| panic!("column {x} is not on one code")))
}

/// Full levels, so that a value's code is simply value × 1023.
fn measuring(space: MeasuredSpace, luma: LumaWeights) -> Measurement {
    Measurement { space, assumed: false, luma, levels: Levels::Full, levels_from: Some(Provenance::Project) }
}

// Expected codes: 0.5 × weight × 1023, rounded, with the weights ADR 0004 quotes for each space.
#[test]
fn rec709_luma_uses_the_rec709_weights() {
    let mut b = Bench::new("w709");
    b.waveform.set_measurement(measuring(MeasuredSpace::Rec709, LumaWeights::FromPrimaries));
    assert_eq!(half_primaries_codes(&mut b), [109, 366, 37]); // 0.2126, 0.7152, 0.0722
    assert!(b.waveform.label().ends_with("Rec.709  ·  Y = 0.2126 R + 0.7152 G + 0.0722 B from primaries"), "{}", b.waveform.label());
}

#[test]
fn p3_d65_luma_uses_weights_derived_from_its_primaries() {
    let mut b = Bench::new("wp3");
    b.waveform.set_measurement(measuring(MeasuredSpace::P3D65, LumaWeights::FromPrimaries));
    assert_eq!(half_primaries_codes(&mut b), [117, 354, 41]); // 0.2290, 0.6917, 0.0793
    assert!(b.waveform.label().ends_with("P3-D65  ·  Y = 0.2290 R + 0.6917 G + 0.0793 B from primaries"), "{}", b.waveform.label());
}

#[test]
fn rec2020_luma_uses_the_rec2020_weights() {
    let mut b = Bench::new("w2020");
    b.waveform.set_measurement(measuring(MeasuredSpace::Rec2020, LumaWeights::FromPrimaries));
    assert_eq!(half_primaries_codes(&mut b), [134, 347, 30]); // 0.2627, 0.6780, 0.0593
    assert!(b.waveform.label().ends_with("Rec.2020  ·  Y = 0.2627 R + 0.6780 G + 0.0593 B from primaries"), "{}", b.waveform.label());
}

#[test]
fn match_resolve_uses_rec709_weights_for_p3() {
    let mut b = Bench::new("wmatchp3");
    b.waveform.set_measurement(measuring(MeasuredSpace::P3D65, LumaWeights::MatchResolve));
    assert_eq!(half_primaries_codes(&mut b), [109, 366, 37]);
    assert!(b.waveform.label().ends_with("P3-D65  ·  Y = 0.2126 R + 0.7152 G + 0.0722 B to match Resolve"), "{}", b.waveform.label());
    b.waveform.set_measurement(measuring(MeasuredSpace::P3Dci, LumaWeights::MatchResolve));
    assert_eq!(half_primaries_codes(&mut b), [109, 366, 37]);
}

#[test]
fn match_resolve_uses_rec2020_weights_for_rec2100() {
    let mut b = Bench::new("wmatch2100");
    b.waveform.set_measurement(measuring(MeasuredSpace::Rec2100Pq, LumaWeights::MatchResolve));
    assert_eq!(half_primaries_codes(&mut b), [134, 347, 30]);
    assert!(b.waveform.label().ends_with("Rec.2100 PQ  ·  Y = 0.2627 R + 0.6780 G + 0.0593 B to match Resolve"), "{}", b.waveform.label());
}

#[test]
fn until_the_colour_setup_is_known_the_label_says_rec709_and_video_levels_are_assumed() {
    let mut b = Bench::new("assumed");
    b.waveform.set_measurement(Measurement::default());
    // 64 + 0.5 × weight × 876 with the Rec.709 weights.
    assert_eq!(half_primaries_codes(&mut b), [157, 377, 96]);
    assert_eq!(b.waveform.label(), "Luma  ·  10-bit Legal 64–940 assumed  ·  Rec.709 assumed, measured as sent, unconverted  ·  Y = 0.2126 R + 0.7152 G + 0.0722 B from primaries");
}

// ------------------------------------------------------------------ Levels

/// Plays one column per grey value and returns, for each, the code it lands on, or "below" or "above".
fn codes_of_greys(b: &mut Bench, greys: &[f32]) -> Vec<String> {
    let n = greys.len() as u32;
    let counts = b.play(n, 4, &picture(n, 4, |x, _| [greys[x as usize]; 3]));
    (0..n)
        .map(|x| match (0..1024).find(|&code| counts.at(x, code) == 4) {
            Some(code) => code.to_string(),
            None if counts.below(x) == 4 => "below".into(),
            None if counts.above(x) == 4 => "above".into(),
            None => panic!("column {x} is not on one code"),
        })
        .collect()
}

fn levels(levels: Levels, from: Option<Provenance>) -> Measurement {
    Measurement { space: MeasuredSpace::Rec709, assumed: false, levels, levels_from: from, ..Measurement::default() }
}

// Video levels put black (0.0) at code 64 and white (1.0) at code 940: 876 codes for 0 to 1. Code 0 is
// -64/876 = -0.0731 and code 1023 is 959/876 = 1.0947.
#[test]
fn video_levels_put_black_at_64_and_white_at_940() {
    let mut b = Bench::new("legal");
    b.waveform.set_measurement(levels(Levels::Video, Some(Provenance::Project)));
    let codes = codes_of_greys(&mut b, &[0.0, 1.0, 0.5, 0.25, -0.0731, 1.0947, -0.08, 1.1]);
    assert_eq!(codes, ["64", "940", "502", "283", "0", "1023", "below", "above"]);
}

#[test]
fn full_levels_put_black_at_0_and_white_at_1023() {
    let mut b = Bench::new("full");
    b.waveform.set_measurement(levels(Levels::Full, Some(Provenance::Project)));
    let codes = codes_of_greys(&mut b, &[0.0, 1.0, 0.25, 0.5005, -0.001, 1.001]);
    assert_eq!(codes, ["0", "1023", "256", "512", "below", "above"]);
}

// ------------------------------------------------------------------ out of range

/// An HD frame of mid grey with one pixel far above the scale at column 1000 and one below it at column 7.
fn grey_with_two_stray_pixels() -> Vec<f32> {
    picture(1920, 1080, |x, from_top| match (x, from_top) {
        (1000, 300) => [1.5, 1.5, 1.5],
        (7, 900) => [-0.2, -0.2, -0.2],
        _ => grey(512),
    })
}

#[test]
fn a_single_out_of_range_pixel_is_counted_in_its_column() {
    let mut b = Bench::new("stray");
    let counts = b.play(1920, 1080, &grey_with_two_stray_pixels());
    assert_eq!(counts.total(), 1920 * 1080, "no pixel is cut off");
    for x in 0..1920 {
        assert_eq!(counts.above(x), if x == 1000 { 1 } else { 0 }, "above 1023, column {x}");
        assert_eq!(counts.below(x), if x == 7 { 1 } else { 0 }, "below 0, column {x}");
    }
    assert_eq!(counts.at(1000, 1023), 0, "not piled onto code 1023");
    assert_eq!(counts.at(7, 0), 0, "not piled onto code 0");
}

/// Draws the waveform into a `width` × `height` picture and returns it as RGBA bytes, top row first.
fn draw(b: &Bench, width: u32, height: u32) -> Vec<u8> {
    b.rig.render(width, height, |pass| b.waveform.draw(pass, [0.0, 0.0, width as f32, height as f32], [width as f32, height as f32], 1.0))
}

#[test]
fn a_single_out_of_range_pixel_is_visible_in_the_edge_rows() {
    let mut b = Bench::new("strayvisible");
    b.play(1920, 1080, &grey_with_two_stray_pixels());
    let (width, height) = (800, 500);
    let image = draw(&b, width, height);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let brightness = |x: f32, y: f32| image[(y as usize * width as usize + x as usize) * 4] as u32;
    let brightest_in = |rect: [f32; 4], column: u32| {
        let x = layout.x_of_column(column, 1920);
        (rect[1] as u32..(rect[1] + rect[3]) as u32).map(|y| brightness(x, y as f32)).max().unwrap()
    };
    let (above, below) = (layout.above(), layout.below());
    assert!(brightest_in(above, 1000) >= 128, "the pixel above 1023 lights the top edge row: {}", brightest_in(above, 1000));
    assert!(brightest_in(above, 1400) < 64, "elsewhere the top edge row stays dark: {}", brightest_in(above, 1400));
    assert!(brightest_in(below, 7) >= 128, "the pixel below 0 lights the bottom edge row: {}", brightest_in(below, 7));
    assert!(brightest_in(below, 1400) < 64, "elsewhere the bottom edge row stays dark: {}", brightest_in(below, 1400));
    let mid_grey = layout.y_of_code(512);
    assert!(brightness(layout.x_of_column(1400, 1920), mid_grey) >= 128, "the mid-grey trace is drawn");
}

/// How bright the empty plot is drawn at each code's row, away from any trace (the graticule).
fn graticule_at(b: &mut Bench, codes: &[u32]) -> Vec<u8> {
    b.play(64, 4, &picture(64, 4, |_, _| [0.5; 3]));
    let (width, height) = (400, 1100);
    let image = draw(b, width, height);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let x = layout.x_of_column(10, 64) as usize;
    codes.iter().map(|&code| image[(layout.y_of_code(code).floor() as usize * width as usize + x) * 4]).collect()
}

#[test]
fn the_legal_lines_mark_black_and_white_for_the_levels_in_use() {
    let mut b = Bench::new("legallines");
    b.waveform.set_measurement(levels(Levels::Video, Some(Provenance::Project)));
    let [black, white, between] = graticule_at(&mut b, &[64, 940, 600])[..] else { unreachable!() };
    assert!(black > between + 20 && white > between + 20, "video levels: lines at 64 and 940 ({black}, {white}, plot {between})");

    b.waveform.set_measurement(levels(Levels::Full, Some(Provenance::Project)));
    let [at_64, at_940, black, white, between] = graticule_at(&mut b, &[64, 940, 0, 1023, 600])[..] else { unreachable!() };
    assert_eq!((at_64, at_940), (between, between), "full levels: no line at 64 or 940");
    assert!(black > between + 20 && white > between + 20, "full levels: lines at 0 and 1023 ({black}, {white}, plot {between})");
}

// ------------------------------------------------------------------ the phosphor look (#53)

/// Drawn brightness (the red byte, 0 to 255) of the plot at `code`'s row, in the screen column of picture column
/// `column`.
fn drawn_at(image: &[u8], width: u32, layout: &PlotLayout, column: u32, columns: u32, code: u32) -> f32 {
    let (x, y) = (layout.x_of_column(column, columns) as usize, layout.y_of_code(code).floor() as usize);
    image[(y * width as usize + x) * 4] as f32
}

/// The mean of `drawn_at` over the codes in `codes`, across picture columns `from..to`.
fn mean_drawn(image: &[u8], width: u32, layout: &PlotLayout, columns: u32, codes: std::ops::Range<u32>, from: u32, to: u32) -> f32 {
    let mut sum = 0.0;
    let mut n = 0.0;
    for code in codes {
        for column in (from..to).step_by(7) {
            sum += drawn_at(image, width, layout, column, columns, code);
            n += 1.0;
        }
    }
    sum / n
}

fn look(brightness: Brightness, spread: bool) -> Look {
    Look { brightness, spread }
}

/// A frame whose tones thin out towards white: code = 1023 · (row / height)², so each column holds many pixels
/// at low codes and few at high codes.
fn thinning_gradient() -> Vec<f32> {
    picture(1920, 1080, |_, from_top| {
        let t = from_top as f32 / 1079.0;
        grey((1023.0 * t * t).round() as u32)
    })
}

const PLOT_W: u32 = 800;
const PLOT_H: u32 = 500;

#[test]
fn the_trace_is_brighter_where_more_pixels_land_and_is_not_a_flat_floor() {
    let mut b = Bench::new("phosphor-density");
    b.play(1920, 1080, &thinning_gradient());
    b.waveform.set_look(look(Brightness::Normal, false));
    let image = draw(&b, PLOT_W, PLOT_H);
    let layout = PlotLayout::new([0.0, 0.0, PLOT_W as f32, PLOT_H as f32], 1.0);
    let dense = mean_drawn(&image, PLOT_W, &layout, 1920, 40..80, 1000, 1400);
    let sparse = mean_drawn(&image, PLOT_W, &layout, 1920, 880..920, 1000, 1400);
    let floor = (0.035 + 0.08 * (0.86 - 0.035)) * 255.0;
    assert!(dense > sparse * 1.5, "dense codes {dense}, sparse codes {sparse}");
    assert!(sparse >= floor - 2.0, "sparse still shows at the floor: {sparse} (floor {floor})");
    assert!(dense > floor + 20.0, "dense is well above the floor: {dense}");
}

#[test]
fn one_stray_pixel_shows_in_the_plot_but_is_clearly_dimmer_than_a_dense_area() {
    let mut b = Bench::new("phosphor-stray");
    // Mid grey everywhere, and one pixel at code 800 in column 1000.
    b.play(1920, 1080, &picture(1920, 1080, |x, from_top| if (x, from_top) == (1000, 300) { grey(800) } else { grey(600) }));
    b.waveform.set_look(look(Brightness::Normal, false));
    let image = draw(&b, PLOT_W, PLOT_H);
    let layout = PlotLayout::new([0.0, 0.0, PLOT_W as f32, PLOT_H as f32], 1.0);
    let (stray, dense) = (drawn_at(&image, PLOT_W, &layout, 1000, 1920, 800), drawn_at(&image, PLOT_W, &layout, 1000, 1920, 600));
    let empty = drawn_at(&image, PLOT_W, &layout, 1000, 1920, 700);
    assert!(stray >= 0.035 * 255.0 + 0.07 * 0.825 * 255.0, "the stray pixel shows (at least about 8 %): {stray}, empty {empty}");
    assert!(stray < dense * 0.5, "and is clearly dimmer than a dense area: {stray} against {dense}");
}

#[test]
fn trace_brightness_changes_the_drawn_brightness() {
    let mut b = Bench::new("phosphor-gain");
    b.play(1920, 1080, &thinning_gradient());
    let layout = PlotLayout::new([0.0, 0.0, PLOT_W as f32, PLOT_H as f32], 1.0);
    let mut means = Vec::new();
    for brightness in [Brightness::Dimmer, Brightness::Normal, Brightness::Brighter] {
        b.waveform.set_look(look(brightness, false));
        let image = draw(&b, PLOT_W, PLOT_H);
        means.push(mean_drawn(&image, PLOT_W, &layout, 1920, 40..400, 1000, 1400));
    }
    assert!(means[0] < means[1] && means[1] < means[2], "dimmer < normal < brighter: {means:?}");
}

#[test]
fn soft_spread_off_draws_a_code_on_its_own_row_and_on_spreads_it_by_about_one_pixel() {
    let mut b = Bench::new("phosphor-spread");
    b.play(1920, 1080, &picture(1920, 1080, |_, _| grey(600)));
    // A plot exactly 1024 pixels tall: one row per code.
    let (width, height) = (400, 1024 + 22);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let rows = |look: Look| {
        b.waveform.set_look(look);
        let image = draw(&b, width, height);
        [-4i32, -2, -1, 0, 1, 2, 4].map(|d| drawn_at(&image, width, &layout, 1000, 1920, (600 + d) as u32))
    };
    let off = rows(look(Brightness::Normal, false));
    let on = rows(look(Brightness::Normal, true));
    let base = off[0];
    assert!(off[3] > 200.0 && off[2] == base && off[4] == base && off[1] == base, "off: only the code's own row is lit {off:?}");
    assert!(on[2] > base + 10.0 && on[4] > base + 10.0, "on: the next rows are lit {on:?}");
    assert!(on[1] == base && on[5] == base && on[0] == base && on[6] == base, "on: nothing beyond about 1 pixel {on:?}");
    assert!(on[3] < off[3] + 1.0 && on[3] > 150.0, "the code's own row stays bright {on:?}");
}

#[test]
fn the_same_frame_looks_the_same_brightness_in_a_large_and_a_small_panel() {
    let mut b = Bench::new("phosphor-size");
    b.play(1920, 1080, &picture(1920, 1080, |_, from_top| grey((from_top as f32 * 1023.0 / 1079.0).round() as u32)));
    b.waveform.set_look(look(Brightness::Normal, false));
    let mean = |width: u32, height: u32| {
        let image = draw(&b, width, height);
        let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
        mean_drawn(&image, width, &layout, 1920, 300..700, 800, 1200)
    };
    let (small, large) = (mean(400, 300), mean(800, 600));
    assert!(small > 40.0 && large > 40.0, "the trace is drawn in both: {small}, {large}");
    assert!((small - large).abs() <= 0.15 * large, "small panel {small}, large panel {large}");
}
