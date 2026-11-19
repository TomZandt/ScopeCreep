//! The gamut/legal warning (ticket #56), proven at the Feed seam: synthetic frames (one-colour frames and a
//! ColorChecker chart) go in through the synthetic writer with a Colour Setup, the app's Feed monitor and Output
//! Conversion hand them to the display exactly as the app does, and the tests read back the pixels the Panel drew
//! and its label. Expected tints come from the ticket's rules written out by hand (a linear fade between the
//! threshold and the limit), never from the shader. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::DisplaySet;
use scopecreep::gamut_warning::GamutWarning;
use scopecreep::feed_loop::FeedLoop;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::settings::PanelSettings;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use std::sync::{Arc, Mutex};


/// A stand-in Resolve and Sender, the app's Feed monitor and Output Conversion, and the gamut/legal warning alone
/// in its set.
struct Bench {
    rig: Rig,
    feed: FeedLoop,
    set: DisplaySet,
    settings: Arc<Mutex<PanelSettings>>,
}

/// Colour Setups that convert nothing, at either Levels.
const VIDEO: &str = "timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\n";
const DATA: &str = "timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\nlevels=full\n";
const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";

impl Bench {
    fn new(name: &str, colour_setup: &str) -> Bench {
        let mut rig = Rig::with_colour_setup(name, Some(colour_setup));
        let mut set = DisplaySet::default();
        set.register(Arc::new(GamutWarning::new(&rig.device, &rig.queue, TARGET)));
        let settings = Arc::new(Mutex::new(PanelSettings::default()));
        let mut feed = FeedLoop::new(set.clone(), settings.clone(), OutputConversion::new(&rig.device, &rig.queue));
        rig.step(&mut feed);
        Bench { rig, feed, set, settings }
    }

    fn choose(&mut self, key: &str, value: &str) {
        self.settings.lock().unwrap().set(self.set.name(0), key, value);
        self.rig.step(&mut self.feed);
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let feed = &mut self.feed;
        self.rig.play(width, height, pixels, |rig| {
            rig.step(feed);
        });
    }

    fn render(&self, width: u32, height: u32) -> Vec<u8> {
        self.rig.render(width, height, |pass| self.set.draw(pass, 0, [0.0, 0.0, width as f32, height as f32], [width as f32, height as f32], 1.0))
    }

    /// What one colour looks like in the Panel: a small one-colour frame, drawn and read back at its middle.
    fn shade(&mut self, rgb: [f32; 3]) -> [u8; 3] {
        let pixels: Vec<f32> = (0..8 * 8).flat_map(|_| [rgb[0], rgb[1], rgb[2], 1.0]).collect();
        self.play(8, 8, &pixels);
        let image = self.render(8, 8);
        let i = (4 * 8 + 4) * 4;
        [image[i], image[i + 1], image[i + 2]]
    }

    fn label(&self) -> String {
        self.set.labels()[0].clone()
    }
}

fn grey(v: f32) -> [f32; 3] {
    [v; 3]
}

fn close(got: [u8; 3], want: [f32; 3]) -> bool {
    got.iter().zip(want).all(|(g, w)| (*g as f32 - w * 255.0).abs() <= 1.5)
}

/// Grey at `v` tinted `t` of the way to `to`: the ticket's "fades towards".
fn tinted(v: f32, to: [f32; 3], t: f32) -> [f32; 3] {
    [v + (to[0] - v) * t, v + (to[1] - v) * t, v + (to[2] - v) * t]
}

const RED: [f32; 3] = [1.0, 0.0, 0.0];
const BLUE: [f32; 3] = [0.0, 0.0, 1.0];

fn assert_ramp_fades_to_the_limits(setup: &str) {
    let mut b = Bench::new("ramp", setup);
    // Between the limits and the thresholds: plain grey.
    assert!(close(b.shade(grey(0.5)), grey(0.5)));
    assert!(close(b.shade(grey(0.9)), grey(0.9)), "90 % is where red starts, with no tint yet");
    // From 90 % up to legal white: a tint that grows towards red.
    for v in [0.925_f32, 0.95, 0.975] {
        let t = (v - 0.9) / 0.1;
        let got = b.shade(grey(v));
        assert!(close(got, tinted(v, RED, t)), "at {v}: {got:?}");
    }
    // Above legal white: solid red.
    assert_eq!(b.shade(grey(1.02)), [255, 0, 0]);
    assert_eq!(b.shade(grey(1.5)), [255, 0, 0]);
    // From 10 % down to legal black: a tint that grows towards blue.
    for v in [0.075_f32, 0.05, 0.025] {
        let t = (0.1 - v) / 0.1;
        let got = b.shade(grey(v));
        assert!(close(got, tinted(v, BLUE, t)), "at {v}: {got:?}");
    }
    // Below legal black: solid blue.
    assert_eq!(b.shade(grey(-0.02)), [0, 0, 255]);
}

#[test]
fn on_a_ramp_the_tint_grows_towards_the_limit_and_is_solid_past_it_at_video_levels() {
    assert_ramp_fades_to_the_limits(VIDEO);
}

#[test]
fn on_a_ramp_the_tint_grows_towards_the_limit_and_is_solid_past_it_at_data_levels() {
    assert_ramp_fades_to_the_limits(DATA);
}

#[test]
fn a_single_channel_past_a_limit_is_solid_whatever_the_luma_is() {
    let mut b = Bench::new("channels", VIDEO);
    // Luma of this colour is about 0.5, nowhere near a threshold; the red channel is above legal white.
    assert_eq!(b.shade([1.3, 0.3, 0.3]), [255, 0, 0]);
    // Negative red with luma about 0.5.
    assert_eq!(b.shade([-0.1, 0.7, 0.7]), [0, 0, 255]);
}

#[test]
fn changing_the_thresholds_moves_where_the_ramp_starts() {
    let mut b = Bench::new("thresholds", VIDEO);
    assert!(close(b.shade(grey(0.85)), grey(0.85)), "red has not started at 85 % by default");
    b.choose("high", "80");
    let got = b.shade(grey(0.85));
    assert!(close(got, tinted(0.85, RED, 0.25)), "with red starting at 80 %: {got:?}");
    assert!(close(b.shade(grey(0.15)), grey(0.15)), "blue has not started at 15 % by default");
    b.choose("low", "20");
    let got = b.shade(grey(0.15));
    assert!(close(got, tinted(0.15, BLUE, 0.25)), "with blue starting at 20 %: {got:?}");
}

#[test]
fn with_r103_a_pixel_at_103_percent_is_not_solid_red_and_105_is_the_limit() {
    let mut b = Bench::new("r103", VIDEO);
    assert_eq!(b.shade(grey(1.03)), [255, 0, 0], "without tolerance it is past legal white");
    b.choose("r103", "on");
    let got = b.shade(grey(1.03));
    assert_ne!(got, [255, 0, 0], "with R103 a pixel at 103 % is tolerated");
    // The fade now runs from 90 % to 105 %; the grey itself is clamped at white.
    assert!(close(got, tinted(1.0, RED, (1.03 - 0.9) / 0.15)), "{got:?}");
    assert_eq!(b.shade(grey(1.06)), [255, 0, 0]);
    // The low side: -3 % is tolerated, -6 % is not; a channel is judged the same way.
    assert_ne!(b.shade(grey(-0.03)), [0, 0, 255]);
    assert_eq!(b.shade(grey(-0.06)), [0, 0, 255]);
    assert_ne!(b.shade([1.03, 0.5, 0.5]), [255, 0, 0], "a channel at 103 % is tolerated too");
    assert_eq!(b.shade([1.06, 0.5, 0.5]), [255, 0, 0]);
}

#[test]
fn the_label_names_the_levels_and_any_tolerance_in_use() {
    let mut video = Bench::new("label-video", VIDEO);
    video.shade(grey(0.5));
    let label = video.label();
    assert!(label.starts_with("Gamut/legal warning"), "{label}");
    assert!(label.contains("Legal 64–940"), "{label}");
    assert!(label.contains("red above 90 % (code 852)") && label.contains("blue below 10 % (code 152)"), "{label}");
    assert!(label.contains("no tolerance"), "{label}");
    video.choose("r103", "on");
    assert!(video.label().contains("EBU R103 tolerance on (-5 % to 105 %)"), "{}", video.label());
    drop(video);

    let mut data = Bench::new("label-data", DATA);
    data.shade(grey(0.5));
    let label = data.label();
    assert!(label.contains("Full 0–1023"), "{label}");
    assert!(label.contains("red above 90 % (code 921)") && label.contains("blue below 10 % (code 102)"), "{label}");
}

#[test]
fn before_the_colour_setup_is_known_the_label_says_what_is_assumed() {
    let b = Bench::new("label-unknown", "");
    let label = b.label();
    assert!(label.contains("assumed"), "{label}");
}

#[test]
fn the_menu_offers_two_thresholds_and_the_r103_tolerance_with_the_ticket_defaults_first() {
    let b = Bench::new("menu", VIDEO);
    let defs = b.set.setting_defs(0);
    let first = |i: usize| defs[i].choices[0].value;
    assert_eq!(defs.iter().map(|d| d.key).collect::<Vec<_>>(), ["low", "high", "r103"]);
    assert_eq!((first(0), first(1), first(2)), ("10", "90", "off"));
}

/// The ColorChecker in DaVinci Wide Gamut / Intermediate, converted to Rec.709 Gamma 2.4 by the app, drawn by the
/// Panel. Cyan is outside Rec.709 (its red is negative); no other patch is outside it, and at true exposure none
/// crosses a limit.
#[test]
fn on_the_colorchecker_in_rec709_only_cyan_is_coloured_blue() {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    let frame = chart.frame(width, height);
    let mut b = Bench::new("chart", DWG_REC709);
    b.play(width, height, &frame.pixels);
    let image = b.render(width, height);
    for patch in chart.patches() {
        let r = frame.patch_rect(patch.number);
        let (x, y) = ((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2);
        let i = (y as usize * width as usize + x as usize) * 4;
        let rgb = [image[i], image[i + 1], image[i + 2]];
        if patch.name == "cyan" {
            assert_eq!(rgb, [0, 0, 255], "cyan (negative red in Rec.709) is solid blue");
        } else {
            // Not solid red or solid blue: a patch inside Rec.709 either stays grey or has a partial tint.
            assert!(rgb != [255, 0, 0] && rgb != [0, 0, 255], "patch {} {} crosses a limit: {rgb:?}", patch.number, patch.name);
        }
    }
}
