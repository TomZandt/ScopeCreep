//! The Panels' displays, driven at the Feed seam (#50, spec #49 "Testing Decisions"): synthetic frames go in
//! through the synthetic writer, the app's Feed monitor hands them to the GPU exactly as the app does, and the
//! tests look at what each display measured, what it drew and its label. Needs a GPU.
mod common;
use common::{Rig, TARGET, layouts, solid};
use scopecreep::display::{Display, DisplayContext, DisplaySet, ScaleText};
use scopecreep::gpu_frame::GpuFrame;
use scopecreep::colour_setup::{ColourSetup, KnownColourSetup, Levels, Provenance, Science};
use scopecreep::panel_grid::PanelLayout;
use scopecreep::settings::PanelSettings;
use scopecreep::waveform::WaveformCounts;
use std::sync::{Arc, Mutex};


/// A display that only records what it is given: the shape a new display follows.
struct Recorder {
    frames: Mutex<Vec<(u64, u32, u32)>>,
    context: Mutex<Option<DisplayContext>>,
}

impl Display for Recorder {
    fn name(&self) -> &'static str {
        "Recorder"
    }
    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.frames.lock().unwrap().push((frame.texture_id, frame.width, frame.height));
    }
    fn configure(&self, context: &DisplayContext) {
        *self.context.lock().unwrap() = Some(context.clone());
    }
    fn label(&self) -> String {
        "Recorder label".into()
    }
    fn draw(&self, _pass: &mut wgpu::RenderPass<'_>, _area: [f32; 4], _surface: [f32; 2], _scale: f32) {}
}

struct Bench {
    rig: Rig,
    set: DisplaySet,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let set = DisplaySet::standard(&rig.device, &rig.queue, TARGET, false);
        Bench { rig, set }
    }

    /// Resolve plays one frame (RGBA, bottom row first) and the app has a moment to take it.
    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let set = &self.set;
        self.rig.play(width, height, pixels, |rig| rig.poll(|encoder, frame| set.on_frame(encoder, frame)));
    }

    /// Draw every display into its Panel of a `width` × `height` target, as the window does, and read it back
    /// (RGBA8, rows top first).
    fn render(&self, width: u32, height: u32, scale: f32) -> (Vec<u8>, Vec<PanelLayout>) {
        let panels = layouts(self.set.len(), width, height, scale);
        let image = self.rig.render(width, height, |pass| {
            for (i, panel) in panels.iter().enumerate() {
                self.set.draw(pass, i, panel.content, [width as f32, height as f32], scale);
            }
        });
        (image, panels)
    }
}

fn pixel(image: &[u8], width: u32, x: f32, y: f32) -> [u8; 3] {
    let i = (y as usize * width as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

fn middle(rect: [f32; 4]) -> (f32, f32) {
    (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0)
}

#[test]
fn the_window_shows_nine_panels_without_the_gamut_warning_highlight_neutrals_cie_plot_and_black_and_white() {
    let b = Bench::new("order");
    let names = b.set.names();
    assert_eq!(names.len(), 9, "{names:?}");
    assert_eq!(names[..3], ["Clean picture", "Luma waveform", "Channel view"]);
    for gone in ["Gamut/legal warning", "Highlight neutrals", "CIE chromaticity", "Black and white"] {
        assert!(!names.contains(&gone), "{gone} is out of the Window: {names:?}");
    }
}

#[test]
fn settings_of_panels_out_of_the_window_stay_in_the_saved_text() {
    let mut saved = PanelSettings::default();
    saved.set("Gamut/legal warning", "limits", "ebu");
    saved.set("CIE chromaticity", "plot", "x");
    let text = saved.to_text();
    let reloaded = PanelSettings::from_text(&text);
    assert_eq!(reloaded.to_text(), text);
}

#[test]
fn every_display_gets_the_same_viewer_frame_from_one_upload() {
    let mut b = Bench::new("oneupload");
    let (first, second) = (Arc::new(Recorder { frames: Default::default(), context: Default::default() }), Arc::new(Recorder { frames: Default::default(), context: Default::default() }));
    b.set.register(first.clone());
    b.set.register(second.clone());
    b.play(64, 36, &solid(64, 36, [0.5; 3]));
    b.play(64, 36, &solid(64, 36, [0.25; 3]));
    let (a, c) = (first.frames.lock().unwrap().clone(), second.frames.lock().unwrap().clone());
    assert!(a.len() >= 2, "each played frame reached the display: {a:?}");
    assert_eq!(a, c, "both displays saw exactly the same frames");
    assert!(a.iter().all(|f| *f == a[0]), "one texture, uploaded into again, not one per display: {a:?}");
    // The built-in displays took the same frame: the waveform counted every pixel of it.
    let counts = b.set.read_back(1).and_then(|r| r.downcast::<WaveformCounts>().ok()).expect("the waveform reads back");
    assert_eq!(counts.total(), 64 * 36);
}

#[test]
fn a_new_display_is_one_module_registered_and_gets_everything_a_display_gets() {
    let mut b = Bench::new("register");
    let before = b.set.len();
    let recorder = Arc::new(Recorder { frames: Default::default(), context: Default::default() });
    b.set.register(recorder.clone());
    assert_eq!(b.set.len(), before + 1);
    assert_eq!(b.set.names()[before], "Recorder");
    assert_eq!(b.set.labels()[before], "Recorder label");
    let context = DisplayContext { colour_label: "Colour Setup unknown".into(), ..DisplayContext::default() };
    b.set.configure(&context, &PanelSettings::default());
    assert_eq!(recorder.context.lock().unwrap().as_ref().unwrap().colour_label, "Colour Setup unknown");
    assert_eq!(b.set.read_back(before).is_none(), true, "a Picture View has nothing to read back");
    assert_eq!(b.set.scale_texts(before, [0.0, 0.0, 200.0, 200.0], 1.0), Vec::<ScaleText>::new());
}

#[test]
fn each_display_draws_only_inside_its_own_panel() {
    let mut b = Bench::new("draws");
    b.play(1920, 1080, &solid(1920, 1080, [0.5; 3]));
    let (image, panels) = b.render(1280, 400, 1.0);
    // Panel 0: the picture, mid grey (the target is not sRGB, so 0.5 is 128).
    let (x, y) = middle(panels[0].content);
    let [r, g, bl] = pixel(&image, 1280, x, y);
    assert!((r as i32 - 128).abs() <= 1 && r == g && g == bl, "the clean picture is in panel 0: {r} {g} {bl}");
    // Panel 1: the waveform; mid grey is a trace across the middle of its plot.
    let content = panels[1].content;
    let trace = (0..content[3] as u32).map(|dy| pixel(&image, 1280, content[0] + content[2] * 0.75, content[1] + dy as f32)[0]).max().unwrap();
    assert!(trace > 150, "the waveform trace is in panel 1: {trace}");
    // Nothing is drawn in the gap between the Panels, nor in the title bar above each display.
    let gap_x = panels[0].content[0] + panels[0].content[2] + 4.0;
    assert_eq!(pixel(&image, 1280, gap_x, 100.0), [0, 0, 0], "the border between Panels is empty");
    let bar = panels[0].title;
    assert_eq!(pixel(&image, 1280, bar[0] + 10.0, bar[1] + 5.0), [0, 0, 0], "the title bar is left for the label text");
}

#[test]
fn the_displays_draw_at_the_screens_scale() {
    let mut b = Bench::new("hidpi");
    b.play(1920, 1080, &solid(1920, 1080, [0.5; 3]));
    let (image, panels) = b.render(2560, 800, 2.0);
    let content = panels[1].content;
    // The waveform's scale gutter is 44 logical pixels: 88 device pixels at a scale of 2, kept clear of the trace.
    let plot_left = content[0] + 88.0;
    let in_gutter = (0..content[3] as u32).map(|dy| pixel(&image, 2560, content[0] + 40.0, content[1] + dy as f32)[0]).max().unwrap();
    assert!(in_gutter < 100, "no trace in the gutter: {in_gutter}");
    let trace = (0..content[3] as u32).map(|dy| pixel(&image, 2560, plot_left + (content[2] - 88.0) * 0.75, content[1] + dy as f32)[0]).max().unwrap();
    assert!(trace > 150, "the trace is drawn at 2x: {trace}");
}

#[test]
fn labels_say_what_each_display_shows() {
    let mut b = Bench::new("labels");
    assert_eq!(b.set.labels()[0], "Clean picture");
    b.set.configure(&DisplayContext { colour_label: "Colour Setup unknown".into(), ..DisplayContext::default() }, &PanelSettings::default());
    b.play(1920, 1080, &solid(1920, 1080, [0.5; 3]));
    let labels = b.set.labels();
    assert!(labels[0].starts_with("Clean picture  ·  1920 × 1080  ·  Colour Setup unknown"), "{}", labels[0]);
    assert!(labels[1].starts_with("Luma  ·  10-bit"), "{}", labels[1]);
}

/// A project graded and delivered in P3-D65 (Resolve's own waveform weights it as Rec.709, ADR 0004).
fn p3_d65_setup() -> DisplayContext {
    let setup = ColourSetup::new(Some(Science::Yrgb), "P3-D65".into(), "P3-D65".into(), None, None, None, Some(Levels::Video));
    DisplayContext { colour_setup: Some(KnownColourSetup { setup, provenance: Provenance::Project }), colour_label: "P3-D65".into(), ..DisplayContext::default() }
}

/// The 10-bit code every pixel of a one-colour frame lands on, as the waveform's first column counted it.
fn code_of_solid(b: &mut Bench, rgb: [f32; 3]) -> u32 {
    b.play(64, 36, &solid(64, 36, rgb));
    let counts = b.set.read_back(1).and_then(|r| r.downcast::<WaveformCounts>().ok()).expect("the waveform reads back");
    let codes: Vec<u32> = (0..1024).filter(|code| counts.at(0, *code) == 36).collect();
    assert_eq!(codes.len(), 1, "all 36 pixels of the column share one code");
    codes[0]
}

#[test]
fn the_waveform_offers_luma_weights_and_the_first_is_from_primaries() {
    let b = Bench::new("luma-menu");
    let defs = b.set.setting_defs(1);
    assert_eq!(defs.iter().map(|d| d.key).collect::<Vec<_>>(), ["luma", "brightness", "spread", "channels"]);
    assert_eq!(defs[1].choices[0].value, "normal", "trace brightness starts normal");
    assert_eq!(defs[2].choices[0].value, "on", "the soft spread starts on");
    assert_eq!(defs[0].choices.iter().map(|c| c.value).collect::<Vec<_>>(), ["primaries", "match-resolve"]);
    assert_eq!(b.set.setting_defs(0).iter().map(|d| d.key).collect::<Vec<_>>(), ["bw"]);
    assert!(b.set.popover_defs(0).is_empty(), "the clean picture has only the B/W chip, so no menu");
}

#[test]
fn the_luma_weights_setting_changes_the_counts_and_the_label_for_a_coloured_frame() {
    let mut b = Bench::new("luma-setting");
    let name = b.set.name(1);
    let mut settings = PanelSettings::default();
    b.set.configure(&p3_d65_setup(), &settings);
    let from_primaries = code_of_solid(&mut b, [0.0, 1.0, 0.0]);
    assert!(b.set.labels()[1].contains("from primaries"), "{}", b.set.labels()[1]);
    // P3-D65's own green weight is 0.6917; Resolve's waveform uses Rec.709's 0.7152 (video levels: 64 + 876 Y).
    assert_eq!(from_primaries, (64.0_f64 + 876.0 * 0.6917).round() as u32);

    settings.set(name, "luma", "match-resolve");
    b.set.configure(&p3_d65_setup(), &settings);
    let match_resolve = code_of_solid(&mut b, [0.0, 1.0, 0.0]);
    assert!(b.set.labels()[1].contains("to match Resolve"), "{}", b.set.labels()[1]);
    assert!(b.set.labels()[1].contains("0.7152 G"), "the label names the weights in use: {}", b.set.labels()[1]);
    assert!(match_resolve.abs_diff((64.0_f64 + 876.0 * 0.7152).round() as u32) <= 1, "Rec.709 green weight: code {match_resolve}");
    assert_ne!(from_primaries, match_resolve);
}

#[test]
fn every_display_that_shows_luma_follows_the_waveforms_luma_weights() {
    let b = Bench::new("luma-everywhere");
    let waveform = b.set.name(1);
    let following = ["ARRI-style False Colour", "Saturation heatmap", "Vectorscope"];
    let label_of = |b: &Bench, prefix: &str| b.set.labels().into_iter().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no label starts with {prefix}"));
    let mut settings = PanelSettings::default();
    b.set.configure(&p3_d65_setup(), &settings);
    for prefix in following {
        let label = label_of(&b, prefix);
        assert!(label.contains("0.6917 G"), "{prefix} uses P3-D65's own weights by default: {label}");
    }
    settings.set(waveform, "luma", "match-resolve");
    b.set.configure(&p3_d65_setup(), &settings);
    for prefix in following {
        let label = label_of(&b, prefix);
        assert!(label.contains("0.7152 G"), "{prefix} follows the waveform to Resolve's weights: {label}");
        assert!(!label.contains("from primaries"), "{prefix} does not claim weights it is not using: {label}");
    }
    assert!(label_of(&b, "Vectorscope").contains("to match Resolve"));
}

#[test]
fn the_trace_brightness_and_soft_spread_settings_change_the_drawing_but_not_the_label_or_counts() {
    let mut b = Bench::new("look-settings");
    let name = b.set.name(1);
    // A vertical grey ramp: moderate density everywhere, so the trace brightness can show.
    let ramp: Vec<f32> = (0..1080u32).flat_map(|row| (0..1920).flat_map(move |_| [row as f32 / 1079.0, row as f32 / 1079.0, row as f32 / 1079.0, 1.0])).collect();
    b.play(1920, 1080, &ramp);
    let lit = |b: &Bench, settings: &PanelSettings| {
        b.set.configure(&DisplayContext::default(), settings);
        let (image, panels) = b.render(1280, 760, 1.0);
        let c = panels[1].content;
        let sum: u64 = (0..c[3] as u32).flat_map(|dy| (0..c[2] as u32).map(move |dx| (dx, dy))).map(|(dx, dy)| pixel(&image, 1280, c[0] + dx as f32, c[1] + dy as f32)[0] as u64).sum();
        sum
    };
    let mut settings = PanelSettings::default();
    let normal = lit(&b, &settings);
    let label = b.set.labels()[1].clone();
    settings.set(name, "brightness", "brighter");
    let brighter = lit(&b, &settings);
    settings.set(name, "brightness", "dimmer");
    let dimmer = lit(&b, &settings);
    assert!(dimmer < normal && normal < brighter, "dimmer {dimmer} < normal {normal} < brighter {brighter}");
    settings.set(name, "brightness", "normal");
    settings.set(name, "spread", "off");
    let unspread = lit(&b, &settings);
    assert_ne!(unspread, normal, "the soft spread changes the drawing");
    assert_eq!(b.set.labels()[1], label, "drawing settings do not change what the label claims");
}

#[test]
fn a_remembered_choice_applies_in_the_next_session() {
    let first = Bench::new("session-1");
    let name = first.set.name(1);
    let mut settings = PanelSettings::default();
    settings.set(name, "luma", "match-resolve");
    let saved = settings.to_text();
    drop(first);
    let mut second = Bench::new("session-2");
    second.set.configure(&p3_d65_setup(), &PanelSettings::from_text(&saved));
    code_of_solid(&mut second, [0.0, 1.0, 0.0]);
    assert!(second.set.labels()[1].contains("to match Resolve"), "{}", second.set.labels()[1]);
}

#[test]
fn a_choice_that_is_no_longer_offered_falls_back_to_the_default() {
    let mut b = Bench::new("stale-choice");
    let name = b.set.name(1);
    let mut settings = PanelSettings::default();
    settings.set(name, "luma", "something-from-a-later-version");
    b.set.configure(&p3_d65_setup(), &settings);
    code_of_solid(&mut b, [0.0, 1.0, 0.0]);
    assert!(b.set.labels()[1].contains("from primaries"), "{}", b.set.labels()[1]);
}
