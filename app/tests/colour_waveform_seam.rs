//! The Colour Waveform (#59), driven at the Feed seam: synthetic frames go in through the synthetic writer, the
//! app's Feed monitor hands them to the GPU exactly as the app does, and the tests look at what the Panel
//! measured, what it drew and its label. Needs a GPU.
mod common;
use common::{Rig, TARGET, layouts, solid};
use scopecreep::display::{DisplayContext, DisplaySet};
use scopecreep::measurement::Measurement;
use scopecreep::panel_grid::PanelLayout;
use scopecreep::picture_view::PictureView;
use scopecreep::settings::PanelSettings;
use scopecreep::waveform::{PlotLayout, Waveform, WaveformCounts};
use std::sync::Arc;

struct Bench {
    rig: Rig,
    set: DisplaySet,
    luma: Waveform,
    colour: Waveform,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        // The Colour Waveform is out of the Window (#145) but kept and tested: the displays it needs, built here.
        let mut set = DisplaySet::default();
        set.register(Arc::new(PictureView::new(
            &rig.device,
            &rig.queue,
            TARGET,
            false,
        )));
        let luma = Waveform::new(&rig.device, &rig.queue, TARGET);
        set.register(Arc::new(luma.clone()));
        let colour = Waveform::new_colour(&rig.device, &rig.queue, TARGET, &luma);
        set.register(Arc::new(colour.clone()));
        Bench {
            rig,
            set,
            luma,
            colour,
        }
    }

    /// Resolve plays one frame (RGBA, bottom row first) and the app has a moment to take it.
    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let set = &self.set;
        self.rig.play(width, height, pixels, |rig| {
            rig.poll(|encoder, frame| set.on_frame(encoder, frame))
        });
    }

    /// Draw every display into its Panel of a `width` × `height` target, as the window does, and read it back
    /// (RGBA8, rows top first).
    fn render(&self, width: u32, height: u32, scale: f32) -> (Vec<u8>, Vec<PanelLayout>) {
        let panels = layouts(self.set.len(), width, height, scale);
        let image = self.rig.render(width, height, |pass| {
            for (i, panel) in panels.iter().enumerate() {
                self.set
                    .draw(pass, i, panel.content, [width as f32, height as f32], scale);
            }
        });
        (image, panels)
    }
}

const COLOUR: &str = "Colour waveform";
const LUMA: &str = "Luma waveform";

fn index_of(b: &Bench, name: &str) -> usize {
    b.set
        .names()
        .iter()
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("a {name} Panel is registered"))
}

fn counts_of(b: &Bench, name: &str) -> WaveformCounts {
    match name {
        COLOUR => b.colour.counts(),
        LUMA => b.luma.counts(),
        _ => panic!("no waveform called {name}"),
    }
}

fn pixel(image: &[u8], width: u32, x: f32, y: f32) -> [u8; 3] {
    let i = (y as usize * width as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// The code every pixel of the first column landed on (the frame must be one code).
fn the_code(counts: &WaveformCounts, height: u32) -> u32 {
    (0..1024)
        .find(|&code| counts.at(0, code) == height)
        .expect("every pixel of the column is on one code")
}

/// What the Colour waveform drew at `code` of the middle picture column.
fn drawn_at_code(b: &Bench, code: u32) -> [u8; 3] {
    let (image, panels) = b.render(1280, 760, 1.0);
    let layout = PlotLayout::fitted(panels[index_of(b, COLOUR)].content, 1.0, [1920, 1080]);
    pixel(
        &image,
        1280,
        layout.x_of_column(960, 1920).floor(),
        layout.y_of_code(code).floor(),
    )
}

/// A colour's channels against its strongest: the colour without its brightness.
fn proportions([r, g, b]: [u8; 3]) -> [f32; 3] {
    let top = r.max(g).max(b) as f32;
    [r as f32 / top, g as f32 / top, b as f32 / top]
}

#[test]
fn a_frame_of_one_flat_colour_draws_a_trace_of_that_colour_at_its_luma_code() {
    let mut b = Bench::new("flat");
    b.play(1920, 1080, &solid(1920, 1080, [0.2, 0.8, 0.3]));
    let code = the_code(&counts_of(&b, COLOUR), 1080);
    let [r, g, bl] = proportions(drawn_at_code(&b, code));
    assert!(
        (r - 0.25).abs() < 0.05 && (g - 1.0).abs() < 1e-6 && (bl - 0.375).abs() < 0.05,
        "a green trace: {r} {g} {bl}"
    );
    let away = drawn_at_code(&b, code + 80);
    assert!(
        away.iter().all(|&c| c < 40),
        "the rest of the plot stays dark: {away:?}"
    );
}

#[test]
fn two_colours_at_the_same_luma_in_one_column_blend_to_their_average_at_that_spot() {
    let mut b = Bench::new("blend");
    let w = Measurement::default().weights().map(|w| w as f32);
    let a = [0.6f32, 0.3, 0.2];
    let luma = w[0] * a[0] + w[1] * a[1] + w[2] * a[2];
    let (r2, b2) = (0.1f32, 0.5f32);
    let c = [r2, (luma - w[0] * r2 - w[2] * b2) / w[1], b2];
    let two: Vec<f32> = (0..1080u32)
        .flat_map(|row| {
            let p = if row % 2 == 0 { a } else { c };
            (0..1920).flat_map(move |_| [p[0], p[1], p[2], 1.0])
        })
        .collect();
    b.play(1920, 1080, &two);
    let code = the_code(&counts_of(&b, COLOUR), 1080);
    let blended = drawn_at_code(&b, code);
    let average = [
        (a[0] + c[0]) / 2.0,
        (a[1] + c[1]) / 2.0,
        (a[2] + c[2]) / 2.0,
    ];
    b.play(1920, 1080, &solid(1920, 1080, average));
    let flat = drawn_at_code(&b, the_code(&counts_of(&b, COLOUR), 1080));
    for i in 0..3 {
        assert!(
            (blended[i] as i32 - flat[i] as i32).abs() <= 3,
            "blended {blended:?} against the average drawn flat {flat:?}"
        );
    }
}

#[test]
fn the_traces_positions_match_the_luma_waveforms_counts() {
    let mut b = Bench::new("positions");
    let busy: Vec<f32> = (0..1080u32)
        .flat_map(|row| {
            (0..1920u32).flat_map(move |x| {
                [
                    (x % 256) as f32 / 255.0,
                    row as f32 / 1079.0,
                    ((x + row) % 97) as f32 / 96.0,
                    1.0,
                ]
            })
        })
        .collect();
    b.play(1920, 1080, &busy);
    assert_eq!(counts_of(&b, COLOUR), counts_of(&b, LUMA));
}

#[test]
fn trace_brightness_and_soft_spread_change_the_drawing_but_not_the_label() {
    let mut b = Bench::new("settings");
    let i = index_of(&b, COLOUR);
    let keys: Vec<&str> = b.set.setting_defs(i).iter().map(|d| d.key).collect();
    assert_eq!(
        keys,
        [
            "brightness",
            "spread",
            "graticule",
            "scale_units",
            "reference_low",
            "reference_high"
        ]
    );
    let ramp: Vec<f32> = (0..1080u32)
        .flat_map(|row| {
            (0..1920).flat_map(move |_| [row as f32 / 1079.0, 0.3, 1.0 - row as f32 / 1079.0, 1.0])
        })
        .collect();
    b.play(1920, 1080, &ramp);
    let lit = |b: &Bench, settings: &PanelSettings| {
        b.set.configure(&DisplayContext::default(), settings);
        let (image, panels) = b.render(1280, 760, 1.0);
        let c = panels[i].content;
        (0..c[3] as u32)
            .flat_map(|dy| (0..c[2] as u32).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| {
                pixel(&image, 1280, c[0] + dx as f32, c[1] + dy as f32)
                    .iter()
                    .map(|&v| v as u64)
                    .sum::<u64>()
            })
            .sum::<u64>()
    };
    let mut settings = PanelSettings::default();
    let normal = lit(&b, &settings);
    let label = b.set.labels()[i].clone();
    settings.set(COLOUR, "brightness", "brighter");
    let brighter = lit(&b, &settings);
    settings.set(COLOUR, "brightness", "dimmer");
    let dimmer = lit(&b, &settings);
    assert!(
        dimmer < normal && normal < brighter,
        "dimmer {dimmer} < normal {normal} < brighter {brighter}"
    );
    settings.set(COLOUR, "brightness", "normal");
    settings.set(COLOUR, "spread", "on");
    assert_eq!(
        lit(&b, &settings),
        normal,
        "the old list value On draws as the default does"
    );
    settings.set(COLOUR, "spread", "off");
    let off = lit(&b, &settings);
    assert_ne!(off, normal, "the soft spread changes the drawing");
    settings.set(COLOUR, "spread", "0");
    assert_eq!(lit(&b, &settings), off, "0 draws what Off drew");
    settings.set(COLOUR, "spread", "2");
    assert_ne!(
        lit(&b, &settings),
        normal,
        "2 px draws more spread than the default"
    );
    assert_eq!(
        b.set.labels()[i],
        label,
        "drawing settings do not change the label"
    );
}

#[test]
fn the_label_says_what_the_colour_waveform_is_and_names_the_luma_weights() {
    let b = Bench::new("label");
    let label = b.set.labels()[index_of(&b, COLOUR)].clone();
    assert!(label.starts_with("Colour waveform"), "{label}");
    assert!(label.contains("Y = "), "{label}");
}

#[test]
fn graticule_brightness_changes_the_grid_not_the_label_and_its_default_is_today() {
    let mut b = Bench::new("graticule");
    b.play(1920, 1080, &vec![0.0; 1920 * 1080 * 4]);
    let i = index_of(&b, COLOUR);
    let lit = |b: &Bench, value: Option<&str>| {
        let mut settings = PanelSettings::default();
        if let Some(value) = value {
            settings.set(COLOUR, "graticule", value);
        }
        b.set.configure(&DisplayContext::default(), &settings);
        b.render(1280, 760, 1.0).0
    };
    let normal = lit(&b, None);
    let label = b.set.labels()[i].clone();
    assert!(lit(&b, Some("100")) == normal, "the default draws as today");
    let sum = |image: &[u8]| image.iter().map(|&v| v as u64).sum::<u64>();
    let (dim, bright) = (sum(&lit(&b, Some("20"))), sum(&lit(&b, Some("300"))));
    assert!(
        dim < sum(&normal) && sum(&normal) < bright,
        "{dim} < {} < {bright}",
        sum(&normal)
    );
    assert_eq!(
        b.set.labels()[i],
        label,
        "drawing only: the label does not change"
    );
}
