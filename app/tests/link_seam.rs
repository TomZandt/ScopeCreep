//! Link on the picture Panels (#168, spec #163): the Feed seam (synthetic frames in, each Panel draws into a target
//! and the tests read the pixels back), the Toolbar seam (the link chip) and the Settings seam (Link is remembered,
//! zoom is not). Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::arri_false_colour::ArriFalseColour;
use scopecreep::display::{DisplaySet, Pointer};
use scopecreep::output_conversion::OutputConversion;
use scopecreep::saturation_heatmap::SaturationHeatmap;
use scopecreep::settings::PanelSettings;
use scopecreep::skin_tone_view::SkinTone;
use scopecreep::stops_false_colour::StopsFalseColour;
use scopecreep::zoom::{LINK_KEY, link_remembered};
use std::sync::Arc;

const W: u32 = 400;
const H: u32 = 300;
const SETUP: &str = "timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\n";
const NAMES: [&str; 4] = [
    "ARRI False Colour",
    "Stops False Colour",
    "Skin Tone",
    "Saturation heatmap",
];
/// Panels of different sizes: 640 x 360 is Fit 50 % in the first and Fit 31 % in the last.
const AREAS: [[f32; 4]; 4] = [
    [20.0, 20.0, 320.0, 240.0],
    [20.0, 20.0, 260.0, 200.0],
    [20.0, 20.0, 200.0, 160.0],
    [20.0, 20.0, 300.0, 180.0],
];
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
    fn new(name: &str) -> Bench {
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
            settings.set(name, "legend", "off");
        }
        poll(&mut rig, &mut conversion, &set, &settings);
        // As the window does on opening: Link as remembered (on when nothing is saved).
        set.set_link(link_remembered(&settings), 0);
        let mut bench = Bench {
            rig,
            conversion,
            set,
            settings,
        };
        bench.play(&split());
        bench
    }

    fn play(&mut self, pixels: &[f32]) {
        let (conversion, set, settings) = (&mut self.conversion, &self.set, &self.settings);
        self.rig.play(FRAME[0], FRAME[1], pixels, |rig| {
            poll(rig, conversion, set, settings)
        });
        for i in 0..NAMES.len() {
            self.render(i);
        }
    }

    fn render(&self, index: usize) -> Vec<u8> {
        self.rig.render(W, H, |pass| {
            self.set
                .draw(pass, index, AREAS[index], [W as f32, H as f32], 1.0)
        })
    }

    fn wheel(&self, index: usize, lines: f32) {
        let a = AREAS[index];
        self.set.pointer(
            index,
            a,
            1.0,
            Pointer::Wheel {
                at: [a[0] + a[2] / 2.0, a[1] + a[3] / 2.0],
                lines,
            },
        );
    }

    /// Wheel one notch at a time until the chip reads `text`.
    fn wheel_to(&self, index: usize, text: &str) {
        for _ in 0..30 {
            if self.text(index) == text {
                return;
            }
            self.wheel(index, 1.0);
        }
        panic!("never reached {text}: at {}", self.text(index));
    }

    fn drag(&self, index: usize, dx: f32) {
        let a = AREAS[index];
        let from = [a[0] + a[2] / 2.0, a[1] + a[3] / 2.0];
        self.set.pointer(index, a, 1.0, Pointer::MiddleDown(from));
        self.set
            .pointer(index, a, 1.0, Pointer::Move([from[0] + dx, from[1]]));
        self.set.pointer(index, a, 1.0, Pointer::MiddleUp);
    }

    fn chips(&self, index: usize) -> scopecreep::toolbar::Toolbar {
        self.set.toolbar(index, &self.settings, AREAS[index], 1.0)
    }

    fn text(&self, index: usize) -> String {
        self.chips(index).chips[0].text.clone()
    }

    /// Click the link chip of Panel `index`, as the window does.
    fn click_link(&self, index: usize) {
        let toolbar = self.chips(index);
        let (key, value) = toolbar.click(1).map(|(k, v)| (k, v.to_string())).unwrap();
        assert_eq!(key, LINK_KEY);
        self.set.set_link(value == "on", index);
    }
}

/// Dark on the left half, bright on the right half: the split is at the middle of the frame.
fn split() -> Vec<f32> {
    (0..FRAME[1])
        .flat_map(|_| {
            (0..FRAME[0]).flat_map(|x| {
                if x < FRAME[0] / 2 {
                    [0.2, 0.2, 0.2, 1.0]
                } else {
                    [0.9, 0.9, 0.9, 1.0]
                }
            })
        })
        .collect()
}

/// The device x, inside the Panel, where the picture changes along its middle row.
fn edge(image: &[u8], area: [f32; 4]) -> f32 {
    let y = (area[1] + area[3] / 2.0) as usize;
    let at = |x: usize| {
        let i = (y * W as usize + x) * 4;
        [image[i], image[i + 1], image[i + 2]]
    };
    let from = area[0] as usize + 1;
    let to = (area[0] + area[2]) as usize - 1;
    let found = (from + 1..to)
        .find(|x| at(*x) != at(*x - 1))
        .expect("an edge in the Panel");
    found as f32
}

#[test]
fn with_link_on_zooming_or_panning_one_panel_draws_the_same_pixels_in_every_panel() {
    let b = Bench::new("link-feed");
    assert!(b.set.link_on(), "a first session opens with Link on");
    for (i, name) in NAMES.iter().enumerate() {
        assert!(
            b.text(i).starts_with("Fit"),
            "{name}: Link on from Fit stays at Fit"
        );
    }
    // Zoom on the last Panel, the smallest at Fit, and pan on another.
    b.wheel_to(3, "200 %");
    b.drag(1, 30.0);
    let texts: Vec<_> = (0..4).map(|i| b.text(i)).collect();
    assert!(
        texts.iter().all(|t| *t == "200 %"),
        "every Panel at the same zoom: {texts:?}"
    );
    // The split of the frame sits as far right of each Panel's middle as the pan moved it (a pixel or two of
    // rounding from the Panel the zoom started in), and the same in every Panel: the same pixels.
    let offsets: Vec<f32> = (0..4)
        .map(|i| edge(&b.render(i), AREAS[i]) - (AREAS[i][0] + AREAS[i][2] / 2.0))
        .collect();
    assert!(
        offsets.iter().all(|o| (o - 30.0).abs() <= 3.0),
        "pan {offsets:?}"
    );
    assert!(
        offsets.iter().all(|o| (o - offsets[0]).abs() <= 1.0),
        "same spot in every Panel: {offsets:?}"
    );
    // Zoom out: each fits its own Panel.
    b.wheel(0, -30.0);
    assert_eq!(
        (b.text(0).as_str(), b.text(3).as_str()),
        ("Fit 50 %", "Fit 47 %")
    );
}

#[test]
fn turning_link_on_takes_the_clicked_panels_view_and_off_keeps_every_view() {
    let b = Bench::new("link-feed-onoff");
    b.click_link(0);
    assert!(!b.set.link_on());
    b.wheel_to(2, "200 %");
    assert_eq!(
        (b.text(0).as_str(), b.text(2).as_str()),
        ("Fit 50 %", "200 %")
    );
    b.drag(2, 20.0);
    b.click_link(2);
    for i in 0..4 {
        assert_eq!(b.text(i), "200 %");
        let a = AREAS[i];
        let offset = edge(&b.render(i), a) - (a[0] + a[2] / 2.0);
        assert!(
            (offset - 20.0).abs() <= 1.0,
            "Panel {i}: on the clicked Panel's spot ({offset})"
        );
    }
    b.click_link(0);
    assert!(!b.set.link_on());
    let before: Vec<_> = (0..4).map(|i| b.render(i)).collect();
    assert_eq!(b.text(1), "200 %", "off leaves each Panel where it is");
    b.wheel(1, -30.0);
    assert_eq!(
        (b.text(1).as_str(), b.text(0).as_str()),
        ("Fit 41 %", "200 %"),
        "then they move apart"
    );
    assert_eq!(b.render(0), before[0]);
}

#[test]
fn with_link_on_clicking_the_zoom_chip_takes_every_panel_back_to_fit() {
    let b = Bench::new("link-fit-click");
    b.wheel_to(3, "200 %");
    assert!((0..4).all(|i| b.text(i) == "200 %"));
    b.set.zoom_to_fit(1, AREAS[1], 1.0);
    for i in 0..4 {
        assert!(b.text(i).starts_with("Fit"), "Panel {i}: {}", b.text(i));
    }
}

// ---- the Toolbar seam

#[test]
fn the_link_chip_shows_on_every_picture_panel_lit_by_default_and_goes_out_on_all_of_them_at_once() {
    let b = Bench::new("link-toolbar");
    for (i, name) in NAMES.iter().enumerate() {
        let t = b.chips(i);
        assert_eq!(t.chips[1].text, "Link", "{name}: beside the zoom chip");
        assert!(t.on[1], "{name}: lit by default");
        assert_eq!(
            t.click(1),
            Some((LINK_KEY, "off")),
            "{name}: a click turns Link off"
        );
    }
    b.click_link(2);
    for (i, name) in NAMES.iter().enumerate() {
        let t = b.chips(i);
        assert!(!t.on[1], "{name}: unlit with Link off");
        assert_eq!(
            t.click(1),
            Some((LINK_KEY, "on")),
            "{name}: a click turns it on"
        );
    }
    b.click_link(3);
    assert!((0..4).all(|i| b.chips(i).on[1]));
}

// ---- the Settings seam

#[test]
fn link_is_remembered_and_zoom_is_not() {
    let mut s = PanelSettings::default();
    assert!(link_remembered(&s), "nothing saved: on");
    s.set_state(scopecreep::zoom::LINK_STATE_PANEL, LINK_KEY, "on");
    let back = PanelSettings::from_text(&s.to_text());
    assert!(link_remembered(&back), "on round-trips");
    assert!(s.to_text().contains("state:link"));
    s.set_state(scopecreep::zoom::LINK_STATE_PANEL, LINK_KEY, "off");
    assert!(
        !link_remembered(&PanelSettings::from_text(&s.to_text())),
        "off round-trips"
    );
    s.set_state(scopecreep::zoom::LINK_STATE_PANEL, LINK_KEY, "off");
    assert!(!link_remembered(&s), "saved off stays off");
    for rubbish in ["rubbish", "ON", "1", "true", "on "] {
        let mut s = PanelSettings::default();
        s.set_state(scopecreep::zoom::LINK_STATE_PANEL, LINK_KEY, rubbish);
        assert!(
            link_remembered(&PanelSettings::from_text(&s.to_text())),
            "{rubbish:?} falls back to on"
        );
    }
}
