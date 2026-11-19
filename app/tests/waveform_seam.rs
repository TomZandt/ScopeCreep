//! The luma waveform, driven at the Feed (spec #2, "Testing Decisions"): synthetic frames go in through the
//! synthetic writer, the app's Feed monitor hands them to the GPU exactly as the app does, and the tests look at
//! what a colourist would see: how many pixels landed at each code value of each column, the label, and the
//! waveform as drawn. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::colour_setup::{Levels, Provenance};
use scopecreep::components::Channel;
use scopecreep::measurement::{LumaWeights, MeasuredSpace, Measurement};
use scopecreep::waveform::{Brightness, Channels, Look, PlotLayout, Spread, Waveform, WaveformCounts, SPREAD_AMOUNT};

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
    Look { brightness, spread: Spread(if spread { 1.0 } else { 0.0 }), graticule: 1.0 }
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
    b.waveform.set_look(look(Brightness(100.0), false));
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
    b.waveform.set_look(look(Brightness(100.0), false));
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
    for brightness in [Brightness(48.0), Brightness(100.0), Brightness(240.0)] {
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
    let off = rows(look(Brightness(100.0), false));
    let on = rows(look(Brightness(100.0), true));
    let base = off[0];
    assert!(off[3] > 200.0 && off[2] == base && off[4] == base && off[1] == base, "off: only the code's own row is lit {off:?}");
    assert!(on[2] > base + 10.0 && on[4] > base + 10.0, "on: the next rows are lit {on:?}");
    assert!(on[1] == base && on[5] == base && on[0] == base && on[6] == base, "on: nothing beyond about 1 pixel {on:?}");
    assert!(on[3] < off[3] + 1.0 && on[3] > 150.0, "the code's own row stays bright {on:?}");
}

#[test]
fn the_soft_spread_amount_runs_from_none_to_about_three_pixels_and_its_default_is_the_old_on() {
    let mut b = Bench::new("spread-amount");
    b.play(1920, 1080, &picture(1920, 1080, |_, _| grey(600)));
    let (width, height) = (400, 1024 + 22);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let rows = |b: &mut Bench, look: Look| {
        b.waveform.set_look(look);
        let image = draw(b, width, height);
        [0i32, 1, 2, 3, 4].map(|d| drawn_at(&image, width, &layout, 1000, 1920, (600 + d) as u32))
    };
    // The default is the old "on"; 0 is the old "off" (nothing beside the code's own row).
    assert_eq!(Look::default().spread, Spread(1.0));
    assert_eq!(rows(&mut b, Look::default()), rows(&mut b, look(Brightness(100.0), true)));
    let none = rows(&mut b, Look { brightness: Brightness(100.0), spread: Spread(0.0), graticule: 1.0 });
    assert_eq!(none, rows(&mut b, look(Brightness(100.0), false)));
    assert!(none[1] == none[4] && none[0] > 200.0, "0: only the code's own row {none:?}");
    // In between spreads between the two, and more spread reaches further and takes more from the centre.
    let mut last = none;
    for amount in [0.25, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0] {
        let now = rows(&mut b, Look { brightness: Brightness(100.0), spread: Spread(amount), graticule: 1.0 });
        assert!(now[1] >= last[1] && now[2] >= last[2] && now[3] >= last[3], "{amount}: spread grows {last:?} {now:?}");
        assert!(now[0] <= last[0], "{amount}: the centre gives way {last:?} {now:?}");
        last = now;
    }
    let half = rows(&mut b, Look { brightness: Brightness(100.0), spread: Spread(0.5), graticule: 1.0 });
    let one = rows(&mut b, Look::default());
    assert!(half[1] > none[1] + 1.0 && half[1] < one[1], "0.5 sits between off and on {none:?} {half:?} {one:?}");
    assert!(one[2] == none[2] && last[3] > none[3], "1 px reaches one pixel, 3 px reaches three {one:?} {last:?}");
    assert_eq!((SPREAD_AMOUNT.min, SPREAD_AMOUNT.max), (0.0, 3.0));
}

#[test]
fn the_same_frame_looks_the_same_brightness_in_a_large_and_a_small_panel() {
    let mut b = Bench::new("phosphor-size");
    b.play(1920, 1080, &picture(1920, 1080, |_, from_top| grey((from_top as f32 * 1023.0 / 1079.0).round() as u32)));
    b.waveform.set_look(look(Brightness(100.0), false));
    let mean = |width: u32, height: u32| {
        let image = draw(&b, width, height);
        let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
        mean_drawn(&image, width, &layout, 1920, 300..700, 800, 1200)
    };
    let (small, large) = (mean(400, 300), mean(800, 600));
    assert!(small > 40.0 && large > 40.0, "the trace is drawn in both: {small}, {large}");
    assert!((small - large).abs() <= 0.15 * large, "small panel {small}, large panel {large}");
}

// ------------------------------------------------------------------ the Y R G B chips (#117)

fn channels(y: bool, r: bool, g: bool, b: bool) -> Channels {
    Channels { y, r, g, b }
}

/// A picture whose red, green and blue are each flat at their own code (0 to 1023).
fn flat_codes(width: u32, height: u32, [r, g, b]: [u32; 3]) -> Vec<f32> {
    picture(width, height, |_, _| [r as f32 / 1023.0, g as f32 / 1023.0, b as f32 / 1023.0])
}

#[test]
fn each_channels_trace_lands_at_that_channels_code() {
    let mut b = Bench::new("channelcodes");
    b.waveform.set_channels(channels(true, true, true, true));
    b.play(4, 6, &flat_codes(4, 6, [200, 600, 900]));
    for (channel, code) in [(Channel::R, 200), (Channel::G, 600), (Channel::B, 900)] {
        let counts = b.waveform.channel_counts(channel);
        assert_eq!(counts.columns(), 4);
        for x in 0..4 {
            assert_eq!(counts.at(x, code), 6, "{channel:?}: column {x} at its code {code}");
            assert_eq!((counts.below(x), counts.above(x)), (0, 0));
        }
        assert_eq!(counts.total(), 24, "{channel:?}: every pixel counted once");
    }
    assert_eq!(b.waveform.counts().total(), 24, "Luma is measured from the same frame");
}

#[test]
fn out_of_range_is_counted_per_channel() {
    let mut b = Bench::new("channeledge");
    b.waveform.set_channels(channels(false, true, true, true));
    // Red over white, green under black, blue in range; on the middle column only.
    b.play(3, 2, &picture(3, 2, |x, _| if x == 1 { [1.2, -0.1, 0.5] } else { [0.5, 0.5, 0.5] }));
    let (r, g, bl) = (b.waveform.channel_counts(Channel::R), b.waveform.channel_counts(Channel::G), b.waveform.channel_counts(Channel::B));
    assert_eq!((r.above(1), r.below(1)), (2, 0));
    assert_eq!((g.above(1), g.below(1)), (0, 2));
    assert_eq!((bl.above(1), bl.below(1)), (0, 0));
    assert_eq!((r.above(0), g.below(0)), (0, 0));
}

#[test]
fn the_channels_are_drawn_in_their_own_colour_and_edge_rows_show_each_ones_out_of_range() {
    let mut b = Bench::new("channeldraw");
    // Red and blue on, green off: red is flat at 700 and runs over white in one column, blue is flat at 300.
    b.waveform.set_channels(channels(false, true, false, true));
    b.waveform.set_colorise(true);
    b.play(1920, 1080, &picture(1920, 1080, |x, _| [if x == 1000 { 1.3 } else { 700.0 / 1023.0 }, 0.0, 300.0 / 1023.0]));
    let (width, height) = (800, 500);
    let image = draw(&b, width, height);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let pixel = |x: f32, y: f32| {
        let at = (y as usize * width as usize + x as usize) * 4;
        [image[at] as i32, image[at + 1] as i32, image[at + 2] as i32]
    };
    let x = layout.x_of_column(400, 1920);
    let red = pixel(x, layout.y_of_code(700));
    let blue = pixel(x, layout.y_of_code(300));
    assert!(red[0] > 100 && red[0] > red[1] + 40 && red[0] > red[2] + 40, "red trace is red: {red:?}");
    assert!(blue[2] > 100 && blue[2] > blue[0] + 40, "blue trace is blue: {blue:?}");
    let green_row = pixel(x, layout.y_of_code(500));
    assert!(green_row[1] < 40, "no green trace is drawn with G off: {green_row:?}");
    let above = layout.above();
    let edge = pixel(layout.x_of_column(1000, 1920), above[1] + above[3] / 2.0);
    assert!(edge[0] > 100 && edge[0] > edge[2] + 40, "red out of range lights the top edge row in red: {edge:?}");
    let quiet = pixel(layout.x_of_column(1400, 1920), above[1] + above[3] / 2.0);
    assert!(quiet[0] < 64, "elsewhere the top edge row stays dark: {quiet:?}");
}

#[test]
fn luma_alone_is_the_default_and_a_setting_with_no_chip_on_falls_back_to_it() {
    assert_eq!(Channels::default(), channels(true, false, false, false));
    assert_eq!(Channels::from_setting(None), Channels::default());
    assert_eq!(Channels::from_setting(Some("")), Channels::default());
    assert_eq!(Channels::from_setting(Some("rb")), channels(false, true, false, true));
    assert!(Channels::default().is_default());
    assert_eq!(channels(true, true, false, true).names(), "Y R B");
}

#[test]
fn the_label_names_the_channels_when_not_just_luma_and_keeps_the_weights_only_with_luma() {
    use scopecreep::display::Display;
    let b = Bench::new("channellabel");
    assert!(Display::label(&b.waveform).starts_with("Luma  ·  10-bit"));
    b.waveform.set_channels(channels(false, true, true, false));
    let label = Display::label(&b.waveform);
    assert!(label.starts_with("R G  ·  10-bit"), "{label}");
    assert!(!label.contains("Y ="), "no luma weights without Luma: {label}");
    b.waveform.set_channels(channels(true, true, false, false));
    assert!(Display::label(&b.waveform).contains("Y = "));
}

#[test]
fn turning_the_colour_channels_off_stops_measuring_them() {
    let mut b = Bench::new("channeloff");
    b.waveform.set_channels(channels(true, true, false, false));
    b.play(2, 2, &flat_codes(2, 2, [300, 300, 300]));
    assert_eq!(b.waveform.channel_counts(Channel::R).total(), 4);
    b.waveform.set_channels(channels(true, false, false, false));
    b.play(2, 2, &flat_codes(2, 2, [300, 300, 300]));
    assert_eq!(b.waveform.channel_counts(Channel::R).columns(), 0, "the R, G and B counts are gone with their chips");
}

// ------------------------------------------------------------------ Colorise (#143, ADR 0032)

/// The drawn colour of the plot at `code` of picture column `column`.
fn colour_at(image: &[u8], width: u32, layout: &PlotLayout, column: u32, columns: u32, code: u32) -> [i32; 3] {
    let (x, y) = (layout.x_of_column(column, columns) as usize, layout.y_of_code(code).floor() as usize);
    let at = (y * width as usize + x) * 4;
    [image[at] as i32, image[at + 1] as i32, image[at + 2] as i32]
}

/// How far apart a drawn colour's strongest and weakest channels are: 0 for a neutral trace.
fn spread_of([r, g, bl]: [i32; 3]) -> i32 {
    r.max(g).max(bl) - r.min(g).min(bl)
}

/// A colour's channels against its strongest: the colour without its brightness.
fn proportions([r, g, bl]: [f32; 3]) -> [f32; 3] {
    let top = r.max(g).max(bl);
    [r / top, g / top, bl / top]
}

#[test]
fn colorise_is_off_by_default() {
    let b = Bench::new("colorisedefault");
    assert!(!b.waveform.colorise());
}

#[test]
fn colorise_on_draws_a_y_trace_in_the_colour_of_the_pixels_and_off_it_is_neutral() {
    let mut b = Bench::new("coloriseflat");
    let colour = [0.2f32, 0.8, 0.3];
    let counts = b.play(1920, 1080, &picture(1920, 1080, |_, _| colour));
    let code = (0..1024).find(|&c| counts.at(0, c) == 1080).expect("one code");
    let (width, height) = (800, 500);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);

    let neutral = colour_at(&draw(&b, width, height), width, &layout, 960, 1920, code);
    assert!(neutral[0] > 100 && spread_of(neutral) <= 3, "off: a white trace {neutral:?}");

    b.waveform.set_colorise(true);
    b.play(1920, 1080, &picture(1920, 1080, |_, _| colour));
    let tinted = colour_at(&draw(&b, width, height), width, &layout, 960, 1920, code);
    let [r, g, bl] = proportions(tinted.map(|v| v as f32));
    assert!((r - 0.25).abs() < 0.06 && (g - 1.0).abs() < 1e-6 && (bl - 0.375).abs() < 0.06, "on: a green trace {tinted:?}");
    let away = colour_at(&draw(&b, width, height), width, &layout, 960, 1920, code + 80);
    assert!(away.iter().all(|&c| c < 40), "the rest of the plot stays dark: {away:?}");
}

#[test]
fn colorise_on_a_colorchecker_frame_tints_each_patchs_trace_in_that_patchs_colour() {
    use scopecreep_accuracy::colorchecker::{ColorChecker, Patch, TimelineSpace};
    let mut b = Bench::new("colorisechart");
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    let frame = chart.frame(600, 400);
    let (width, height) = (1200, 700);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let weights = Measurement::default().weights();
    // Patches 13 (blue), 14 (green), 15 (red) and 16 (yellow): clearly coloured.
    let patches: Vec<Patch> = chart.patches()[12..16].to_vec();
    let at = |b: &Bench, p: &Patch| {
        let r = frame.patch_rect(p.number);
        let code = ((weights[0] * p.encoded[0] + weights[1] * p.encoded[1] + weights[2] * p.encoded[2]) * 1023.0).round() as u32;
        colour_at(&draw(b, width, height), width, &layout, (r.x0 + r.x1) / 2, 600, code)
    };
    b.play(600, 400, &frame.pixels);
    for p in &patches {
        assert!(spread_of(at(&b, p)) <= 3, "off: patch {} {} is neutral", p.number, p.name);
    }
    b.waveform.set_colorise(true);
    b.play(600, 400, &frame.pixels);
    for p in &patches {
        let want = proportions(p.encoded.map(|v| v as f32));
        let got = proportions(at(&b, p).map(|v| v as f32));
        for c in 0..3 {
            assert!((want[c] - got[c]).abs() < 0.1, "patch {} {}: wanted {want:?}, drawn {got:?}", p.number, p.name);
        }
    }
}

#[test]
fn colorise_changes_neither_the_counts_nor_which_edge_rows_light() {
    let mut b = Bench::new("coloriseedge");
    b.waveform.set_channels(channels(true, true, true, true));
    let frame = picture(1920, 1080, |x, _| if x == 1000 { [1.3, -0.1, 0.4] } else { [0.7, 0.5, 0.3] });
    let (width, height) = (800, 500);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let measured = |b: &mut Bench| {
        let luma = b.play(1920, 1080, &frame);
        [Channel::R, Channel::G, Channel::B].map(|c| b.waveform.channel_counts(c)).into_iter().chain([luma]).collect::<Vec<_>>()
    };
    let edge_lit = |image: &[u8]| {
        let [above, below] = [layout.above(), layout.below()];
        [1000u32, 1400].map(|column| {
            let x = layout.x_of_column(column, 1920) as usize;
            [above, below].map(|r| {
                let y = (r[1] + r[3] / 2.0) as usize;
                let at = (y * width as usize + x) * 4;
                image[at].max(image[at + 1]).max(image[at + 2]) > 100
            })
        })
    };
    let off_counts = measured(&mut b);
    let off_edges = edge_lit(&draw(&b, width, height));
    b.waveform.set_colorise(true);
    let on_counts = measured(&mut b);
    let on_edges = edge_lit(&draw(&b, width, height));
    assert_eq!(off_counts, on_counts, "counts are the same either way");
    assert_eq!(off_edges, on_edges, "the same edge rows light either way");
    assert_eq!(off_edges, [[true, true], [false, false]], "the out-of-range column lights both edge rows");
}

#[test]
fn colorise_does_not_change_the_label() {
    use scopecreep::display::Display;
    let b = Bench::new("coloriselabel");
    let off = Display::label(&b.waveform);
    b.waveform.set_colorise(true);
    assert_eq!(Display::label(&b.waveform), off);
}

// ------------------------------------------------------------------ Scale units (#125)

#[test]
fn the_scale_unit_never_changes_what_is_measured() {
    use scopecreep::waveform_scale::Unit;
    let mut b = Bench::new("scaleunits");
    b.waveform.set_measurement(measuring(MeasuredSpace::Rec2100Pq, LumaWeights::FromPrimaries));
    let counts = b.play(256, 16, &picture(256, 16, |x, _| grey(x * 4)));
    assert_eq!(b.waveform.unit(), Unit::Codes);
    for unit in [Unit::Percent, Unit::Nits] {
        b.waveform.set_unit(unit);
        assert_eq!(b.waveform.unit(), unit);
        assert_eq!(b.waveform.counts(), counts, "{unit:?}: the counts, edge rows included, are the same");
    }
}

#[test]
fn the_scale_numbers_follow_the_unit() {
    use scopecreep::display::Display;
    use scopecreep::waveform_scale::Unit;
    let mut b = Bench::new("scaletexts");
    b.waveform.set_measurement(Measurement { levels: Levels::Video, ..measuring(MeasuredSpace::Rec2100Pq, LumaWeights::FromPrimaries) });
    b.play(64, 16, &picture(64, 16, |_, _| grey(500)));
    let numbers = |b: &Bench| b.waveform.scale_texts([0.0, 0.0, 400.0, 800.0], 1.0).into_iter().map(|t| t.text).collect::<Vec<_>>();
    assert_eq!(numbers(&b), ["> 1023", "940", "512", "64", "< 0"]);
    b.waveform.set_unit(Unit::Percent);
    assert_eq!(numbers(&b), ["> 109.5", "100", "50", "0", "< -7.3"]);
    b.waveform.set_unit(Unit::Nits);
    assert_eq!(numbers(&b), ["> 10000", "10000", "1000", "100", "10", "0", "< 0"]);
    assert!(Display::label(&b.waveform).ends_with("scale in nits (PQ)"), "{}", Display::label(&b.waveform));
    // An output without light: nits are not offered, and what was chosen falls back to codes.
    b.waveform.set_measurement(Measurement { levels: Levels::Video, ..measuring(MeasuredSpace::Rec709, LumaWeights::FromPrimaries) });
    assert_eq!(numbers(&b), ["> 1023", "940", "512", "64", "< 0"]);
}

#[test]
fn the_popover_offers_nits_only_for_pq_and_hlg() {
    use scopecreep::display::Display;
    let b = Bench::new("scaleoffer");
    let choices = |b: &Bench| {
        let defs = b.waveform.offered_defs();
        defs.iter().find(|d| d.key == "scale_units").unwrap().choices.iter().map(|c| c.value).collect::<Vec<_>>()
    };
    assert_eq!(choices(&b), ["codes", "percent"]);
    for space in [MeasuredSpace::Rec2100Pq, MeasuredSpace::Rec2100Hlg] {
        b.waveform.set_measurement(measuring(space, LumaWeights::FromPrimaries));
        assert_eq!(choices(&b), ["codes", "percent", "nits"], "{space:?}");
    }
    assert_eq!(b.waveform.setting_defs().iter().find(|d| d.key == "scale_units").unwrap().choices[0].value, "codes", "10-bit codes are the default");
}

// ------------------------------------------------------------------ Colorise on R, G and B, any combination (#144)

const CHIP_COMBINATIONS: [[bool; 4]; 15] = [
    [true, false, false, false], [false, true, false, false], [false, false, true, false], [false, false, false, true],
    [true, true, false, false], [true, false, true, false], [true, false, false, true], [false, true, true, false],
    [false, true, false, true], [false, false, true, true], [true, true, true, false], [true, true, false, true],
    [true, false, true, true], [false, true, true, true], [true, true, true, true],
];

/// Red, green and blue flat at three well separated codes, so each trace has its own row.
const SEPARATE: [u32; 3] = [200, 500, 800];

fn separate_traces(name: &str, chips: [bool; 4], colorise: bool) -> (Vec<u8>, PlotLayout, u32, u32) {
    let mut b = Bench::new(name);
    b.waveform.set_channels(channels(chips[0], chips[1], chips[2], chips[3]));
    b.waveform.set_colorise(colorise);
    b.play(1920, 1080, &flat_codes(1920, 1080, SEPARATE));
    let (width, height) = (800, 500);
    (draw(&b, width, height), PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0), width, height)
}

#[test]
fn with_colorise_off_every_chip_combination_draws_neutral_traces_each_at_its_own_code() {
    for (i, chips) in CHIP_COMBINATIONS.into_iter().enumerate() {
        let (image, layout, width, _) = separate_traces(&format!("neutral{i}"), chips, false);
        for (chip, code) in [(1, SEPARATE[0]), (2, SEPARATE[1]), (3, SEPARATE[2])] {
            let drawn = colour_at(&image, width, &layout, 960, 1920, code);
            if chips[chip] {
                assert!(drawn[0] > 100 && spread_of(drawn) <= 3, "{chips:?}: channel {chip} neutral at its code: {drawn:?}");
            } else {
                assert!(drawn.iter().all(|&c| c < 40), "{chips:?}: channel {chip} off draws nothing: {drawn:?}");
            }
        }
    }
}

#[test]
fn with_colorise_on_every_chip_combination_draws_each_channel_in_its_colour_at_its_own_code() {
    for (i, chips) in CHIP_COMBINATIONS.into_iter().enumerate() {
        let (image, layout, width, _) = separate_traces(&format!("coloured{i}"), chips, true);
        for (chip, code) in [(1, SEPARATE[0]), (2, SEPARATE[1]), (3, SEPARATE[2])] {
            let drawn = colour_at(&image, width, &layout, 960, 1920, code);
            if chips[chip] {
                let others = (1..=3).filter(|&o| o != chip).map(|o| drawn[o - 1]).max().unwrap();
                assert!(drawn[chip - 1] > 100 && drawn[chip - 1] > others + 40, "{chips:?}: channel {chip} in its colour: {drawn:?}");
            } else {
                assert!(drawn.iter().all(|&c| c < 40), "{chips:?}: channel {chip} off draws nothing: {drawn:?}");
            }
        }
    }
}

#[test]
fn colorise_on_tints_y_and_the_channels_together_and_off_makes_them_all_neutral() {
    // A flat colour (0.2, 0.8, 0.3): Y lands at its Luma code, R, G and B at their own, all apart.
    let frame = picture(1920, 1080, |_, _| [0.2, 0.8, 0.3]);
    let (width, height) = (800, 500);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let mut b = Bench::new("colorisetogether");
    b.waveform.set_channels(channels(true, true, true, true));
    let counts = b.play(1920, 1080, &frame);
    let y_code = (0..1024).find(|&c| counts.at(0, c) == 1080).expect("one code");
    let codes = [y_code, (0.2f32 * 1023.0).round() as u32, (0.8f32 * 1023.0).round() as u32, (0.3f32 * 1023.0).round() as u32];
    let rows: Vec<[i32; 3]> = codes.iter().map(|&c| colour_at(&draw(&b, width, height), width, &layout, 960, 1920, c)).collect();
    assert!(rows.iter().all(|r| r[0] > 100 && spread_of(*r) <= 3), "off: all four neutral {rows:?}");
    b.waveform.set_colorise(true);
    b.play(1920, 1080, &frame);
    let image = draw(&b, width, height);
    let r = colour_at(&image, width, &layout, 960, 1920, codes[1]);
    let g = colour_at(&image, width, &layout, 960, 1920, codes[2]);
    let y = colour_at(&image, width, &layout, 960, 1920, codes[0]);
    assert!(r[0] > r[2] + 40 && g[1] > g[0] + 40, "on: R red, G green: {r:?} {g:?}");
    let [yr, yg, yb] = proportions(y.map(|v| v as f32));
    assert!(yg > yr + 0.5 && yg > yb + 0.3, "on: Y takes the image's colour: {y:?}");
}

#[test]
fn edge_rows_show_each_channels_out_of_range_in_its_colour_with_colorise_on_and_neutral_off() {
    let (width, height) = (800, 500);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    for colorise in [false, true] {
        let mut b = Bench::new(if colorise { "edgecoloured" } else { "edgeneutral" });
        b.waveform.set_channels(channels(false, true, true, true));
        b.waveform.set_colorise(colorise);
        // Green over white and blue under black in the middle column.
        b.play(1920, 1080, &picture(1920, 1080, |x, _| if x == 1000 { [0.5, 1.3, -0.2] } else { [0.5, 0.5, 0.5] }));
        let image = draw(&b, width, height);
        let (above, below) = (layout.above(), layout.below());
        let x = layout.x_of_column(1000, 1920) as usize;
        let pixel = |r: [f32; 4]| {
            let at = ((r[1] + r[3] / 2.0) as usize * width as usize + x) * 4;
            [image[at] as i32, image[at + 1] as i32, image[at + 2] as i32]
        };
        let (top, bottom) = (pixel(above), pixel(below));
        if colorise {
            assert!(top[1] > top[0] + 40 && top[1] > top[2] + 40, "green over white lights the top row green: {top:?}");
            assert!(bottom[2] > bottom[0] + 40 && bottom[2] > bottom[1] + 40, "blue under black lights the bottom row blue: {bottom:?}");
        } else {
            assert!(top[1] > 100 && spread_of(top) <= 3, "off: top row lit neutral: {top:?}");
            assert!(bottom[2] > 100 && spread_of(bottom) <= 3, "off: bottom row lit neutral: {bottom:?}");
        }
    }
}

#[test]
fn three_neutral_traces_on_a_colorchecker_frame_stay_readable_by_position() {
    use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
    let mut b = Bench::new("neutraloverlap");
    b.waveform.set_channels(channels(true, true, true, true));
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    let frame = chart.frame(600, 400);
    let (width, height) = (1200, 700);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    b.play(600, 400, &frame.pixels);
    let image = draw(&b, width, height);
    // Patches 13 to 16 (blue, green, red, yellow): every trace is a lit neutral line at its code, and where channels
    // are more than 30 codes apart there is dark plot between them, so they read as separate lines by position.
    for number in 13..=16 {
        let p = &chart.patches()[number - 1];
        let r = frame.patch_rect(number);
        let column = (r.x0 + r.x1) / 2;
        let mut codes = p.encoded.map(|v| (v * 1023.0).round() as i32);
        codes.sort();
        for pair in codes.windows(2) {
            if pair[1] - pair[0] <= 30 {
                continue;   // channels this close share one band: what neutral traces cannot tell apart (finding)
            }
            let between = colour_at(&image, width, &layout, column, 600, ((pair[0] + pair[1]) / 2) as u32);
            assert!(between.iter().all(|&c| c < 40), "patch {number}: dark between the traces: {between:?}");
        }
        for code in codes {
            let lit = colour_at(&image, width, &layout, column, 600, code as u32);
            assert!(lit[0] > 100 && spread_of(lit) <= 3, "patch {number}: a neutral line at code {code}: {lit:?}");
        }
    }
}

#[test]
fn graticule_brightness_scales_the_grid_and_legal_lines_and_the_default_is_today() {
    let mut b = Bench::new("graticule-gain");
    b.waveform.set_measurement(levels(Levels::Video, Some(Provenance::Project)));
    let codes: Vec<u32> = (0..1024).collect();
    let drawn = |b: &mut Bench, graticule: f32| {
        b.waveform.set_look(Look { graticule, ..Look::default() });
        graticule_at(b, &codes)
    };
    let sum = |v: &[u8]| v.iter().map(|&x| x as u32).sum::<u32>();
    let normal = drawn(&mut b, 1.0);
    b.waveform.set_look(Look::default());
    assert_eq!(normal, graticule_at(&mut b, &codes), "the default look draws as it always did");
    let (dim, bright) = (drawn(&mut b, 0.2), drawn(&mut b, 3.0));
    assert!(sum(&dim) < sum(&normal) && sum(&normal) < sum(&bright), "{} < {} < {}", sum(&dim), sum(&normal), sum(&bright));
    // The legal line at 64 follows it, and is still above the plot at the lowest setting.
    let plot = normal[600];
    assert!(dim[64] > plot && dim[64] < normal[64] && normal[64] < bright[64], "{} {} {}", dim[64], normal[64], bright[64]);
    assert_eq!((dim[600], bright[600]), (plot, plot), "the plot itself and the trace are not the graticule");
    // From the settings: the slider's range, its default and the gain.
    use scopecreep::scope_shader::{graticule_gain, GRATICULE_AMOUNT};
    assert_eq!((GRATICULE_AMOUNT.min, GRATICULE_AMOUNT.default, GRATICULE_AMOUNT.max), (20.0, 100.0, 300.0));
    assert_eq!((graticule_gain(None), graticule_gain(Some("250")), graticule_gain(Some("1")), graticule_gain(Some("junk"))), (1.0, 2.5, 0.2, 1.0));
    let settings: scopecreep::settings::Settings = [("graticule".to_string(), "50".to_string())].into();
    assert_eq!(scopecreep::waveform::look_from(&settings).graticule, 0.5);
}
