//! Highlight Neutrals (#65), proven at the Feed seam: synthetic frames go in through the synthetic writer, the
//! app's Feed monitor and Output Conversion hand them to the display exactly as the app does, and the test reads
//! back what the Panel drew and its label. Expected neutrals come from the ColorChecker chart code (#52). Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::{Display, DisplayContext};
use scopecreep::highlight_neutrals::HighlightNeutrals;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::settings::PanelSettings;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};

struct Bench {
    rig: Rig,
    conversion: OutputConversion,
    display: HighlightNeutrals,
}

/// What the app does after every poll of the Feed: upload, Output Conversion, the display, and the conversion
/// following the Colour Setup.
fn poll(rig: &mut Rig, conversion: &mut OutputConversion, display: &HighlightNeutrals) {
    rig.poll(|encoder, gpu_frame| {
        let shown = conversion
            .convert(encoder, gpu_frame)
            .expect("GPU memory for the converted picture");
        display.on_frame(encoder, shown);
    });
    let setup = rig.monitor.status().colour_setup().map(|c| c.setup);
    conversion.follow(setup.as_ref(), rig.uploader.frame(), |encoder, shown| {
        display.on_frame(encoder, shown)
    });
}

impl Bench {
    /// `colour_setup`: the Sender's manual override (`key=value` lines), or none for a picture shown as sent.
    fn new(name: &str, colour_setup: Option<&str>) -> Bench {
        let mut rig = Rig::with_colour_setup(name, colour_setup);
        let (mut conversion, display) = (
            OutputConversion::new(&rig.device, &rig.queue),
            HighlightNeutrals::new(&rig.device, &rig.queue, TARGET, false),
        );
        poll(&mut rig, &mut conversion, &display);
        let b = Bench {
            rig,
            conversion,
            display,
        };
        b.configure(None);
        b
    }

    /// The Colour Setup the Feed carries and the user's tolerance choice (a percentage; none is the default).
    fn configure(&self, tolerance: Option<&str>) {
        let mut settings = PanelSettings::default();
        if let Some(tolerance) = tolerance {
            settings.set(self.display.name(), "tolerance", tolerance);
        }
        let known = self.rig.monitor.status().colour_setup();
        let colour_label = known
            .as_ref()
            .map_or("Colour Setup unknown".to_string(), |c| {
                c.setup.timeline.clone()
            });
        let context = DisplayContext {
            colour_setup: known,
            colour_label,
            settings: settings.resolve(self.display.name(), self.display.setting_defs()),
            ..DisplayContext::default()
        };
        self.display.configure(&context);
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let (conversion, display) = (&mut self.conversion, &self.display);
        self.rig
            .play(width, height, pixels, |rig| poll(rig, conversion, display));
        self.configure(None);
    }

    /// What the Panel drew into a `width` × `height` target that has the frame's own shape (RGBA8, rows top first).
    fn render(&self, width: u32, height: u32) -> Vec<u8> {
        self.rig.render(width, height, |pass| {
            self.display.draw(
                pass,
                [0.0, 0.0, width as f32, height as f32],
                [width as f32, height as f32],
                1.0,
            )
        })
    }
}

fn pixel(image: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * width as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// Shown green: the green channel clearly above both others.
fn is_green([r, g, b]: [u8; 3]) -> bool {
    g as i32 > r as i32 + 60 && g as i32 > b as i32 + 60
}

/// Shown grey: all three channels equal.
fn is_grey([r, g, b]: [u8; 3]) -> bool {
    r == g && g == b
}

const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";
const ACESCCT_REC709: &str = "timeline=ACEScct\noutput=Rec.709 BT.1886\naces_version=2.0\n";

fn assert_chart_greys_green_and_colours_not(name: &str, space: TimelineSpace, setup: &str) {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(space);
    let frame = chart.frame(width, height);
    let mut b = Bench::new(name, Some(setup));
    b.play(width, height, &frame.pixels);
    let image = b.render(width, height);
    for patch in chart.patches() {
        let r = frame.patch_rect(patch.number);
        let (mut green, mut total) = (0, 0);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                total += 1;
                green += is_green(pixel(&image, width, x, y)) as u32;
            }
        }
        if patch.number >= 19 {
            assert_eq!(
                green, total,
                "{space:?} grey patch {} {} is green all over",
                patch.number, patch.name
            );
        } else {
            assert_eq!(
                green, 0,
                "{space:?} coloured patch {} {} has no green",
                patch.number, patch.name
            );
        }
    }
}

#[test]
fn every_grey_colorchecker_patch_is_green_in_a_davinci_intermediate_chart_and_no_coloured_patch_is()
{
    assert_chart_greys_green_and_colours_not("di", TimelineSpace::DavinciIntermediate, DWG_REC709);
}

#[test]
fn every_grey_colorchecker_patch_is_green_in_an_acescct_chart_and_no_coloured_patch_is() {
    assert_chart_greys_green_and_colours_not("acescct", TimelineSpace::AcesCct, ACESCCT_REC709);
}

/// A flat frame whose red and green are `base` and whose blue is `base + offset`, shown as sent (the values are
/// exactly what the display sees).
fn offset_grey(base: f32, offset: f32) -> Vec<f32> {
    (0..64 * 36)
        .flat_map(|_| [base, base, base + offset, 1.0])
        .collect()
}

fn centre(b: &Bench) -> [u8; 3] {
    pixel(&b.render(64, 36), 64, 32, 18)
}

#[test]
fn a_grey_with_a_small_channel_offset_turns_green_only_once_the_tolerance_covers_it() {
    let mut b = Bench::new("offset", None);
    b.play(64, 36, &offset_grey(0.5, 0.04));
    // The offset is 4 % of the range: not neutral at the default 3 %, so it stays grey; neutral from 5 % on.
    assert!(
        is_grey(centre(&b)),
        "4 % off at 3 % tolerance stays grey: {:?}",
        centre(&b)
    );
    b.configure(Some("5"));
    assert!(
        is_green(centre(&b)),
        "4 % off at 5 % tolerance is green: {:?}",
        centre(&b)
    );
    b.configure(Some("1"));
    assert!(
        is_grey(centre(&b)),
        "4 % off at 1 % tolerance is grey again: {:?}",
        centre(&b)
    );
    // 2 % off is within the default 3 %.
    b.play(64, 36, &offset_grey(0.5, 0.02));
    assert!(
        is_green(centre(&b)),
        "2 % off at the default 3 % is green: {:?}",
        centre(&b)
    );
}

#[test]
fn dragging_the_tolerance_up_only_ever_lights_more_and_a_value_in_between_works() {
    let mut b = Bench::new("monotonic", None);
    b.play(64, 36, &offset_grey(0.5, 0.04));
    let lit: Vec<bool> = (1..=20)
        .map(|half| {
            b.configure(Some(&(half as f32 * 0.5).to_string()));
            is_green(centre(&b))
        })
        .collect();
    assert!(
        lit.windows(2).all(|w| !w[0] || w[1]),
        "lit area shrank as the slider went up: {lit:?}"
    );
    // 4 % off: not neutral at 3.5 %, neutral at 4.5 % (4 % itself is a float coin toss).
    assert!(!lit[6] && lit[8], "{lit:?}");
    // Out of range clamps to 10 %, which lights it.
    b.configure(Some("50"));
    assert!(is_green(centre(&b)));
}

#[test]
fn a_coloured_pixel_is_shown_in_grey_that_is_its_luma() {
    let mut b = Bench::new("colour", None);
    b.play(
        64,
        36,
        &(0..64 * 36)
            .flat_map(|_| [1.0, 0.0, 0.0, 1.0])
            .collect::<Vec<f32>>(),
    );
    let [r, g, bl] = centre(&b);
    assert!(is_grey([r, g, bl]), "{r} {g} {bl}");
    // Rec.709's red weight, 0.2126, as the waveform weighs it when the Colour Setup is unknown (target not sRGB).
    assert!(
        (r as i32 - (0.2126_f32 * 255.0).round() as i32).abs() <= 1,
        "{r}"
    );
}

#[test]
fn the_tolerance_is_a_slider_of_percent_defaulting_to_three_and_the_label_names_it() {
    let mut b = Bench::new("label", Some(DWG_REC709));
    let defs = b.display.setting_defs();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].key, "tolerance");
    let amount = defs[0].amount.expect("a slider");
    assert_eq!((amount.default, amount.text(3.0).as_str()), (3.0, "3.0 %"));
    assert!(defs[0].choices.is_empty());
    b.play(
        16,
        9,
        &(0..16 * 9)
            .flat_map(|_| [0.5, 0.5, 0.5, 1.0])
            .collect::<Vec<f32>>(),
    );
    let label = b.display.label();
    assert!(label.starts_with("Neutrals"), "{label}");
    assert!(label.contains("Output picture"), "{label}");
    assert!(label.contains("within 3.0 %"), "{label}");
    assert!(label.contains("DaVinci WG/Intermediate"), "{label}");
    assert!(label.contains("Output Conversion"), "{label}");
    b.configure(Some("5"));
    assert!(
        b.display.label().contains("within 5.0 %"),
        "{}",
        b.display.label()
    );
}

#[test]
fn nothing_is_drawn_before_a_frame_arrives() {
    let b = Bench::new("empty", None);
    assert_eq!(pixel(&b.render(64, 36), 64, 32, 18), [0, 0, 0]);
    assert_eq!(b.display.label(), "Neutrals");
}
