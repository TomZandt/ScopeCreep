//! The B/W toggle (#118) at the Feed seam: the clean picture, ARRI False Colour and Stops False Colour draw their
//! unpainted picture as Luma grey or in colour, and what they paint is the same either way. Skin Tone's own B/W is
//! in its seam file. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::arri_false_colour::ArriFalseColour;
use scopecreep::display::DisplaySet;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::picture_view::PictureView;
use scopecreep::settings::PanelSettings;
use scopecreep::stops_false_colour::StopsFalseColour;
use std::sync::Arc;

const W: u32 = 16;
const H: u32 = 9;
const SETUP: &str = "timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\n";
const CLEAN: &str = "Clean picture";
const ARRI: &str = "ARRI False Colour";
const STOPS: &str = "Stops False Colour";
/// Not neutral, and in no band of ARRI's nor within a sixth of a stop of 18 % grey or one stop over it.
const COLOURFUL: [f32; 3] = [0.6, 0.4, 0.3];

struct Bench {
    rig: Rig,
    conversion: OutputConversion,
    set: DisplaySet,
    settings: PanelSettings,
}

fn poll(rig: &mut Rig, conversion: &mut OutputConversion, set: &DisplaySet, settings: &PanelSettings) {
    rig.poll(|encoder, gpu_frame| {
        let shown = conversion.convert(encoder, gpu_frame);
        set.on_frame(encoder, shown);
        set.on_timeline_frame(encoder, gpu_frame);
    });
    set.configure(&rig.monitor.status().display_context(), settings);
}

impl Bench {
    fn new(name: &str) -> Bench {
        let mut rig = Rig::with_colour_setup(name, Some(SETUP));
        let mut set = DisplaySet::default();
        set.register(Arc::new(PictureView::new(&rig.device, &rig.queue, TARGET, false)));
        set.register(Arc::new(ArriFalseColour::new(&rig.device, &rig.queue, TARGET)));
        set.register(Arc::new(StopsFalseColour::new(&rig.device, &rig.queue, TARGET)));
        let mut conversion = OutputConversion::new(&rig.device, &rig.queue);
        let mut settings = PanelSettings::default();
        for name in [ARRI, STOPS] {
            settings.set(name, "legend", "off");
        }
        settings.set(STOPS, "mode", "grey-plus-one");
        poll(&mut rig, &mut conversion, &set, &settings);
        Bench { rig, conversion, set, settings }
    }

    fn play(&mut self, rgb: [f32; 3]) {
        let pixels = common::solid(W, H, rgb);
        let (conversion, set, settings) = (&mut self.conversion, &self.set, &self.settings);
        self.rig.play(W, H, &pixels, |rig| poll(rig, conversion, set, settings));
    }

    fn bw(&mut self, panel: &str, on: bool) {
        self.settings.set(panel, "bw", if on { "on" } else { "off" });
        self.set.configure(&self.rig.monitor.status().display_context(), &self.settings);
    }

    /// The middle pixel of `panel`.
    fn shown(&self, panel: &str) -> [u8; 3] {
        let index = self.set.names().iter().position(|n| *n == panel).unwrap();
        let image = self.rig.render(W, H, |pass| self.set.draw(pass, index, [0.0, 0.0, W as f32, H as f32], [W as f32, H as f32], 1.0));
        let i = ((H / 2) as usize * W as usize + (W / 2) as usize) * 4;
        [image[i], image[i + 1], image[i + 2]]
    }
}

fn is_grey(p: [u8; 3]) -> bool {
    p[0].abs_diff(p[1]) <= 1 && p[1].abs_diff(p[2]) <= 1
}

#[test]
fn the_clean_picture_is_in_colour_until_b_w_and_then_its_luma_grey() {
    let mut b = Bench::new("bw-clean");
    b.play(COLOURFUL);
    let colour = b.shown(CLEAN);
    assert!(colour[0] > colour[2] + 40, "{colour:?}");
    b.bw(CLEAN, true);
    let grey = b.shown(CLEAN);
    let luma = (0.2126 * 0.6 + 0.7152 * 0.4 + 0.0722 * 0.3) * 255.0;
    assert!(is_grey(grey) && (grey[0] as f32 - luma).abs() <= 2.0, "{grey:?} vs {luma}");
    b.bw(CLEAN, false);
    assert_eq!(b.shown(CLEAN), colour, "and back");
}

#[test]
fn arri_is_grey_by_default_and_colour_when_toggled_and_its_bands_do_not_change() {
    let mut b = Bench::new("bw-arri");
    b.play(COLOURFUL);
    assert!(is_grey(b.shown(ARRI)), "{:?}", b.shown(ARRI));
    b.bw(ARRI, false);
    let colour = b.shown(ARRI);
    assert!(colour[0] > colour[2] + 40, "{colour:?}");
    // 40 % Luma is in the green band, in either mode.
    b.play([0.4; 3]);
    let green = b.shown(ARRI);
    assert!(green[1] > green[0] + 100, "{green:?}");
    b.bw(ARRI, true);
    assert_eq!(b.shown(ARRI), green);
}

#[test]
fn stops_grey_and_plus_one_is_grey_by_default_and_colour_when_toggled_and_its_marks_do_not_change() {
    let mut b = Bench::new("bw-stops");
    b.play(COLOURFUL);
    assert!(is_grey(b.shown(STOPS)), "{:?}", b.shown(STOPS));
    b.bw(STOPS, false);
    let colour = b.shown(STOPS);
    assert!(colour[0] > colour[2] + 40, "{colour:?}");
    // 18 % grey (encoded 0.18^(1/2.4)) is painted green in either mode.
    let v = 0.18f32.powf(1.0 / 2.4);
    b.play([v; 3]);
    let green = b.shown(STOPS);
    assert!(green[1] > green[0] + 100, "{green:?}");
    b.bw(STOPS, true);
    assert_eq!(b.shown(STOPS), green);
}
