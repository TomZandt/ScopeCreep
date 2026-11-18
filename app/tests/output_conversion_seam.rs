//! Output Conversion (ticket #17), driven at the Feed seam: synthetic Timeline Colour Space frames go in through
//! the synthetic writer with a Colour Setup (the Sender's manual override travels in the Feed), the app's Feed
//! monitor hands them to the GPU exactly as the app does, and the tests look at what a colourist would see: the
//! converted picture's values, the waveform of the converted picture, and the labels. Expected values come from
//! the f64 published maths in the accuracy crate (the transforms proven against Resolve's renders in #10), never
//! from the shader. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::project::{Answer, ProjectReader};
use scopecreep::output_conversion::{self, OutputConversion};
use scopecreep::waveform::{LumaWeights, Waveform, WaveformCounts};
use scopecreep_accuracy::aces2::AcesOutputTransform;
use scopecreep_accuracy::compare::{TOLERANCE_FLOAT, TOLERANCE_LINEAR};
use scopecreep_accuracy::pipeline::{Encoding, Pipeline};
use scopecreep_accuracy::colour::REC2020;
use scopecreep_accuracy::transfer;

/// A stand-in Resolve and Sender (the synthetic writer), the app's Feed monitor, and the app's GPU side of the
/// Feed: upload, Output Conversion, waveform, as `main.rs` wires them.
struct Bench {
    rig: Rig,
    conversion: OutputConversion,
    waveform: Waveform,
}

/// As the app does after every poll: upload, Output Conversion, measure in the Measured Space, and follow the
/// Colour Setup.
fn poll(rig: &mut Rig, conversion: &mut OutputConversion, waveform: &Waveform) {
    rig.poll(|encoder, gpu_frame| {
        let shown = conversion.convert(encoder, gpu_frame);
        waveform.measure(encoder, shown);
    });
    let setup = rig.monitor.status().colour_setup().map(|c| c.setup);
    waveform.set_measurement(output_conversion::measurement(rig.monitor.status().colour_setup(), LumaWeights::FromPrimaries));
    conversion.follow(setup.as_ref(), rig.uploader.frame(), |encoder, shown| waveform.measure(encoder, shown));
}

impl Bench {
    /// The Sender node says the project is `colour_setup` (protocol 1.1 `key=value` lines).
    fn new(name: &str, colour_setup: &str) -> Bench {
        Bench::make(name, Some(colour_setup), None)
    }

    /// No manual Colour Setup on the Sender; the project, read by script once Resolve stops, has `settings`
    /// (Resolve's own setting keys, as the scripting helper reports them).
    fn with_project(name: &str, settings: &[(&str, &str)]) -> Bench {
        let mut answer: Answer = vec![("result".into(), "ok".into()), ("project".into(), "scopecreep-test".into()), ("studio".into(), "1".into())];
        answer.extend(settings.iter().map(|(k, v)| (format!("setting.{k}"), v.to_string())));
        Bench::make(name, None, Some(Box::new(Helper(answer))))
    }

    fn make(name: &str, colour_setup: Option<&str>, project: Option<Box<dyn ProjectReader>>) -> Bench {
        let mut rig = Rig::build(name, 1024 * 64 * 16, colour_setup, project);
        let (mut conversion, waveform) = (OutputConversion::new(&rig.device, &rig.queue), Waveform::new(&rig.device, &rig.queue, TARGET));
        poll(&mut rig, &mut conversion, &waveform);
        Bench { rig, conversion, waveform }
    }

    /// Resolve stays stopped for `ms` milliseconds.
    fn wait(&mut self, ms: u64) {
        let (conversion, waveform) = (&mut self.conversion, &self.waveform);
        self.rig.wait(ms, |rig| poll(rig, conversion, waveform));
    }

    /// Resolve plays one frame of Timeline Colour Space values (`rgb(x, row from the top)`), and the app has a
    /// moment to take it.
    fn play(&mut self, width: u32, height: u32, rgb: impl Fn(u32, u32) -> [f32; 3]) {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for row in 0..height {
            let from_top = height - 1 - row; // the Feed stores rows bottom first
            for x in 0..width {
                let [r, g, b] = rgb(x, from_top);
                pixels.extend_from_slice(&[r, g, b, 1.0]);
            }
        }
        let (conversion, waveform) = (&mut self.conversion, &self.waveform);
        self.rig.play(width, height, &pixels, |rig| poll(rig, conversion, waveform));
    }

    /// The picture the Picture View shows and the waveform measures, top row first, RGB.
    fn shown(&self) -> Vec<[f32; 3]> {
        self.conversion.read_shown(self.rig.uploader.frame().expect("a frame was shown"))
    }

    fn label(&self) -> String {
        let status = self.rig.monitor.status();
        scopecreep::picture_view::PictureView::label_for(status.picture.as_ref().map(|p| [p.width, p.height]), &status.display_context())
    }

    fn counts(&self) -> WaveformCounts {
        self.waveform.counts()
    }
}

/// A scripting helper that answers every read at once with the same project.
struct Helper(Answer);

impl ProjectReader for Helper {
    fn request(&mut self) {}
    fn take_answer(&mut self) -> Option<Answer> {
        Some(self.0.clone())
    }
    fn abandon(&mut self) {}
}

const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\nlevels=video\n";

// ------------------------------------------------------------------ labels

/// A Colour Setup set by hand on the Sender cannot say whether tone or gamut mapping is on, so the label does not
/// claim the conversion is exact: it says what it assumes.
#[test]
fn a_colour_setup_set_by_hand_does_not_claim_to_be_exact() {
    let mut b = Bench::new("label-manual", DWG_REC709);
    b.play(16, 9, |_, _| [0.4, 0.4, 0.4]);
    assert_eq!(
        b.label(),
        "Clean picture  ·  16 × 9  ·  DaVinci WG/Intermediate → Rec.709 Gamma 2.4 (set manually)  ·  Output Conversion approximated: assumes tone and gamut mapping off"
    );
}

// ------------------------------------------------------------------ converted values

/// Timeline code values that exercise a transform: a grey ramp from below 0 to above 1, and every mix of a few
/// levels per channel (saturated colours, out-of-range channels).
fn test_values() -> Vec<[f32; 3]> {
    let mut v: Vec<[f32; 3]> = (0..=260).map(|i| { let x = -0.15 + i as f32 * 0.005; [x, x, x] }).collect();
    let levels = [-0.05f32, 0.0, 0.02, 0.1, 0.25, 0.4, 0.55, 0.7, 0.85, 1.0, 1.2];
    for r in levels {
        for g in levels {
            for b in levels {
                v.push([r, g, b]);
            }
        }
    }
    v
}

/// Plays `values` (one per pixel, 64 to a row) and checks every channel of the converted picture against the f64
/// `reference`, within the accuracy suite's "exact" tolerance (#10): 1e-4 encoded, or 5e-6 in display light.
fn assert_converted_like(b: &mut Bench, reference: &Pipeline, values: &[[f32; 3]]) {
    let width = 64u32;
    let height = values.len().div_ceil(64) as u32;
    let at = |x: u32, y: u32| values.get((y * width + x) as usize).copied().unwrap_or([0.0; 3]);
    b.play(width, height, at);
    let shown = b.shown();
    let mut worst = (0.0f64, [0f32; 3], [0f32; 3], [0f64; 3]);
    let mut failing = 0;
    for y in 0..height {
        for x in 0..width {
            let input = at(x, y);
            let ours = shown[(y * width + x) as usize];
            let want = reference.apply([input[0] as f64, input[1] as f64, input[2] as f64]);
            for c in 0..3 {
                let err = (ours[c] as f64 - want[c]).abs();
                let light = (reference.decode(ours[c] as f64) - reference.decode(want[c])).abs();
                if !(err <= TOLERANCE_FLOAT || light <= TOLERANCE_LINEAR) {
                    failing += 1;
                }
                if !(err <= worst.0) {
                    worst = (err, input, ours, want);
                }
            }
        }
    }
    assert_eq!(failing, 0, "{failing} channel values outside the tolerance; worst {:.3e}: in {:?} ours {:?} reference {:?}", worst.0, worst.1, worst.2, worst.3);
}

#[test]
fn dwg_to_rec709_gamma_24_matches_the_published_maths() {
    let mut b = Bench::new("dwg-rec709", DWG_REC709);
    assert_converted_like(&mut b, &Pipeline::dwg_rec709(), &test_values());
}

#[test]
fn dwg_to_p3_d65_matches_the_published_maths_at_gamma_26() {
    let mut b = Bench::new("dwg-p3d65", "timeline=DaVinci WG/Intermediate\noutput=P3-D65\n");
    assert_converted_like(&mut b, &Pipeline::dwg_p3d65(), &test_values());
}

#[test]
fn dwg_to_rec2100_pq_matches_the_published_maths() {
    let mut b = Bench::new("dwg-pq", "timeline=DaVinci WG/Intermediate\noutput=Rec.2100 ST2084\n");
    assert_converted_like(&mut b, &Pipeline::dwg_rec2100_pq(), &test_values());
}

/// ACEScct code values: from below 0 linear to far above white (1.3 is about 222 x 18% grey).
fn acescct_values() -> Vec<[f32; 3]> {
    let mut v: Vec<[f32; 3]> = (0..=260).map(|i| { let x = i as f32 * 0.005; [x, x, x] }).collect();
    let levels = [0.0f32, 0.07, 0.15, 0.25, 0.35, 0.41, 0.5, 0.6, 0.75, 0.9, 1.1];
    for &r in &levels {
        for &g in &levels {
            for &b in &levels {
                v.push([r, g, b]);
            }
        }
    }
    v
}

#[test]
fn acescct_to_aces_2_rec709_matches_the_reference_ctl_port() {
    let mut b = Bench::new("aces-rec709", "timeline=ACEScct\noutput=Rec.709 BT.1886\naces_version=2.0\n");
    assert_converted_like(&mut b, &Pipeline::Aces2(Box::new(AcesOutputTransform::rec709_bt1886())), &acescct_values());
}

#[test]
fn acescct_to_aces_2_p3_d65_matches_the_reference_ctl_port() {
    let mut b = Bench::new("aces-p3d65", "timeline=ACEScct\noutput=P3-D65\naces_version=2.0\n");
    assert_converted_like(&mut b, &Pipeline::Aces2(Box::new(AcesOutputTransform::p3_d65())), &acescct_values());
}

#[test]
fn acescct_to_aces_2_rec2100_pq_1000_nit_matches_the_reference_ctl_port() {
    let mut b = Bench::new("aces-pq", "timeline=ACEScct\noutput=Rec.2100 ST2084 (1000 nit)\naces_version=2.0\n");
    assert_converted_like(&mut b, &Pipeline::Aces2(Box::new(AcesOutputTransform::rec2100_pq_1000())), &acescct_values());
}

// ------------------------------------------------------------------ approximated and unsupported

#[test]
fn rec2020_gamma_24_is_converted_with_published_maths_but_labelled_approximated() {
    let mut b = Bench::new("dwg-rec2020", "timeline=DaVinci WG/Intermediate\noutput=Rec.2020 Gamma 2.4\n");
    assert_converted_like(&mut b, &Pipeline::dwg_to_display(REC2020, Encoding::SignedGamma(2.4)), &test_values());
    assert!(b.label().ends_with("Output Conversion approximated: Rec.2020 Gamma 2.4 not yet measured against Resolve"), "{}", b.label());
}

#[test]
fn davinci_tone_mapping_is_not_applied_and_the_label_says_so() {
    let mut b = Bench::new("dwg-tone-mapping", "timeline=DaVinci WG/Intermediate\noutput=Rec.2100 ST2084\ntone_mapping=DaVinci\n");
    assert_converted_like(&mut b, &Pipeline::dwg_rec2100_pq(), &test_values());
    assert!(b.label().ends_with("Output Conversion approximated: DaVinci tone mapping not applied (unpublished)"), "{}", b.label());
}

#[test]
fn gamut_mapping_is_not_applied_and_the_label_says_so() {
    let mut b = Bench::new("dwg-gamut-mapping", "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\ngamut_mapping=Saturation Compression\n");
    b.play(16, 9, |_, _| [0.4, 0.4, 0.4]);
    assert!(b.label().ends_with("Output Conversion approximated: Saturation Compression gamut mapping not applied (unpublished)"), "{}", b.label());
}

#[test]
fn a_yrgb_project_is_shown_as_sent_because_resolve_converts_nothing() {
    let mut b = Bench::new("yrgb", "timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\nscience=yrgb\n");
    assert_converted_like(&mut b, &Pipeline::Identity, &test_values());
    assert!(b.label().ends_with("Output Conversion exact"), "{}", b.label());
}

/// Colour Setups with no conversion yet: the picture is shown and measured exactly as sent, and the label says
/// there is no Output Conversion and why.
#[test]
fn colour_setups_without_a_conversion_are_shown_as_sent_and_say_why() {
    for (name, setup, why) in [
        ("hlg", "timeline=DaVinci WG/Intermediate\noutput=Rec.2100 HLG\n", "output Rec.2100 HLG not yet supported"),
        ("p3dci", "timeline=DaVinci WG/Intermediate\noutput=P3-DCI\n", "output P3-DCI not yet supported"),
        ("aces13", "timeline=ACEScct\noutput=Rec.709 BT.1886\naces_version=1.3\n", "ACES 1.3 not yet supported"),
        ("acescc", "timeline=ACEScc\noutput=Rec.709 BT.1886\naces_version=2.0\n", "ACEScc timelines not yet supported"),
        ("aces-odt", "timeline=ACEScct\noutput=P3-D65 (1000 nit)\naces_version=2.0\n", "ACES output transform P3-D65 (1000 nit) not yet supported"),
    ] {
        let mut b = Bench::new(name, setup);
        assert_converted_like(&mut b, &Pipeline::Identity, &test_values());
        assert!(b.label().ends_with(&format!("as sent, no Output Conversion: {why}")), "{name}: {}", b.label());
        assert!(b.waveform.label().contains("Rec.709 assumed, measured as sent, unconverted"), "{name}: the waveform says it measures as sent: {}", b.waveform.label());
    }
}

#[test]
fn an_aces_project_whose_version_is_assumed_is_exact_only_for_aces_2() {
    let mut b = Bench::with_project("aces-assumed", &[("colorScienceMode", "acescct"), ("colorAcesODT", "Rec.709 BT.1886")]);
    b.play(16, 9, |_, _| [0.4, 0.4, 0.4]);
    b.wait(1100);
    assert!(b.label().ends_with("ACEScct → Rec.709 BT.1886 (from project, assumes ACES 2.0)  ·  Output Conversion exact for ACES 2.0"), "{}", b.label());
}

// ------------------------------------------------------------------ what the waveform measures

/// 10-bit code of an encoded value at video levels (black 64, white 940: #18), as the waveform rounds it.
fn code(v: f64) -> u32 {
    (v * 876.0 + 64.0).round() as u32
}

#[test]
fn the_waveform_measures_the_converted_picture() {
    // DaVinci Intermediate 18% grey and 90% white: as sent (Timeline Colour Space) they would sit at codes 358 and 497.
    let mut b = Bench::new("waveform", DWG_REC709);
    let (grey, white) = (transfer::di_encode(0.18) as f32, transfer::di_encode(0.9) as f32);
    b.play(8, 100, |x, _| if x < 4 { [grey; 3] } else { [white; 3] });
    let reference = Pipeline::dwg_rec709();
    let (grey_code, white_code) = (code(reference.apply([grey as f64; 3])[1]), code(reference.apply([white as f64; 3])[1]));
    // Rec.709 Gamma 2.4: 0.18 ^ (1 / 2.4) x 876 + 64 = 492.8, 0.9 ^ (1 / 2.4) x 876 + 64 = 902.4.
    assert_eq!((grey_code, white_code), (493, 902), "Rec.709 Gamma 2.4 codes of 18% grey and 90% white");
    let counts = b.counts();
    for x in 0..8 {
        let want = if x < 4 { grey_code } else { white_code };
        assert_eq!(counts.at(x, want), 100, "column {x}: every pixel at code {want}");
    }
    assert!(b.waveform.label().contains("Rec.709  ·"), "{}", b.waveform.label());
    assert!(!b.waveform.label().contains("assumed"), "{}", b.waveform.label());
}

#[test]
fn the_waveform_measures_an_aces_picture_after_the_output_transform() {
    // ACEScct 18% grey: through ACES 2.0 Rec.709 BT.1886 it lands at 10.013 nit of 100 (the CTL tonescale).
    let mut b = Bench::new("waveform-aces", "timeline=ACEScct\noutput=Rec.709 BT.1886\naces_version=2.0\n");
    let grey = transfer::acescct_encode(0.18) as f32;
    b.play(4, 50, |_, _| [grey; 3]);
    let want = code(Pipeline::Aces2(Box::new(AcesOutputTransform::rec709_bt1886())).apply([grey as f64; 3])[1]);
    assert_eq!(want, 400, "ACES 2.0 puts 18% grey at 10.013 nit: 0.10013 ^ (1 / 2.4) x 876 + 64 = 399.8");
    let counts = b.counts();
    for x in 0..4 {
        assert_eq!(counts.at(x, want), 50, "column {x}");
    }
}

#[test]
fn a_colour_setup_read_after_playback_stops_converts_the_picture_already_on_screen() {
    let mut b = Bench::with_project(
        "late-setup",
        &[("colorScienceMode", "davinciYRGBColorManagedv2"), ("colorSpaceTimeline", "DaVinci WG/Intermediate"), ("colorSpaceOutput", "Rec.709 Gamma 2.4")],
    );
    let grey = transfer::di_encode(0.18) as f32;
    b.play(4, 10, |_, _| [grey; 3]);
    assert!(b.label().ends_with("as sent, no Output Conversion"), "before the project is read: {}", b.label());
    assert_eq!(b.counts().at(0, code(grey as f64)), 10, "measured as sent");
    b.wait(1100); // Resolve stays stopped: the project is read, and no new frame comes
    assert!(b.label().ends_with("Output Conversion exact"), "{}", b.label());
    let want = code(Pipeline::dwg_rec709().apply([grey as f64; 3])[1]);
    assert_eq!(b.counts().at(0, want), 10, "the same frame, measured again after conversion");
    assert!((b.shown()[0][1] as f64 - Pipeline::dwg_rec709().apply([grey as f64; 3])[1]).abs() < 1e-4, "the Picture View shows the converted frame");
}
