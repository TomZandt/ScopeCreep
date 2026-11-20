//! The black-and-white Panel (#54, spec #49 stories 47 and 48), driven at the Feed seam: ColorChecker frames go in
//! through the synthetic writer, the app's Feed monitor and Output Conversion hand them to every display as the
//! app does, and the tests read what the waveform measured and what the black-and-white Panel drew, and its label.
//! A grey you see must line up with its trace: within one 8-bit step. Needs a GPU.
mod common;
use common::{Rig, TARGET, layouts};
use scopecreep::black_and_white::BlackAndWhite;
use scopecreep::display::DisplaySet;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::panel_grid::PanelLayout;
use scopecreep::picture_view::PictureView;
use scopecreep::settings::PanelSettings;
use scopecreep::waveform::Waveform;
use scopecreep::waveform::WaveformCounts;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use std::sync::Arc;

const WAVEFORM: &str = "Luma waveform";
const BLACK_AND_WHITE: &str = "Black and white";

/// A stand-in Resolve and Sender, the app's Feed monitor, and the app's GPU side of the Feed, wired as `main.rs`
/// does: upload, Output Conversion, every display, and the Colour Setup and settings given to every display.
struct Bench {
    rig: Rig,
    conversion: OutputConversion,
    set: DisplaySet,
    luma: Waveform,
    settings: PanelSettings,
}

/// What the app does after every poll of the Feed: upload, Output Conversion, every display, the Colour Setup and
/// settings given to every display, and the conversion following the Colour Setup.
fn poll(
    rig: &mut Rig,
    conversion: &mut OutputConversion,
    set: &DisplaySet,
    settings: &PanelSettings,
) {
    rig.poll(|encoder, gpu_frame| {
        let shown = conversion
            .convert(encoder, gpu_frame)
            .expect("GPU memory for the converted picture");
        set.on_frame(encoder, shown);
    });
    let setup = rig.monitor.status().colour_setup().map(|c| c.setup);
    set.configure(&rig.monitor.status().display_context(), settings);
    conversion.follow(setup.as_ref(), rig.uploader.frame(), |encoder, shown| {
        set.on_frame(encoder, shown)
    });
}

/// The Window's displays plus the black and white Panel, which is out of the Window (#118) but kept and tested.
fn standard_with_black_and_white(rig: &Rig) -> (DisplaySet, Waveform) {
    let mut set = DisplaySet::default();
    set.register(Arc::new(PictureView::new(
        &rig.device,
        &rig.queue,
        TARGET,
        false,
    )));
    let luma = Waveform::new(&rig.device, &rig.queue, TARGET);
    set.register(Arc::new(luma.clone()));
    set.register(Arc::new(BlackAndWhite::new(
        &rig.device,
        &rig.queue,
        TARGET,
        false,
    )));
    (set, luma)
}

impl Bench {
    fn new(name: &str, colour_setup: &str) -> Bench {
        let mut rig = Rig::with_colour_setup(name, Some(colour_setup));
        let (mut conversion, (set, luma), settings) = (
            OutputConversion::new(&rig.device, &rig.queue),
            standard_with_black_and_white(&rig),
            PanelSettings::default(),
        );
        poll(&mut rig, &mut conversion, &set, &settings);
        Bench {
            rig,
            conversion,
            set,
            luma,
            settings,
        }
    }

    fn index(&self, name: &str) -> usize {
        self.set
            .names()
            .iter()
            .position(|n| *n == name)
            .unwrap_or_else(|| panic!("no display called {name}"))
    }

    /// The user picks a choice in a Panel's menu: the displays take it and measure the frame on screen again.
    fn choose(&mut self, panel: &str, key: &str, value: &str) {
        self.settings.set(panel, key, value);
        self.set
            .configure(&self.rig.monitor.status().display_context(), &self.settings);
        let set = &self.set;
        self.conversion
            .again(self.rig.uploader.frame(), |encoder, shown| {
                set.on_frame(encoder, shown)
            });
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let (conversion, set, settings) = (&mut self.conversion, &self.set, &self.settings);
        self.rig.play(width, height, pixels, |rig| {
            poll(rig, conversion, set, settings)
        });
    }

    fn counts(&self) -> WaveformCounts {
        self.luma.counts()
    }

    /// Draw every display into its Panel of a `width` × `height` target as the window does and read it back
    /// (RGBA8, rows top first), with the Panel of the black-and-white display.
    fn render(&self, width: u32, height: u32) -> (Vec<u8>, PanelLayout) {
        let panels = layouts(self.set.len(), width, height, 1.0);
        let image = self.rig.render(width, height, |pass| {
            for (i, panel) in panels.iter().enumerate() {
                self.set
                    .draw(pass, i, panel.content, [width as f32, height as f32], 1.0);
            }
        });
        (image, panels[self.index(BLACK_AND_WHITE)])
    }
}

/// Where picture pixel (`x`, `y`) of a `picture` sized frame is drawn when it is fitted into `area`.
fn on_screen(area: [f32; 4], picture: [u32; 2], x: f32, y: f32) -> (usize, usize) {
    let scale = (area[2] / picture[0] as f32).min(area[3] / picture[1] as f32);
    let (w, h) = (picture[0] as f32 * scale, picture[1] as f32 * scale);
    let (left, top) = (
        (area[0] + (area[2] - w) / 2.0).round(),
        (area[1] + (area[3] - h) / 2.0).round(),
    );
    ((left + x * scale) as usize, (top + y * scale) as usize)
}

fn grey_at(image: &[u8], width: u32, (x, y): (usize, usize)) -> [u8; 3] {
    let i = (y * width as usize + x) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";
const DWG_P3D65: &str = "timeline=DaVinci WG/Intermediate\noutput=P3-D65\n";
const PICTURE: [u32; 2] = [600, 400];

/// Plays the chart with only patch `number` painted (the rest black, so its columns hold nothing else), and
/// reports the code the waveform traced the patch at, and the grey the black-and-white Panel drew for it.
/// Video levels: code = 64 + 876 Luma.
fn trace_and_draw(b: &mut Bench, chart: &ColorChecker, number: usize) -> (u32, [u8; 3]) {
    let mut frame = chart.frame(PICTURE[0], PICTURE[1]);
    let black = [frame.pixels[0], frame.pixels[1], frame.pixels[2]];
    for other in (1..=24).filter(|n| *n != number) {
        let r = frame.patch_rect(other);
        for y in r.y0..r.y1 {
            let row = (PICTURE[1] - 1 - y) as usize; // rows are stored bottom first
            for x in r.x0..r.x1 {
                let i = (row * PICTURE[0] as usize + x as usize) * 4;
                frame.pixels[i..i + 3].copy_from_slice(&black);
            }
        }
    }
    b.play(PICTURE[0], PICTURE[1], &frame.pixels);
    let r = frame.patch_rect(number);
    let (cx, cy) = ((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2);
    let counts = b.counts();
    // Every pixel of the patch's middle column that is not the black around it sits on one code.
    let traced = (0..1024u32)
        .find(|c| counts.at(cx, *c) == r.y1 - r.y0)
        .unwrap_or_else(|| (0..1024u32).max_by_key(|c| counts.at(cx, *c)).unwrap());
    let (image, panel) = b.render(1500, 600);
    (
        traced,
        grey_at(
            &image,
            1500,
            on_screen(panel.content, PICTURE, cx as f32 + 0.5, cy as f32 + 0.5),
        ),
    )
}

fn assert_drawn_is_traced(number: usize, name: &str, (code, drawn): (u32, [u8; 3])) {
    let want = ((code as f64 - 64.0) / 876.0 * 255.0).clamp(0.0, 255.0);
    assert!(
        drawn[0] == drawn[1] && drawn[1] == drawn[2],
        "patch {number} {name}: drawn grey, not colour: {drawn:?}"
    );
    assert!(
        (drawn[0] as f64 - want).abs() <= 1.0,
        "patch {number} {name}: drawn {} but its trace (code {code}) says {want:.2}",
        drawn[0]
    );
}

#[test]
fn every_colorchecker_patch_is_drawn_at_the_luma_the_waveform_traces_for_it() {
    let mut b = Bench::new("patches", DWG_REC709);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    for patch in chart.patches() {
        assert_drawn_is_traced(
            patch.number,
            patch.name,
            trace_and_draw(&mut b, &chart, patch.number),
        );
    }
}

#[test]
fn the_waveforms_luma_weights_setting_keeps_the_two_in_agreement() {
    // Output P3-D65: from primaries and Resolve's weights differ, so the two modes draw different greys.
    let mut b = Bench::new("weights", DWG_P3D65);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    let green = 14; // the chart's green: the weights matter most there
    let primaries = trace_and_draw(&mut b, &chart, green);
    assert_drawn_is_traced(green, "green", primaries);
    let label = b.set.labels()[b.index(BLACK_AND_WHITE)].clone();
    assert!(label.contains("from primaries"), "{label}");

    // Picking Resolve's weights on the waveform changes the black-and-white Panel too, with nothing else touched.
    b.choose(WAVEFORM, "luma", "match-resolve");
    let matched = trace_and_draw(&mut b, &chart, green);
    assert_drawn_is_traced(green, "green", matched);
    assert_ne!(
        primaries.1, matched.1,
        "the green patch is a different grey with the other weights"
    );
    let label = b.set.labels()[b.index(BLACK_AND_WHITE)].clone();
    assert!(
        label.contains("to match Resolve") && label.contains("0.7152 G"),
        "the label names the weights: {label}"
    );
    assert!(
        b.set.labels()[b.index(WAVEFORM)].contains("0.7152 G"),
        "the waveform names the same weights"
    );
    for number in [1, 9, 20, 22] {
        assert_drawn_is_traced(number, "", trace_and_draw(&mut b, &chart, number));
    }
}
#[test]
fn the_panel_has_no_settings_of_its_own_and_follows_the_waveforms() {
    let b = Bench::new("menu", DWG_REC709);
    assert!(
        b.set.setting_defs(b.index(BLACK_AND_WHITE)).is_empty(),
        "no menu: the weights are the waveform's setting"
    );
    assert!(!b.set.setting_defs(b.index(WAVEFORM)).is_empty());
}

#[test]
fn the_label_says_what_is_shown_and_which_weights_make_the_grey() {
    let mut b = Bench::new("label", DWG_REC709);
    let i = b.index(BLACK_AND_WHITE);
    assert_eq!(b.set.labels()[i], "Black and white");
    b.play(
        64,
        36,
        &ColorChecker::new(TimelineSpace::DavinciIntermediate)
            .frame(64, 36)
            .pixels,
    );
    let label = b.set.labels()[i].clone();
    assert!(label.starts_with("Black and white  ·  Luma  ·  Rec.709  ·  Y = 0.2126 R + 0.7152 G + 0.0722 B from primaries"), "{label}");
}
