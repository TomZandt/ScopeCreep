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
use scopecreep::stops_false_colour::{StopsFalseColour, StopsMap};
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
const BOUNDARIES: [f64; 14] = [-6.0, -5.0, -4.0, -3.0, -2.0, -1.0, -0.5, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

/// A ramp of columns: for each zone its middle, and for each boundary a column just below and just above it.
/// Returns the frame and the stops of each column.
fn ramp() -> (Vec<f32>, Vec<f64>, u32) {
    let mut stops = vec![BOUNDARIES[0] - 1.0];
    for w in BOUNDARIES.windows(2) {
        stops.push(w[0] - 0.02);
        stops.push(w[0] + 0.02);
        stops.push((w[0] + w[1]) / 2.0);
    }
    stops.push(BOUNDARIES[13] - 0.02);
    stops.push(BOUNDARIES[13] + 0.02);
    stops.push(BOUNDARIES[13] + 1.0);
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
    let mut by_zone: Vec<Option<[u8; 3]>> = vec![None; 15];
    for s in &stops {
        let c = colour(*s);
        match &by_zone[zone_of(*s)] {
            None => by_zone[zone_of(*s)] = Some(c),
            Some(first) => assert_eq!(*first, c, "{s:+.2} stops is in zone {}, which has one colour", zone_of(*s)),
        }
    }
    let zones: Vec<[u8; 3]> = by_zone.into_iter().map(|c| c.expect("every zone is on the ramp")).collect();
    for i in 0..15 {
        for j in i + 1..15 {
            assert_ne!(zones[i], zones[j], "zones {i} and {j} share a colour");
        }
    }
    assert!(is_grey(zones[7]), "the zone of middle grey is grey: {:?}", zones[7]);
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

#[test]
fn a_timeline_colour_space_it_cannot_decode_is_labelled_and_never_guessed() {
    let mut b = Bench::new("unknown", "timeline=ARRI LogC4\noutput=Rec.709 Gamma 2.4\n");
    let frame = ColorChecker::new(TimelineSpace::AcesCg).frame(60, 40);
    b.play(60, 40, &frame.pixels);
    assert!(b.display.label().contains("not measured"), "{}", b.display.label());
    let image = b.render(60, 40);
    assert!(image.chunks(4).all(|p| p[..3] == [0, 0, 0]), "nothing is drawn for a space it cannot decode");
    drop(b);
    let mut none = Bench::new("nosetup", "");
    none.play(60, 40, &frame.pixels);
    assert!(none.display.label().contains("not measured"), "{}", none.display.label());
}

#[test]
fn the_legend_can_be_turned_off() {
    let (pixels, width) = (vec![0.4f32; 60 * 40 * 4], 600u32);
    let mut b = Bench::new("legend", SETUPS[2].1);
    b.play(60, 40, &pixels);
    let strip_colours = |image: &[u8]| {
        let mut seen = std::collections::BTreeSet::new();
        for y in 0..400 {
            for x in 0..44 {
                seen.insert(pixel(image, width, x, y));
            }
        }
        seen
    };
    let on = strip_colours(&b.render(width, 400));
    assert!(on.len() >= 15, "the legend shows every zone beside the picture: {} colours", on.len());
    assert!(!b.set.scale_texts(0, [0.0, 0.0, width as f32, 400.0], 1.0).is_empty(), "the legend has numbers");
    b.choose("legend", "off");
    let off = strip_colours(&b.render(width, 400));
    assert!(off.len() <= 2, "no legend: only picture and its edge: {} colours", off.len());
    assert!(b.set.scale_texts(0, [0.0, 0.0, width as f32, 400.0], 1.0).is_empty());
}

#[test]
fn the_settings_are_a_mode_and_the_legend_and_the_mode_defaults_to_full_zones() {
    let b = Bench::new("settings", SETUPS[2].1);
    let defs = b.set.setting_defs(0);
    assert_eq!(defs.iter().map(|d| d.key).collect::<Vec<_>>(), ["mode", "legend"]);
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
