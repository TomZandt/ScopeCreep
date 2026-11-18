//! Stops False Colour (ticket #58), proven at the Feed seam: a ColorChecker chart or a test ramp goes in through
//! the synthetic writer with a Colour Setup (the Sender's manual override travels in the Feed), the app's Feed
//! monitor hands it to the GPU as the app does, and the tests read back what the Panel measured, what it drew and
//! its label. Expected stops come from the chart's published values (the accuracy crate, checked against the
//! research table), never from the shader. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::{Display, DisplaySet};
use scopecreep::output_conversion::OutputConversion;
use scopecreep::settings::PanelSettings;
use scopecreep::stops_false_colour::{StopsFalseColour, StopsMap, ZONE_COLOURS, ZONE_NAMES, zone_of};
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_accuracy::transfer;
use std::path::PathBuf;
use std::sync::Arc;


/// A stand-in Resolve and Sender, the app's Feed monitor and its GPU side, with only Stops False Colour shown.
struct Bench {
    rig: Rig,
    conversion: OutputConversion,
    set: DisplaySet,
    display: Arc<StopsFalseColour>,
    settings: PanelSettings,
}

/// As the app does: the picture after Output Conversion to every display, and the Timeline Colour Space frame
/// to the displays that measure scene light.
fn poll(rig: &mut Rig, conversion: &mut OutputConversion, set: &DisplaySet, settings: &PanelSettings) {
    rig.poll(|encoder, gpu_frame| {
        let shown = conversion.convert(encoder, gpu_frame);
        set.on_frame(encoder, shown);
        set.on_timeline_frame(encoder, gpu_frame);
    });
    set.configure(&rig.monitor.status().display_context(), settings);
}

impl Bench {
    fn new(name: &str, colour_setup: &str) -> Bench {
        let mut rig = Rig::with_colour_setup(name, (!colour_setup.is_empty()).then_some(colour_setup));
        let mut set = DisplaySet::default();
        let display = Arc::new(StopsFalseColour::new(&rig.device, &rig.queue, TARGET));
        set.register(display.clone());
        let (mut conversion, settings) = (OutputConversion::new(&rig.device, &rig.queue), PanelSettings::default());
        poll(&mut rig, &mut conversion, &set, &settings);
        Bench { rig, conversion, set, display, settings }
    }

    fn choose(&mut self, key: &str, value: &str) {
        self.settings.set("Stops False Colour", key, value);
        self.configure();
    }

    fn configure(&self) {
        self.set.configure(&self.rig.monitor.status().display_context(), &self.settings);
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let (conversion, set, settings) = (&mut self.conversion, &self.set, &self.settings);
        self.rig.play(width, height, pixels, |rig| poll(rig, conversion, set, settings));
    }

    fn measured(&self) -> StopsMap {
        *self.set.read_back(0).and_then(|r| r.downcast::<StopsMap>().ok()).expect("the display reads back what it measured")
    }

    /// Draw the display into a `width` × `height` target at scale 1 (RGBA8, rows top first).
    fn render(&self, width: u32, height: u32) -> Vec<u8> {
        self.rig.render(width, height, |pass| self.set.draw(pass, 0, [0.0, 0.0, width as f32, height as f32], [width as f32, height as f32], 1.0))
    }
}

const SETUPS: [(TimelineSpace, &str); 4] = [
    (TimelineSpace::AcesCct, "timeline=ACEScct\noutput=Rec.709 BT.1886\naces_version=2.0\n"),
    (TimelineSpace::AcesCg, "timeline=ACEScg\noutput=Rec.709 BT.1886\naces_version=2.0\n"),
    (TimelineSpace::DavinciIntermediate, "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n"),
    (TimelineSpace::Rec709Gamma24, "timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\n"),
];

const CHART: (u32, u32) = (600, 400);

fn pixel(image: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * width as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

fn centre(rect: scopecreep_accuracy::colorchecker::Rect) -> (u32, u32) {
    ((rect.x0 + rect.x1) / 2, (rect.y0 + rect.y1) / 2)
}

fn is_grey(c: [u8; 3]) -> bool {
    c[0] == c[1] && c[1] == c[2]
}

fn chart_bench(name: &str, space: TimelineSpace, setup: &str) -> (Bench, ColorChecker, scopecreep_accuracy::colorchecker::Frame) {
    let chart = ColorChecker::new(space);
    let frame = chart.frame(CHART.0, CHART.1);
    let mut b = Bench::new(name, setup);
    b.play(CHART.0, CHART.1, &frame.pixels);
    (b, chart, frame)
}

#[test]
fn every_patch_measures_its_published_stops_in_every_supported_timeline_colour_space() {
    for (i, (space, setup)) in SETUPS.iter().enumerate() {
        let (b, chart, frame) = chart_bench(&format!("stops{i}"), *space, setup);
        let map = b.measured();
        for patch in chart.patches() {
            let (x, y) = centre(frame.patch_rect(patch.number));
            let got = map.at(x, y) as f64;
            assert!((got - patch.stops).abs() < 0.01, "{space:?} patch {} {}: measured {got:+.3} stops, published {:+.3}", patch.number, patch.name, patch.stops);
        }
        let at = |n: usize| map.at(centre(frame.patch_rect(n)).0, centre(frame.patch_rect(n)).1);
        assert_eq!(format!("{:+.2}", at(22)), "+0.08", "{space:?}: neutral 5");
        assert_eq!(format!("{:+.2}", at(21)), "+1.02", "{space:?}: neutral 6.5");
    }
}

#[test]
fn grey_and_plus_one_stop_mode_paints_neutral_5_green_neutral_6_5_pink_and_the_other_greys_grey() {
    for (i, (space, setup)) in SETUPS.iter().enumerate() {
        let (mut b, _, frame) = chart_bench(&format!("chart{i}"), *space, setup);
        b.choose("mode", "grey-plus-one");
        b.choose("legend", "off");
        let image = b.render(CHART.0, CHART.1);
        let at = |n: usize| {
            let (x, y) = centre(frame.patch_rect(n));
            pixel(&image, CHART.0, x, y)
        };
        let green = at(22);
        let pink = at(21);
        assert!(green[1] > 150 && green[0] < 100 && green[2] < 120, "{space:?}: neutral 5 is green, got {green:?}");
        assert!(pink[0] > 200 && pink[2] > 120 && pink[1] < pink[0], "{space:?}: neutral 6.5 is pink, got {pink:?}");
        for n in [19, 20, 23, 24] {
            assert!(is_grey(at(n)), "{space:?}: patch {n} is grey, got {:?}", at(n));
        }
        assert_ne!(at(19), at(24), "the picture is a black and white picture, not one flat grey");
    }
}

/// Every zone boundary, in stops from 18 % grey.
const BOUNDARIES: [f64; 16] = [-6.5, -5.5, -4.5, -3.5, -2.5, -1.5, -0.75, -0.25, 0.25, 0.75, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5];

fn zone_of_stops(stops: f64) -> usize {
    zone_of(stops as f32)
}

/// A ramp of columns: for each zone its middle, and for each boundary a column just below and just above it.
/// Returns the frame and the stops of each column.
fn ramp() -> (Vec<f32>, Vec<f64>, u32) {
    let mut stops = vec![BOUNDARIES[0] - 1.0];
    for w in BOUNDARIES.windows(2) {
        stops.push(w[0] - 0.02);
        stops.push(w[0] + 0.02);
        stops.push((w[0] + w[1]) / 2.0);
    }
    stops.push(BOUNDARIES[15] - 0.02);
    stops.push(BOUNDARIES[15] + 0.02);
    stops.push(BOUNDARIES[15] + 1.0);
    let cell = 10u32;
    let (width, height) = (stops.len() as u32 * cell, 20u32);
    let mut pixels = Vec::new();
    for _ in 0..height {
        for x in 0..width {
            let code = transfer::di_encode(0.18 * 2f64.powf(stops[(x / cell) as usize])) as f32;
            pixels.extend([code, code, code, 1.0]);
        }
    }
    (pixels, stops, width)
}

#[test]
fn full_zones_put_every_stop_in_its_own_colour_and_change_colour_at_every_boundary() {
    let (pixels, stops, width) = ramp();
    let mut b = Bench::new("ramp", SETUPS[2].1);
    b.choose("legend", "off");
    b.play(width, 20, &pixels);
    let image = b.render(width, 20);
    let colour = |s: f64| {
        let column = stops.iter().position(|v| *v == s).unwrap() as u32;
        pixel(&image, width, column * 10 + 5, 10)
    };
    let zone_of = |s: f64| BOUNDARIES.iter().filter(|e| **e <= s).count();
    let mut by_zone: Vec<Option<[u8; 3]>> = vec![None; 17];
    for s in &stops {
        let c = colour(*s);
        match &by_zone[zone_of(*s)] {
            None => by_zone[zone_of(*s)] = Some(c),
            Some(first) => assert_eq!(*first, c, "{s:+.2} stops is in zone {}, which has one colour", zone_of(*s)),
        }
    }
    let zones: Vec<[u8; 3]> = by_zone.into_iter().map(|c| c.expect("every zone is on the ramp")).collect();
    for i in 0..17 {
        for j in i + 1..17 {
            assert_ne!(zones[i], zones[j], "zones {i} and {j} share a colour");
        }
    }
    assert!(is_grey(zones[8]), "the zone of middle grey is grey: {:?}", zones[8]);
}

#[test]
fn the_measured_ramp_reads_the_stops_it_was_made_with() {
    let (pixels, stops, width) = ramp();
    let mut b = Bench::new("rampmeasure", SETUPS[2].1);
    b.play(width, 20, &pixels);
    let map = b.measured();
    for (i, s) in stops.iter().enumerate() {
        let got = map.at(i as u32 * 10 + 5, 10) as f64;
        assert!((got - s).abs() < 0.005, "column {i}: made at {s:+.2} stops, measured {got:+.3}");
    }
}

#[test]
fn the_label_says_approximated_on_a_display_referred_timeline_and_not_otherwise() {
    let label = {
        let (b, _, _) = chart_bench("labelrec709", TimelineSpace::Rec709Gamma24, SETUPS[3].1);
        b.display.label()
    };
    assert!(label.contains("approximated"), "{label}");
    assert!(!label.contains("scene light decoded"), "it does not claim scene light: {label}");
    for i in 0..3 {
        let label = {
            let (b, _, _) = chart_bench(&format!("labelscene{i}"), SETUPS[i].0, SETUPS[i].1);
            b.display.label()
        };
        assert!(!label.contains("approximated"), "{label}");
        assert!(label.contains("scene light"), "{label}");
        assert!(label.contains(SETUPS[i].0.name()) || label.contains("DaVinci"), "names the space it decoded: {label}");
    }
}

const FRAME_BAR: u32 = 600;

/// What the legend's bar shows at the bottom of a Panel `height` tall, one colour per swatch, read at the middle
/// of the swatch row (a swatch is 16 logical pixels with 3 and a line of text under it).
fn bar(image: &[u8], width: u32, height: u32, swatches: u32) -> Vec<[u8; 3]> {
    (0..swatches).map(|i| pixel(image, width, (i * 2 + 1) * width / (2 * swatches), height - 25)).collect()
}

fn rgb8(c: [f32; 3]) -> [u8; 3] {
    [(c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8]
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(&b).all(|(x, y)| x.abs_diff(*y) <= 1)
}

#[test]
fn the_colorchecker_patches_land_in_the_zones_the_edges_say_in_every_supported_timeline_colour_space() {
    for (i, (space, setup)) in SETUPS.iter().enumerate() {
        let (mut b, chart, frame) = chart_bench(&format!("zones{i}"), *space, setup);
        b.choose("legend", "off");
        let image = b.render(CHART.0, CHART.1);
        for patch in chart.patches() {
            let (x, y) = centre(frame.patch_rect(patch.number));
            let zone = zone_of_stops(patch.stops);
            assert!(near(pixel(&image, CHART.0, x, y), rgb8(ZONE_COLOURS[zone])), "{space:?} patch {} {} ({:+.2} stops) is in zone {zone}, got {:?}", patch.number, patch.name, patch.stops, pixel(&image, CHART.0, x, y));
        }
        let at = |n: usize| {
            let (x, y) = centre(frame.patch_rect(n));
            pixel(&image, CHART.0, x, y)
        };
        assert!(near(at(22), rgb8(ZONE_COLOURS[8])) && is_grey(at(22)), "{space:?}: neutral 5 (+0.08) is on the 18 % grey zone, a plain grey");
        assert!(near(at(21), rgb8(ZONE_COLOURS[10])), "{space:?}: neutral 6.5 (+1.02) is in the +1 zone");
    }
}

#[test]
fn black_below_six_and_a_half_stops_under_and_white_above_six_and_a_half_over() {
    let (pixels, stops, width) = ramp();
    let mut b = Bench::new("blackwhite", SETUPS[2].1);
    b.choose("legend", "off");
    b.play(width, 20, &pixels);
    let image = b.render(width, 20);
    let colour = |s: f64| pixel(&image, width, stops.iter().position(|v| *v == s).unwrap() as u32 * 10 + 5, 10);
    assert_eq!(colour(BOUNDARIES[0] - 1.0), [0, 0, 0]);
    assert_eq!(colour(BOUNDARIES[0] - 0.02), [0, 0, 0]);
    assert_eq!(colour(BOUNDARIES[BOUNDARIES.len() - 1] + 1.0), [255, 255, 255]);
    assert_eq!(colour(BOUNDARIES[BOUNDARIES.len() - 1] + 0.02), [255, 255, 255]);
    assert_ne!(colour(BOUNDARIES[0] + 0.02), [0, 0, 0], "just inside the scale is purple, not black");
}

/// Draw a flat picture into a Panel `width` x `height` with the legend on.
fn legend_bench(name: &str, setup: &str, width: u32, height: u32) -> (Bench, Vec<u8>) {
    let mut b = Bench::new(name, setup);
    b.play(60, 40, &vec![0.4f32; 60 * 40 * 4]);
    let image = b.render(width, height);
    (b, image)
}

#[test]
fn the_legend_is_a_horizontal_bar_of_the_colours_the_picture_uses() {
    let (pixels, stops, width) = ramp();
    let mut b = Bench::new("legendcolours", SETUPS[2].1);
    b.choose("legend", "off");
    b.play(width, 20, &pixels);
    let image = b.render(width, 20);
    let used: Vec<[u8; 3]> = (0..17).map(|z| {
        let column = stops.iter().position(|v| zone_of_stops(*v) == z).unwrap() as u32;
        pixel(&image, width, column * 10 + 5, 10)
    }).collect();
    b.choose("legend", "on");
    let with = b.render(FRAME_BAR, 400);
    let swatches = bar(&with, FRAME_BAR, 400, 17);
    for z in 0..17 {
        assert!(near(swatches[z], used[z]), "swatch {z} is the colour the picture uses for that zone: {:?} against {:?}", swatches[z], used[z]);
    }
    assert!(is_grey(swatches[8]), "middle grey is the one neutral");
    assert_eq!(swatches[0], [0, 0, 0]);
    assert_eq!(swatches[16], [255, 255, 255]);
    // Full width and in order left to right, dark to bright: the bar's first and last pixels are black and white.
    assert_eq!(pixel(&with, FRAME_BAR, 0, 400 - 25), [0, 0, 0]);
    assert_eq!(pixel(&with, FRAME_BAR, FRAME_BAR - 1, 400 - 25), [255, 255, 255]);
}

#[test]
fn the_numbers_sit_under_their_own_swatches_in_order_with_the_captions_over_the_scale() {
    let (b, _) = legend_bench("legendtext", SETUPS[2].1, FRAME_BAR, 400);
    let texts = b.set.scale_texts(0, [0.0, 0.0, FRAME_BAR as f32, 400.0], 1.0);
    let numbers: Vec<_> = texts.iter().filter(|t| ZONE_NAMES.contains(&t.text.as_str())).collect();
    assert_eq!(numbers.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["-6", "-5", "-4", "-3", "-2", "-1", "-1/2", "18 %", "+1/2", "+1", "+2", "+3", "+4", "+5", "+6"]);
    let cell = FRAME_BAR as f32 / 17.0;
    for (n, t) in numbers.iter().enumerate() {
        assert!(t.centred, "{} is centred", t.text);
        assert!((t.left + t.width / 2.0 - (n as f32 + 1.5) * cell).abs() < 0.01, "{} is under swatch {}", t.text, n + 1);
        assert!(t.top > 400.0 - 20.0, "numbers are under the bar: {}", t.top);
    }
    let caption = |name: &str| texts.iter().find(|t| t.text == name).unwrap_or_else(|| panic!("caption {name} missing"));
    let over = numbers[0].top;
    for name in ["Underexposed", "Dark skin", "Light skin", "Overexposed"] {
        assert!(caption(name).top < over - 30.0, "{name} is over the bar");
    }
    assert!(caption("Underexposed").left < cell * 2.0, "Underexposed is at the dark end");
    assert!(caption("Overexposed").left + caption("Overexposed").width > FRAME_BAR as f32 - cell * 2.0, "Overexposed is at the bright end");
    assert!(caption("Underexposed").left + caption("Underexposed").width < caption("Dark skin").left, "captions do not overlap");
    assert!(caption("Dark skin").left + caption("Dark skin").width < caption("Light skin").left);
    assert!(caption("Light skin").left + caption("Light skin").width < caption("Overexposed").left);
}

#[test]
fn the_skin_captions_sit_over_the_zones_the_colorcheckers_skin_patches_fall_in() {
    let (b, _) = legend_bench("skin", SETUPS[2].1, FRAME_BAR, 400);
    let texts = b.set.scale_texts(0, [0.0, 0.0, FRAME_BAR as f32, 400.0], 1.0);
    let cell = FRAME_BAR as f32 / 17.0;
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    for (name, patch) in [("Dark skin", 1), ("Light skin", 2)] {
        let stops = chart.patches().iter().find(|p| p.number == patch).unwrap().stops;
        let zone = zone_of_stops(stops);
        let t = texts.iter().find(|t| t.text == name).unwrap();
        let middle = t.left + t.width / 2.0;
        assert!(middle >= zone as f32 * cell && middle < (zone + 1) as f32 * cell, "{name} ({stops:+.2} stops, zone {zone}) is centred over its swatch, at {middle}");
    }
}

#[test]
fn numbers_and_captions_are_dropped_never_overlapped_when_the_panel_is_narrow() {
    let numbers = |width: f32| {
        let (b, _) = legend_bench(&format!("narrow{}", width as u32), SETUPS[2].1, FRAME_BAR, 400);
        let texts = b.set.scale_texts(0, [0.0, 0.0, width, 400.0], 1.0);
        let nums: Vec<String> = texts.iter().filter(|t| ZONE_NAMES.contains(&t.text.as_str())).map(|t| t.text.clone()).collect();
        let captions = texts.iter().filter(|t| !ZONE_NAMES.contains(&t.text.as_str())).count();
        (nums, captions)
    };
    assert_eq!(numbers(900.0).0.len(), 15);
    let (every_second, _) = numbers(300.0);
    assert_eq!(every_second, ["-5", "-3", "-1", "18 %", "+1", "+3", "+5"], "every second number from middle grey");
    assert!(numbers(120.0).0.is_empty(), "none rather than overlapping");
    assert_eq!(numbers(900.0).1, 4);
    assert!(numbers(240.0).1 < 4, "captions are dropped before they overlap");
    assert!(numbers(120.0).1 < 4);
}

#[test]
fn swatches_are_always_drawn_even_when_the_panel_is_too_small_for_text() {
    let (b, image) = legend_bench("tiny", SETUPS[2].1, 120, 120);
    assert!(b.set.scale_texts(0, [0.0, 0.0, 120.0, 120.0], 1.0).is_empty());
    let colours: std::collections::BTreeSet<_> = (0..120).map(|x| pixel(&image, 120, x, 120 - 8)).collect();
    assert!(colours.len() >= 15, "the swatches are there: {} colours", colours.len());
}

#[test]
fn grey_and_plus_one_stop_has_a_horizontal_two_swatch_legend() {
    let mut b = Bench::new("greylegend", SETUPS[2].1);
    b.choose("mode", "grey-plus-one");
    b.play(60, 40, &vec![0.4f32; 60 * 40 * 4]);
    let image = b.render(FRAME_BAR, 400);
    let swatches = bar(&image, FRAME_BAR, 400, 2);
    assert!(swatches[0][1] > 150 && swatches[0][0] < 100, "green first: {:?}", swatches[0]);
    assert!(swatches[1][0] > 200 && swatches[1][2] > 120, "pink second: {:?}", swatches[1]);
    let texts = b.set.scale_texts(0, [0.0, 0.0, FRAME_BAR as f32, 400.0], 1.0);
    assert_eq!(texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["18 % grey", "+1"]);
    assert!(texts[0].left + texts[0].width / 2.0 < 300.0 && texts[1].left + texts[1].width / 2.0 > 300.0);
}

#[test]
fn a_timeline_colour_space_it_cannot_decode_measures_the_output_picture_and_says_approximated_and_assumed() {
    // The Feed is shown as sent (no conversion for this timeline), so it is read as Rec.709 Gamma 2.4, assumed.
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    let frame = chart.frame(CHART.0, CHART.1);
    for (name, setup) in [("unsupported", "timeline=ARRI LogC4\noutput=Rec.709 Gamma 2.4\n"), ("nosetup", "")] {
        let mut b = Bench::new(name, setup);
        b.play(CHART.0, CHART.1, &frame.pixels);
        let label = b.display.label();
        assert!(label.contains("approximated") && label.contains("assumed") && label.contains("display-referred"), "{label}");
        assert!(!label.contains("not measured"), "{label}");
        let map = b.measured();
        for patch in chart.patches() {
            let (x, y) = centre(frame.patch_rect(patch.number));
            assert!((map.at(x, y) as f64 - patch.stops).abs() < 0.02, "{name} patch {}: {:+.3} against {:+.3}", patch.number, map.at(x, y), patch.stops);
        }
        let mut b = b;
        b.choose("legend", "off");
        let image = b.render(CHART.0, CHART.1);
        let (x, y) = centre(frame.patch_rect(22));
        assert!(near(pixel(&image, CHART.0, x, y), rgb8(ZONE_COLOURS[8])), "zones are drawn, not black");
    }
}

#[test]
fn a_converted_picture_is_read_through_its_output_colour_spaces_curve() {
    // DaVinci YRGB converts nothing but tags the Output Colour Space, so the Output picture is in it.
    let mut b = Bench::new("p3", "science=yrgb\ntimeline=ARRI LogC4\noutput=P3-D65\n");
    let code = 0.18f32.powf(1.0 / 2.6);
    b.play(60, 40, &[code, code, code, 1.0].repeat(60 * 40));
    let label = b.display.label();
    assert!(label.contains("gamma 2.6") && label.contains("approximated") && !label.contains("assumed"), "{label}");
    assert!(b.measured().at(30, 20).abs() < 0.01, "18 % in gamma 2.6 is zero stops: {}", b.measured().at(30, 20));
}

#[test]
fn a_message_in_the_middle_replaces_black_when_there_is_nothing_to_measure_and_goes_when_there_is() {
    let mut b = Bench::new("message", "science=yrgb\ntimeline=ARRI LogC4\noutput=Rec.2100 HLG\n");
    let frame = ColorChecker::new(TimelineSpace::AcesCg).frame(60, 40);
    b.play(60, 40, &frame.pixels);
    assert!(b.display.label().contains("not measured"), "{}", b.display.label());
    let image = b.render(60, 40);
    assert!(image.chunks(4).all(|p| p[..3] == image[..3] && p[..3] != [0, 0, 0]), "a neutral background, not black, and no picture");
    let texts = b.set.scale_texts(0, [0.0, 0.0, 600.0, 400.0], 1.0);
    let all: String = texts.iter().map(|t| t.text.clone()).collect::<Vec<_>>().join(" | ");
    assert!(all.contains("ARRI LogC4") && all.contains("Rec.2100 HLG") && all.contains("ACEScct") && all.contains("Rec.709 Gamma 2.4"), "{all}");
    assert!(texts.iter().all(|t| t.centred && t.left == 0.0 && t.width == 600.0), "centred in the Panel");
    let (first, last) = (texts.first().unwrap().top, texts.last().unwrap().top);
    assert!(((first + last) / 2.0 - 200.0).abs() < 1.0, "in the middle");
    // The Colour Setup becomes one it can measure: the message is gone.
    b.rig.writer.set_colour_setup(Some(scopecreep_feed::SentColourSetup::new(scopecreep_feed::Provenance::Manual, SETUPS[2].1)));
    b.play(60, 40, &frame.pixels);
    let texts = b.set.scale_texts(0, [0.0, 0.0, 600.0, 400.0], 1.0);
    assert!(texts.iter().all(|t| !t.text.contains("HLG") && !t.text.contains("Cannot")), "no message once it can measure");
    assert!(!b.display.label().contains("not measured"));
}

#[test]
fn the_legend_can_be_turned_off() {
    let mut b = Bench::new("legend", SETUPS[2].1);
    b.play(60, 40, &vec![0.4f32; 60 * 40 * 4]);
    let strip = |image: &[u8]| (0..FRAME_BAR).map(|x| pixel(image, FRAME_BAR, x, 400 - 25)).collect::<std::collections::BTreeSet<_>>();
    assert!(strip(&b.render(FRAME_BAR, 400)).len() >= 15, "the legend shows every zone under the picture");
    assert!(!b.set.scale_texts(0, [0.0, 0.0, FRAME_BAR as f32, 400.0], 1.0).is_empty(), "the legend has numbers");
    b.choose("legend", "off");
    assert!(strip(&b.render(FRAME_BAR, 400)).len() <= 2, "no legend: only the picture");
    assert!(b.set.scale_texts(0, [0.0, 0.0, FRAME_BAR as f32, 400.0], 1.0).is_empty());
}

#[test]
fn the_settings_are_a_mode_and_the_legend_and_the_mode_defaults_to_full_zones() {
    let b = Bench::new("settings", SETUPS[2].1);
    let defs = b.set.setting_defs(0);
    assert_eq!(defs.iter().map(|d| d.key).collect::<Vec<_>>(), ["mode", "legend", "bw"]);
    assert_eq!(defs[0].choices[0].value, "zones", "full zones is the default");
    assert_eq!(defs[1].choices.iter().map(|c| c.value).collect::<Vec<_>>(), ["on", "off"]);
    assert!(b.display.label().contains("full zones"), "{}", b.display.label());
}

#[test]
fn the_name_of_the_licensed_scheme_appears_nowhere_in_the_app() {
    let banned = ["EL", " ", "ZONE"].concat().to_lowercase();
    fn walk(dir: &std::path::Path, banned: &str, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, banned, found);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                if text.to_lowercase().contains(banned) {
                    found.push(path.display().to_string());
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"), &banned, &mut found);
    assert!(found.is_empty(), "the licensed name is in {found:?}");
    let b = Bench::new("name", SETUPS[0].1);
    let mut text = b.display.label().to_lowercase();
    for def in b.set.setting_defs(0) {
        text += &format!(" {} {}", def.label, def.choices.iter().map(|c| c.text).collect::<Vec<_>>().join(" ")).to_lowercase();
    }
    assert!(!text.contains(&banned), "{text}");
}
