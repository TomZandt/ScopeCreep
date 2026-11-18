//! The saturation heatmap (#64), driven at the Feed seam: synthetic frames go in through the synthetic writer, the app's
//! Feed monitor hands them to the GPU as the app does, and the tests read back what the Panel drew, its label and
//! its legend. Expected values come from the BT.709 constants and the research table
//! through the ColorChecker frames of #52. Needs a GPU.
mod common;
use common::{Rig, TARGET, solid};
use scopecreep::muted_palette::{FULL_STRENGTH_AT, STOPS, heat, overlay, strength};
use scopecreep::saturation_heatmap::SaturationHeatmap;
use scopecreep::display::{DisplayContext, DisplaySet};
use scopecreep::settings::PanelSettings;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_accuracy::transfer::bt709_oetf;
use std::sync::Arc;


struct Bench {
    rig: Rig,
    set: DisplaySet,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let mut set = DisplaySet::default();
        set.register(Arc::new(SaturationHeatmap::new(&rig.device, &rig.queue, TARGET)));
        Bench { rig, set }
    }

    fn configure(&self, legend: &str, scale: &str) {
        let mut settings = PanelSettings::default();
        settings.set(self.set.name(0), "legend", legend);
        settings.set(self.set.name(0), "scale", scale);
        self.set.configure(&DisplayContext::default(), &settings);
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let set = &self.set;
        self.rig.play(width, height, pixels, |rig| rig.poll(|encoder, frame| set.on_frame(encoder, frame)));
    }

    /// Draw the Panel into a `width` × `height` target at scale 1 and read it back (RGBA8, rows top first).
    fn render(&self, width: u32, height: u32) -> Vec<u8> {
        self.rig.render(width, height, |pass| self.set.draw(pass, 0, [0.0, 0.0, width as f32, height as f32], [width as f32, height as f32], 1.0))
    }
}

fn pixel(image: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * width as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// A frame (RGBA, bottom row first) of one solid colour.
/// BT.709 Cb and Cr from the standard's own constants (1.8556, 1.5748), and the distance from neutral as a share
/// of the vectorscope's 100 % ring (Cb/Cr 0.6).
fn percent(rgb: [f64; 3]) -> f64 {
    let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let (cb, cr) = ((rgb[2] - y) / 1.8556, (rgb[0] - y) / 1.5748);
    cb.hypot(cr) / scopecreep::vectorscope::RING * 100.0
}

/// The Luma (BT.709 weights) of an encoded colour.
fn luma(rgb: [f64; 3]) -> f32 {
    (0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]) as f32
}

/// The picture's pixel is its Luma in grey with the heat colour laid over it at `t` on the scale.
fn assert_overlay(got: [u8; 3], grey: f32, t: f32, what: &str) {
    let want = overlay(grey, t);
    assert!((0..3).all(|c| (got[c] as f32 - want[c] * 255.0).abs() <= 2.0), "{what}: {got:?} is not the grey {grey:.3} with heat at {t:.3} ({:?})", want.map(|v| v * 255.0));
}

/// The encoded Output picture of a patch of the research table: linear Rec.709 through the BT.709 OETF, mirrored
/// about zero for a channel below 0 (cyan's red).
fn encoded(linear: [f64; 3]) -> [f64; 3] {
    linear.map(|l| l.signum() * bt709_oetf(l.abs()))
}

/// Patch number, name, encoded colour and drawn colour (the middle of the patch) for the ColorChecker as the
/// Output picture.
fn chart_patches(b: &mut Bench) -> Vec<(usize, &'static str, [f64; 3], [u8; 3])> {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    let layout = chart.frame(width, height);
    let mut pixels = vec![0.0f32; layout.pixels.len()];
    for patch in chart.patches() {
        let r = layout.patch_rect(patch.number);
        let rgb = encoded(patch.rec709_linear);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let i = (((height - 1 - y) * width + x) * 4) as usize; // the frame's bottom row is first
                pixels[i..i + 4].copy_from_slice(&[rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0]);
            }
        }
    }
    b.play(width, height, &pixels);
    let image = b.render(width, height);
    chart
        .patches()
        .iter()
        .map(|patch| {
            let r = layout.patch_rect(patch.number);
            (patch.number, patch.name, encoded(patch.rec709_linear), pixel(&image, width, (r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2))
        })
        .collect()
}

#[test]
fn the_palette_is_cool_to_hot_and_the_colour_strengthens_over_the_grey() {
    assert_eq!(heat(0.0), STOPS[0]);
    assert!(heat(1.0).iter().zip(STOPS[STOPS.len() - 1]).all(|(a, b)| (a - b).abs() < 1e-5));
    assert_eq!(heat(-1.0), heat(0.0));
    assert_eq!(heat(9.0), heat(1.0));
    // Cool is blue and hot is red.
    assert!(STOPS[0][2] > STOPS[0][0] && STOPS[STOPS.len() - 1][0] > STOPS[STOPS.len() - 1][2]);
    // Nothing at 0, all of it from the full-strength point, rising in between.
    assert_eq!(strength(0.0), 0.0);
    assert_eq!(strength(FULL_STRENGTH_AT), 1.0);
    assert_eq!(strength(1.0), 1.0);
    assert!(strength(FULL_STRENGTH_AT / 2.0) > 0.0 && strength(FULL_STRENGTH_AT / 2.0) < 1.0);
    // Neutral shows the grey; saturated shows the colour.
    assert_eq!(overlay(0.4, 0.0), [0.4; 3]);
    assert!(overlay(0.4, 1.0).iter().zip(heat(1.0)).all(|(a, b)| (a - b).abs() < 1e-5));
    // The ramp is chosen for contrast: its stops differ a lot in brightness, not the even-brightness rule.
    let lumas: Vec<f32> = STOPS.iter().map(|s| luma(s.map(|v| v as f64))).collect();
    let (lo, hi) = lumas.iter().fold((1.0f32, 0.0f32), |(lo, hi), l| (lo.min(*l), hi.max(*l)));
    assert!(hi - lo > 0.2, "{lumas:?}");
}

#[test]
fn the_display_is_registered_in_the_standard_set() {
    let rig = Rig::new("registered");
    let (device, queue) = (&rig.device, &rig.queue);
    let set = DisplaySet::standard(device, queue, TARGET, false);
    assert!(set.names().contains(&"Saturation heatmap"), "{:?}", set.names());
}

#[test]
fn neutral_patches_show_as_their_luma_grey() {
    let mut b = Bench::new("greys");
    b.configure("off", "100");
    let patches = chart_patches(&mut b);
    for (number, name, rgb, got) in patches.iter().filter(|p| p.0 >= 19) {
        // The chart's greys are neutral to within the research table's rounding, so the colour laid over them is
        // faint: the pixel is its Luma grey to within 8/255.
        let grey = luma(*rgb) * 255.0;
        assert!((0..3).all(|c| (got[c] as f32 - grey).abs() <= 8.0), "patch {number} {name}: {got:?} is not the grey {grey:.1}");
    }
}

#[test]
fn colour_bars_at_100_percent_sit_at_the_100_percent_point_and_75_percent_bars_below_it_in_strong_colour() {
    let mut b = Bench::new("bars");
    b.configure("off", "100");
    for level in [1.0f64, 0.75] {
        let colours = [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]].map(|c: [f64; 3]| c.map(|v| v * level));
        let (w, h) = (60u32, 36u32);
        let pixels: Vec<f32> = (0..w * h).flat_map(|i| colours[(i % w / 10) as usize].map(|v| v as f32).into_iter().chain([1.0])).collect();
        b.play(w, h, &pixels);
        let image = b.render(w, h);
        for (i, c) in colours.iter().enumerate() {
            let got = pixel(&image, w, i as u32 * 10 + 5, 18);
            let at = percent(*c);
            // Bars scale with their level, and 100 % bars are within a fifth of the 100 % point (the ring is at the
            // furthest bar, so green and magenta are at 99 % and red, yellow, cyan and blue at 84 to 86 %).

            assert!((at / level - 100.0).abs() <= 20.0, "bar {i} at {at:.1} %");
            assert_overlay(got, luma(*c), (at / 100.0).min(1.0) as f32, &format!("{level} bar {i}"));
        }
    }
}

#[test]
fn each_colorchecker_patch_shows_its_grey_with_its_saturation_laid_over_it() {
    let mut b = Bench::new("patches");
    b.configure("off", "100");
    for (number, name, rgb, got) in chart_patches(&mut b) {
        let want = (percent(rgb) / 100.0).min(1.0) as f32;
        assert_overlay(got, luma(rgb), want, &format!("patch {number} {name}"));
    }
}

#[test]
fn the_default_scale_lets_a_graded_picture_use_most_of_it() {
    // Measured on the ColorChecker as the Output picture: the most saturated of the 18 colour patches uses at
    // least 70 % of the default scale and none is past it.
    let b = Bench::new("default");
    let default: f64 = b.set.setting_defs(0)[0].choices[0].value.parse().unwrap();
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    let mut shares: Vec<f64> = chart.patches().iter().filter(|p| p.number < 19).map(|p| percent(encoded(p.rec709_linear)) / default).collect();
    shares.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eprintln!("default {default} %, shares of the scale: {shares:.3?}");
    let most = shares[shares.len() - 1];
    assert!(most <= 1.0 && most >= 0.7 && default == 75.0, "most saturated patch uses {most:.2} of the scale");
}

#[test]
fn changing_the_scale_remaps_the_colours() {
    let mut b = Bench::new("scale");
    // An orange, about 36 % saturated.
    let rgb = [0.75f64, 0.5, 0.25];
    let at = percent(rgb);
    assert!(at > 20.0 && at < 50.0, "{at}");
    b.play(40, 40, &solid(40, 40, rgb.map(|v| v as f32)));
    for scale in ["100", "50", "25", "200"] {
        b.configure("off", scale);
        let image = b.render(40, 40);
        let want = (at / scale.parse::<f64>().unwrap()).min(1.0) as f32;
        assert_overlay(pixel(&image, 40, 20, 20), luma(rgb), want, &format!("scale {scale}"));
    }
    // The offered scales, the first the default.
    let defs = b.set.setting_defs(0);
    assert_eq!(defs[0].key, "scale");
    assert_eq!(defs[0].choices[0].value, "75");
    // The label says the scale.
    b.configure("off", "50");
    assert!(b.set.labels()[0].contains("hot 50 %"), "{}", b.set.labels()[0]);
    b.configure("off", "200");
    assert!(b.set.labels()[0].contains("hot 200 %"), "{}", b.set.labels()[0]);
}

#[test]
fn the_legend_toggles_and_shows_the_palette_with_its_ends() {
    let mut b = Bench::new("legend");
    let (width, height) = (600u32, 400u32);
    b.play(width, height, &solid(width, height, [0.5, 0.5, 0.5]));

    b.configure("on", "100");
    let defs = b.set.setting_defs(0);
    assert_eq!(defs[1].key, "legend");
    assert_eq!(defs[1].choices.iter().map(|c| c.value).collect::<Vec<_>>(), ["on", "off"]);
    let image = b.render(width, height);
    // The bar under the picture runs cool (left) to warm (right).
    assert_overlay(pixel(&image, width, 2, height - 26 + 4), 0.5, 2.5 / width as f32, "legend left");
    assert_overlay(pixel(&image, width, width - 3, height - 26 + 4), 0.5, (width as f32 - 2.5) / width as f32, "legend right");
    // Along the bar the colour is the picture's own: a pixel at that place on the scale, over mid grey.
    for x in [width / 10, width / 4, width / 2, width * 3 / 4] {
        assert_overlay(pixel(&image, width, x, height - 26 + 4), 0.5, (x as f32 + 0.5) / width as f32, "legend");
    }
    let texts = b.set.scale_texts(0, [0.0, 0.0, width as f32, height as f32], 1.0);
    assert_eq!(texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["0 %", "100 %"]);
    // The picture still shows above it (a grey: the cool end).
    assert_overlay(pixel(&image, width, 300, 100), 0.5, 0.0, "the picture");
    b.configure("on", "50");
    assert_eq!(b.set.scale_texts(0, [0.0, 0.0, width as f32, height as f32], 1.0)[1].text, "50 %");

    // Off: no bar, no text, the picture takes the whole Panel.
    b.configure("off", "100");
    let image = b.render(width, height);
    assert_overlay(pixel(&image, width, width - 3, height - 26 + 4), 0.5, 0.0, "picture runs to the bottom");
    assert!(b.set.scale_texts(0, [0.0, 0.0, width as f32, height as f32], 1.0).is_empty());
}

#[test]
fn the_label_says_what_is_measured_and_that_the_picture_is_assumed_until_the_colour_setup_is_known() {
    let b = Bench::new("label");
    b.configure("on", "100");
    let label = b.set.labels()[0].clone();
    assert!(label.starts_with("Saturation heatmap"), "{label}");
    assert!(label.contains("Output picture"), "{label}");
    assert!(label.contains("Rec.709 assumed, unconverted"), "{label}");
    assert!(label.contains("100 % = the vectorscope's 100 % ring"), "{label}");
}

#[test]
fn nothing_is_drawn_until_a_frame_has_arrived() {
    let b = Bench::new("empty");
    b.configure("on", "100");
    let image = b.render(200, 100);
    assert!(image.chunks(4).all(|p| p[..3] == [0, 0, 0]));
    assert!(b.set.scale_texts(0, [0.0, 0.0, 200.0, 100.0], 1.0).is_empty());
}
