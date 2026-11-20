//! Mattes stay black (#102, ADR 0029), proven at the Feed seam: letterboxed and pillarboxed synthetic frames go in
//! through the synthetic writer, the app's Feed monitor and Output Conversion hand them to the displays as the
//! app does, and the Panels that colour the picture are drawn and read back. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::arri_false_colour::ArriFalseColour;
use scopecreep::display::DisplaySet;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::saturation_heatmap::SaturationHeatmap;
use scopecreep::settings::PanelSettings;
use scopecreep::skin_tone_view::SkinTone;
use scopecreep::stops_false_colour::StopsFalseColour;
use std::sync::Arc;

const W: u32 = 160;
const H: u32 = 90;
const SETUP: &str = "timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\n";
const NAMES: [&str; 4] = [
    "ARRI False Colour",
    "Stops False Colour",
    "Skin Tone",
    "Saturation heatmap",
];
/// A picture colour that is not neutral and not black, so every Panel colours it.
const PICTURE: [f32; 3] = [0.5, 0.35, 0.3];

struct Bench {
    rig: Rig,
    conversion: OutputConversion,
    set: DisplaySet,
    settings: PanelSettings,
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
        Bench {
            rig,
            conversion,
            set,
            settings,
        }
    }

    /// Resolve plays a frame given top row first.
    fn play(&mut self, rows: &[Vec<[f32; 3]>]) {
        let bottom_first: Vec<f32> = rows
            .iter()
            .rev()
            .flat_map(|row| row.iter().flat_map(|p| [p[0], p[1], p[2], 1.0]))
            .collect();
        let (conversion, set, settings) = (&mut self.conversion, &self.set, &self.settings);
        self.rig.play(W, H, &bottom_first, |rig| {
            poll(rig, conversion, set, settings)
        });
    }

    fn render(&self, index: usize) -> Vec<u8> {
        self.rig.render(W, H, |pass| {
            self.set.draw(
                pass,
                index,
                [0.0, 0.0, W as f32, H as f32],
                [W as f32, H as f32],
                1.0,
            )
        })
    }
}

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
        set.on_timeline_frame(encoder, gpu_frame);
    });
    set.configure(&rig.monitor.status().display_context(), settings);
}

fn pixel(image: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * W as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// A picture frame with black bars: rows `top` from the top and `bottom` from the bottom, columns `left` and `right`.
fn framed(top: u32, bottom: u32, left: u32, right: u32) -> Vec<Vec<[f32; 3]>> {
    (0..H)
        .map(|y| {
            (0..W)
                .map(|x| {
                    if y < top || y >= H - bottom || x < left || x >= W - right {
                        [0.0; 3]
                    } else {
                        PICTURE
                    }
                })
                .collect()
        })
        .collect()
}

fn all_black(image: &[u8], x0: u32, y0: u32, x1: u32, y1: u32) -> bool {
    (y0..y1).all(|y| (x0..x1).all(|x| pixel(image, x, y) == [0, 0, 0]))
}

#[test]
fn a_letterboxed_frame_has_black_bars_and_a_coloured_picture_on_every_colouring_panel() {
    let mut b = Bench::new("matte-letterbox");
    b.play(&framed(12, 6, 0, 0));
    for (i, name) in NAMES.iter().enumerate() {
        let image = b.render(i);
        assert!(
            all_black(&image, 0, 0, W, 12),
            "{name}: the top bar is black"
        );
        assert!(
            all_black(&image, 0, H - 6, W, H),
            "{name}: the bottom bar is black"
        );
        assert!(
            !all_black(&image, 0, 12, W, H - 6),
            "{name}: the picture is not black"
        );
        assert_ne!(
            pixel(&image, W / 2, 12),
            [0, 0, 0],
            "{name}: the picture starts at the bar's edge"
        );
        assert_ne!(
            pixel(&image, W / 2, H - 7),
            [0, 0, 0],
            "{name}: and runs to the bar's edge"
        );
    }
}

#[test]
fn a_pillarboxed_frame_has_black_bars_and_a_coloured_picture_on_every_colouring_panel() {
    let mut b = Bench::new("matte-pillarbox");
    b.play(&framed(0, 0, 20, 30));
    for (i, name) in NAMES.iter().enumerate() {
        let image = b.render(i);
        assert!(
            all_black(&image, 0, 0, 20, H),
            "{name}: the left bar is black"
        );
        assert!(
            all_black(&image, W - 30, 0, W, H),
            "{name}: the right bar is black"
        );
        assert_ne!(
            pixel(&image, 20, H / 2),
            [0, 0, 0],
            "{name}: the picture starts at the bar's edge"
        );
        assert_ne!(
            pixel(&image, W - 31, H / 2),
            [0, 0, 0],
            "{name}: and runs to the bar's edge"
        );
    }
}

#[test]
fn a_crushed_black_area_inside_the_picture_is_still_coloured() {
    let mut b = Bench::new("matte-crushed");
    let mut rows = framed(12, 12, 0, 0);
    for row in rows.iter_mut().take(60).skip(40) {
        for p in row.iter_mut() {
            *p = [0.0; 3];
        }
    }
    b.play(&rows);
    // ARRI False Colour is the Panel that paints black in a colour of its own (purple): inside the picture it
    // still does, while the bars at the edge are black.
    let image = b.render(0);
    assert_ne!(
        pixel(&image, W / 2, 50),
        [0, 0, 0],
        "black inside the picture is coloured"
    );
    assert!(
        all_black(&image, 0, 0, W, 12) && all_black(&image, 0, H - 12, W, H),
        "the bars are black"
    );
}

#[test]
fn a_frame_with_no_bars_is_drawn_whole_and_an_all_black_frame_is_not_a_matte() {
    let mut b = Bench::new("matte-none");
    b.play(&framed(0, 0, 0, 0));
    let image = b.render(0);
    assert!(
        !all_black(&image, 0, 0, W, 1)
            && !all_black(&image, 0, H - 1, W, H)
            && !all_black(&image, 0, 0, 1, H)
            && !all_black(&image, W - 1, 0, W, H)
    );
    b.play(&framed(H, 0, 0, 0));
    assert_ne!(
        pixel(&b.render(0), W / 2, H / 2),
        [0, 0, 0],
        "ARRI False Colour still colours an all black frame"
    );
}
