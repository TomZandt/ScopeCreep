//! Zoom and pan on every picture Panel in the Window (#167, spec #163), driven at the Feed seam: synthetic frames go
//! in as Resolve's would, each Panel draws into a target and the tests read the pixels back; the wheel, the middle
//! button and the zoom chip are what the window gives a Panel. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::arri_false_colour::ArriFalseColour;
use scopecreep::display::{DisplaySet, Pointer};
use scopecreep::output_conversion::OutputConversion;
use scopecreep::panel_grid::PanelLayout;
use scopecreep::saturation_heatmap::SaturationHeatmap;
use scopecreep::settings::PanelSettings;
use scopecreep::skin_tone_view::SkinTone;
use scopecreep::stops_false_colour::StopsFalseColour;
use scopecreep::window_text::{self, panel_title};
use std::sync::Arc;

const W: u32 = 400;
const H: u32 = 300;
/// The Panel's content: 320 x 240 at (20, 20).
const AREA: [f32; 4] = [20.0, 20.0, 320.0, 240.0];
const SETUP: &str = "timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\n";
const NAMES: [&str; 4] = [
    "ARRI False Colour",
    "Stops False Colour",
    "Skin Tone",
    "Saturation heatmap",
];
/// 640 x 360 in 320 x 240 is Fit 50 % (less the legend strip, where there is one).
const FRAME: [u32; 2] = [640, 360];

struct Bench {
    rig: Rig,
    conversion: OutputConversion,
    set: DisplaySet,
    settings: PanelSettings,
}

fn poll(
    rig: &mut Rig,
    conversion: &mut OutputConversion,
    set: &DisplaySet,
    settings: &PanelSettings,
) {
    rig.poll(|encoder, gpu_frame| {
        let shown = conversion.convert(encoder, gpu_frame);
        set.on_frame(encoder, shown);
        set.on_timeline_frame(encoder, gpu_frame);
    });
    set.configure(&rig.monitor.status().display_context(), settings);
}

impl Bench {
    fn new(name: &str, legend: &str) -> Bench {
        let mut rig = Rig::with_colour_setup(name, Some(SETUP));
        let mut set = DisplaySet::default();
        set.register(Arc::new(ArriFalseColour::new(
            &rig.device,
            &rig.queue,
            TARGET,
        )));
        set.register(Arc::new(StopsFalseColour::new(
            &rig.device,
            &rig.queue,
            TARGET,
        )));
        set.register(Arc::new(SkinTone::new(
            &rig.device,
            &rig.queue,
            TARGET,
            false,
        )));
        set.register(Arc::new(SaturationHeatmap::new(
            &rig.device,
            &rig.queue,
            TARGET,
        )));
        let mut conversion = OutputConversion::new(&rig.device, &rig.queue);
        let mut settings = PanelSettings::default();
        for name in NAMES {
            settings.set(name, "legend", legend);
        }
        poll(&mut rig, &mut conversion, &set, &settings);
        let mut bench = Bench {
            rig,
            conversion,
            set,
            settings,
        };
        bench.play(&stripes());
        bench
    }

    fn play(&mut self, pixels: &[f32]) {
        let (conversion, set, settings) = (&mut self.conversion, &self.set, &self.settings);
        self.rig.play(FRAME[0], FRAME[1], pixels, |rig| {
            poll(rig, conversion, set, settings)
        });
        // Panels that take their frame at draw time hold it from the first draw.
        for i in 0..NAMES.len() {
            self.render(i);
        }
    }

    fn render(&self, index: usize) -> Vec<u8> {
        self.rig.render(W, H, |pass| {
            self.set.draw(pass, index, AREA, [W as f32, H as f32], 1.0)
        })
    }

    fn wheel(&self, index: usize, lines: f32) -> bool {
        self.set
            .pointer(
                index,
                AREA,
                1.0,
                Pointer::Wheel {
                    at: middle(),
                    lines,
                },
            )
            .used
    }

    fn chip(&self, index: usize) -> String {
        self.set.toolbar(index, &self.settings, AREA, 1.0).chips[0]
            .text
            .clone()
    }

    fn wheel_to(&self, index: usize, text: &str) {
        for _ in 0..30 {
            if self.chip(index) == text {
                return;
            }
            self.wheel(index, 1.0);
        }
        panic!(
            "{}: never reached {text}: at {}",
            NAMES[index],
            self.chip(index)
        );
    }
}

fn middle() -> [f32; 2] {
    [AREA[0] + AREA[2] / 2.0, AREA[1] + AREA[3] / 2.0]
}

fn px(image: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * W as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// A frame (bottom row first) whose neighbouring columns differ in luma, saturation and hue: dull grey, then saturated orange.
fn stripes() -> Vec<f32> {
    (0..FRAME[1])
        .flat_map(|_| {
            (0..FRAME[0]).flat_map(|x| {
                if x % 2 == 0 {
                    [0.3, 0.3, 0.3, 1.0]
                } else {
                    [0.9, 0.5, 0.2, 1.0]
                }
            })
        })
        .collect()
}

/// Lengths of the runs of equal values, less the first and last (which the Panel's edge cuts short).
fn runs(values: &[[u8; 3]]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut n = 1;
    for pair in values.windows(2) {
        if pair[0] == pair[1] {
            n += 1;
        } else {
            out.push(n);
            n = 1;
        }
    }
    out.push(n);
    out[1..out.len() - 1].to_vec()
}

fn row(image: &[u8], y: u32) -> Vec<[u8; 3]> {
    (AREA[0] as u32..(AREA[0] + AREA[2]) as u32)
        .map(|x| px(image, x, y))
        .collect()
}

#[test]
fn every_picture_panel_zooms_in_exact_squares_with_the_wheel() {
    let b = Bench::new("panels-zoom-squares", "off");
    for (i, name) in NAMES.iter().enumerate() {
        assert_eq!(
            b.chip(i),
            "Fit 50 %",
            "{name}: the chip reads Fit by default"
        );
        let fit = b.render(i);
        for (text, k) in [("100 %", 1usize), ("200 %", 2), ("400 %", 4)] {
            b.wheel_to(i, text);
            let r = runs(&row(&b.render(i), (AREA[1] + AREA[3] / 2.0) as u32));
            assert!(
                r.len() > 20 && r.iter().all(|n| *n == k),
                "{name} at {text}: runs {r:?}"
            );
        }
        for _ in 0..30 {
            b.wheel(i, -1.0);
        }
        assert_eq!(b.chip(i), "Fit 50 %", "{name}: and back to Fit");
        assert_eq!(b.render(i), fit, "{name}: drawn as before at Fit");
    }
}

#[test]
fn zooming_one_panel_leaves_the_others_at_their_own_zoom() {
    let b = Bench::new("panels-zoom-own", "off");
    b.wheel_to(0, "200 %");
    b.wheel_to(3, "300 %");
    assert_eq!(b.chip(0), "200 %");
    assert_eq!(b.chip(1), "Fit 50 %");
    assert_eq!(b.chip(2), "Fit 50 %");
    assert_eq!(b.chip(3), "300 %");
}

#[test]
fn a_middle_drag_pans_each_picture_panel() {
    let b = Bench::new("panels-zoom-pan", "off");
    for (i, name) in NAMES.iter().enumerate() {
        b.wheel_to(i, "200 %");
        let first = b.render(i);
        let from = middle();
        let pointer = |e| b.set.pointer(i, AREA, 1.0, e).used;
        assert!(pointer(Pointer::MiddleDown(from)), "{name}");
        assert!(
            pointer(Pointer::Move([from[0] - 41.0, from[1] - 10.0])),
            "{name}"
        );
        assert!(pointer(Pointer::MiddleUp), "{name}");
        assert_ne!(b.render(i), first, "{name}: the picture moved");
    }
}

#[test]
fn zoomed_the_mattes_stay_black_and_nothing_is_drawn_outside_the_panel() {
    let mut b = Bench::new("panels-zoom-mattes", "off");
    // Black bars across the top and bottom 45 rows of the frame (bottom row first).
    let barred: Vec<f32> = (0..FRAME[1])
        .flat_map(|y| {
            (0..FRAME[0]).flat_map(move |_| {
                if !(45..315).contains(&y) {
                    [0.0, 0.0, 0.0, 1.0]
                } else {
                    [0.9, 0.5, 0.2, 1.0]
                }
            })
        })
        .collect();
    b.play(&barred);
    for (i, name) in NAMES.iter().enumerate() {
        b.wheel_to(i, "100 %");
        // At 100 % the Panel shows 240 of the frame's 360 rows: pan down to the frame's top edge, where its bar is.
        let from = middle();
        let pointer = |e| b.set.pointer(i, AREA, 1.0, e).used;
        pointer(Pointer::MiddleDown(from));
        pointer(Pointer::Move([from[0], from[1] + 300.0]));
        pointer(Pointer::MiddleUp);
        let image = b.render(i);
        // Held at the top edge: the frame's first 45 rows (bar) fill the top 45 px of the Panel.
        assert_eq!(px(&image, 100, 25), [0, 0, 0], "{name}: the bar is black");
        assert_eq!(px(&image, 100, 60), [0, 0, 0], "{name}: still the bar");
        assert_ne!(
            px(&image, 100, 140),
            [0, 0, 0],
            "{name}: the picture is not"
        );
        for (x, y) in [
            (10, 150),
            (345, 150),
            (150, 10),
            (150, 265),
            (2, 2),
            (390, 290),
        ] {
            assert_eq!(
                px(&image, x, y),
                [0, 0, 0],
                "{name}: outside the Panel at {x}, {y}"
            );
        }
    }
}

#[test]
fn zoomed_the_legend_strip_is_unchanged() {
    let b = Bench::new("panels-zoom-legend", "on");
    // The strip under each picture: the bar's tick numbers, the bar and (ARRI only) the line of marks.
    let heights = [
        (0usize, scopecreep::arri_false_colour::BAR.strip(1.0)),
        (
            3,
            scopecreep::scale_bar::ScaleBar {
                from: 0.0,
                to: 1.0,
                unit: "",
                steps: &[],
                marks: &[],
            }
            .strip(1.0),
        ),
    ];
    for (i, height) in heights {
        let top = (AREA[1] + AREA[3] - height).ceil() as u32;
        let strip = |image: &[u8]| -> Vec<[u8; 3]> {
            (top..(AREA[1] + AREA[3]) as u32)
                .flat_map(|y| (20..340).map(move |x| px(image, x, y)))
                .collect()
        };
        let fit = strip(&b.render(i));
        assert!(
            fit.iter().any(|p| *p != [0, 0, 0]),
            "{}: the legend bar is drawn",
            NAMES[i]
        );
        b.wheel_to(i, "200 %");
        assert_eq!(
            strip(&b.render(i)),
            fit,
            "{}: the legend strip is as at Fit",
            NAMES[i]
        );
    }
}

// ---- the toolbar seam

#[test]
fn every_picture_panel_has_a_zoom_chip_before_its_own_chips_that_reads_fit_and_survives_a_small_panel()
 {
    let b = Bench::new("panels-zoom-chips", "off");
    for (i, name) in NAMES.iter().enumerate() {
        let toolbar = b.set.toolbar(i, &b.settings, AREA, 1.0);
        assert_eq!(toolbar.chips[0].text, "Fit 50 %", "{name}");
        assert_eq!(
            toolbar.click(0),
            Some((scopecreep::zoom::ZOOM_KEY, "")),
            "{name}: a click asks for Fit"
        );
        assert!(
            toolbar.chips.len() > 1 || i == 1 || i == 3 || name == &"Skin Tone",
            "{name}: beside its own chips"
        );
        let title = panel_title(name, &Default::default(), String::new());
        for scale in [1.0f32, 2.0] {
            let small = PanelLayout::new([0.0, 0.0, 190.0 * scale, 200.0 * scale], scale).title;
            let mut t = b.set.toolbar(i, &b.settings, AREA, scale);
            t.fit_to(small[2], true, scale);
            let (l, _, _) =
                window_text::lay_out_toolbar_title(&title, &t.chips, true, small, scale);
            let chip = l.chips[0];
            assert!(
                chip[0] >= small[0] && chip[0] + chip[2] <= small[0] + small[2] + 0.01,
                "{name}: {chip:?} stays inside {small:?}"
            );
        }
    }
}

#[test]
fn at_the_minimum_panel_width_every_picture_panel_keeps_its_chips_inside_and_the_rest_in_its_popover()
 {
    let b = Bench::new("panels-zoom-min-width", "off");
    for (i, name) in NAMES.iter().enumerate() {
        let title = panel_title(name, &Default::default(), String::new());
        for scale in [1.0f32, 2.0] {
            let small = PanelLayout::new(
                [
                    0.0,
                    0.0,
                    scopecreep::style::SPACE_MIN_PANEL_WIDTH * scale,
                    200.0 * scale,
                ],
                scale,
            )
            .title;
            let mut t = b.set.toolbar(i, &b.settings, AREA, scale);
            t.fit_to(small[2], !b.set.popover_defs(i).is_empty(), scale);
            let popover: Vec<&str> = b.set.popover_for(i, &t).iter().map(|d| d.key).collect();
            let (l, _, _) = window_text::lay_out_toolbar_title(
                &title,
                &t.chips,
                !popover.is_empty(),
                small,
                scale,
            );
            let inside =
                |r: [f32; 4]| r[0] >= small[0] && r[0] + r[2] <= small[0] + small[2] + 0.01;
            assert_eq!(
                t.chips[0].text, "50 %",
                "{name}: the zoom chip stays, short"
            );
            for chip in l.chips.iter().chain(&l.settings) {
                assert!(inside(*chip), "{name}: {chip:?} stays inside {small:?}");
            }
            if let (Some(last), Some(button)) = (l.chips.last(), l.settings) {
                assert!(
                    last[0] + last[2] <= button[0],
                    "{name}: no chip runs under the settings button"
                );
            }
            for key in t
                .collapsed
                .iter()
                .filter(|k| **k != scopecreep::zoom::ZOOM_KEY)
            {
                assert!(popover.contains(key), "{name}: {key} is in the popover");
            }
        }
    }
}

#[test]
fn clicking_the_zoom_chip_takes_each_picture_panel_back_to_fit_and_a_scope_has_none() {
    let b = Bench::new("panels-zoom-slider", "off");
    for (i, name) in NAMES.iter().enumerate() {
        b.wheel_to(i, "200 %");
        assert_eq!(b.chip(i), "200 %", "{name}");
        b.set.zoom_to_fit(i, AREA, 1.0);
        assert_eq!(b.chip(i), "Fit 50 %", "{name}");
    }
    // The rig holds the GPU, so the Panels' session ends before the Scope's starts.
    drop(b);
    let rig = Rig::new("panels-zoom-scope");
    let (set, _) = DisplaySet::standard_with_handles(&rig.device, &rig.queue, TARGET, false);
    let waveform = (0..set.len())
        .find(|i| set.name(*i) == "Luma waveform")
        .unwrap();
    assert!(set.zoom(waveform, AREA, 1.0).is_none());
}
