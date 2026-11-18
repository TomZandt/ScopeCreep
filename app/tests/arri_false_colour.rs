//! ARRI False Colour (#57), driven at the Feed seam: synthetic frames go in through the synthetic writer, the app's
//! Feed monitor hands them to the GPU as the app does, and the tests read back what the Panel drew, its label and
//! its legend. Expected bands come from the research table (`docs/research/colorchecker-reference-values.md`),
//! through the ColorChecker frames of #52. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::arri_false_colour::{ArriFalseColour, BANDS, band_at};
use scopecreep::display::{DisplayContext, DisplaySet};
use scopecreep::settings::PanelSettings;
use scopecreep_accuracy::colorchecker::{ArriBand, ColorChecker, TimelineSpace};
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
        set.register(Arc::new(ArriFalseColour::new(&rig.device, &rig.queue, TARGET)));
        Bench { rig, set }
    }

    fn configure(&self, legend: &str) {
        let mut settings = PanelSettings::default();
        settings.set(self.set.name(0), "legend", legend);
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

fn near(got: [u8; 3], want: [f32; 3]) -> bool {
    (0..3).all(|c| (got[c] as f32 - want[c] * 255.0).abs() <= 1.5)
}

fn band_colour(name: &str) -> [f32; 3] {
    BANDS.iter().find(|b| b.name == name).unwrap().colour
}

fn name_of(band: ArriBand) -> &'static str {
    match band {
        ArriBand::Purple => "purple",
        ArriBand::Blue => "blue",
        ArriBand::Green => "green",
        ArriBand::Pink => "pink",
        ArriBand::Yellow => "yellow",
        ArriBand::Red => "red",
    }
}

/// A frame (RGBA, bottom row first) whose every column `x` holds the grey `levels[x]`, for every row.
fn columns(levels: &[f32], height: u32) -> Vec<f32> {
    (0..height).flat_map(|_| levels.iter().flat_map(|v| [*v, *v, *v, 1.0])).collect()
}

#[test]
fn the_display_is_registered_in_the_standard_set() {
    let rig = Rig::new("registered");
    let (device, queue) = (&rig.device, &rig.queue);
    let set = DisplaySet::standard(device, queue, TARGET, false);
    assert!(set.names().contains(&"ARRI False Colour"), "{:?}", set.names());
}

#[test]
fn every_colorchecker_patch_shows_the_band_the_research_table_gives_for_the_bt709_oetf() {
    let mut b = Bench::new("patches");
    b.configure("off");
    let (width, height) = (600u32, 400u32);
    // The Output picture of the research table: each patch's linear Rec.709 light through the BT.709 OETF,
    // mirrored about zero for a channel below 0 (cyan's red), as the table's cyan 34.56 % needs. The chart's
    // layout comes from the ColorChecker frame; the display is given the picture as it is (no Colour Setup).
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    let layout = chart.frame(width, height);
    let mut pixels = vec![0.0f32; layout.pixels.len()];
    for patch in chart.patches() {
        let r = layout.patch_rect(patch.number);
        let encoded = patch.rec709_linear.map(|l| (l.signum() * bt709_oetf(l.abs())) as f32);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let i = (((height - 1 - y) * width + x) * 4) as usize; // the frame's bottom row is first
                pixels[i..i + 4].copy_from_slice(&[encoded[0], encoded[1], encoded[2], 1.0]);
            }
        }
    }
    b.play(width, height, &pixels);
    let image = b.render(width, height);
    let mut seen = Vec::new();
    for patch in chart.patches() {
        let r = layout.patch_rect(patch.number);
        let got = pixel(&image, width, (r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2);
        match patch.arri_band {
            Some(band) => assert!(near(got, band_colour(name_of(band))), "patch {} {}: {got:?} is not {}", patch.number, patch.name, name_of(band)),
            None => {
                let grey = (patch.rec709_luma.clamp(0.0, 1.0) * 255.0) as f32;
                assert!((0..3).all(|c| (got[c] as f32 - grey).abs() <= 1.5), "patch {} {}: {got:?} is not grey {grey:.1}", patch.number, patch.name);
            }
        }
        seen.push((patch.number, patch.arri_band));
    }
    // The routine's examples, spelled out: blue sky is green; neutral 5 (42.23 %) and neutral 6.5 (59.92 %) are grey.
    assert_eq!(seen[2].1, Some(ArriBand::Green));
    assert_eq!(seen[21].1, None);
    assert_eq!(seen[20].1, None);
    let neutral_5 = layout.patch_rect(22);
    let got = pixel(&image, width, (neutral_5.x0 + neutral_5.x1) / 2, (neutral_5.y0 + neutral_5.y1) / 2);
    assert!(got[0] == got[1] && got[1] == got[2], "neutral 5 is grey: {got:?}");
    // Cyan (18) sits out of the Rec.709 gamut: its Luma is 34.56 % and so it is grey, not green (ADR 0016).
    assert_eq!(seen[17].1, None);
}

#[test]
fn the_band_table_includes_each_lower_edge_and_excludes_each_upper_edge() {
    for (i, b) in BANDS.iter().enumerate() {
        assert_eq!(band_at(b.from).map(|x| x.name), Some(b.name), "{} includes its lower edge {}", b.name, b.from);
        if i < BANDS.len() - 1 {
            assert_ne!(band_at(b.to).map(|x| x.name), Some(b.name), "{} excludes its upper edge {}", b.name, b.to);
        }
    }
    // Grey between the bands: 4 % is the first grey, 38 % the first green.
    assert_eq!(band_at(4.0), None);
    assert_eq!(band_at(37.999).map(|b| b.name), None);
    assert_eq!(band_at(42.0), None);
    assert_eq!(band_at(56.0), None);
    assert_eq!(band_at(99.0).map(|b| b.name), Some("red"));
    // The ends are open: crushed past black is purple, clipped at or over white is red.
    assert_eq!(band_at(-3.0).map(|b| b.name), Some("purple"));
    assert_eq!(band_at(100.0).map(|b| b.name), Some("red"));
    assert_eq!(band_at(120.0).map(|b| b.name), Some("red"));
}

#[test]
fn a_synthetic_ramp_changes_band_at_each_edge() {
    let mut b = Bench::new("ramp");
    b.configure("off");
    // For every band edge: a column just under it and a column just over it (a hundredth of a percent either
    // side, which the GPU's 32-bit arithmetic resolves), and mid-band and mid-grey columns.
    let mut levels: Vec<f32> = Vec::new();
    let mut want: Vec<Option<&str>> = Vec::new();
    for edge in [0.0f64, 2.5, 4.0, 38.0, 42.0, 52.0, 56.0, 97.0, 99.0, 100.0] {
        for delta in [-0.01, 0.01] {
            let percent = edge + delta;
            if percent < 0.0 {
                continue;
            }
            levels.push((percent / 100.0) as f32);
            want.push(band_at(percent).map(|b| b.name));
        }
    }
    for percent in [1.0, 3.0, 20.0, 40.0, 47.0, 54.0, 75.0, 98.0] {
        levels.push(percent as f32 / 100.0);
        want.push(band_at(percent).map(|b| b.name));
    }
    let width = levels.len() as u32;
    let height = 8;
    b.play(width, height, &columns(&levels, height));
    let image = b.render(width, height);
    for (x, (level, want)) in levels.iter().zip(&want).enumerate() {
        let got = pixel(&image, width, x as u32, 4);
        match want {
            Some(name) => assert!(near(got, band_colour(name)), "{:.3} %: {got:?} is not {name}", level * 100.0),
            None => assert!(got[0] == got[1] && got[1] == got[2], "{:.3} %: {got:?} is not grey", level * 100.0),
        }
    }
    assert!(want.iter().filter(|w| w.is_none()).count() >= 5, "the ramp has grey stretches too");
}

#[test]
fn the_legend_toggles_and_the_label_says_the_bands_are_third_party_and_not_affiliated() {
    let mut b = Bench::new("legend");
    let (width, height) = (600u32, 400u32);
    b.play(width, height, &columns(&vec![0.2f32; width as usize], height));

    // On by default: six swatches under the picture, each band's colour, with the band edges as text.
    b.configure("on");
    let defs = b.set.setting_defs(0);
    assert_eq!(defs.len(), 2, "the legend and B/W");
    assert_eq!((defs[0].key, defs[0].choices.iter().map(|c| c.value).collect::<Vec<_>>()), ("legend", vec!["on", "off"]));
    let image = b.render(width, height);
    for (i, band) in BANDS.iter().enumerate() {
        let got = pixel(&image, width, 50 + 100 * i as u32, height - 26 + 4);
        assert!(near(got, band.colour), "legend swatch {i} is {}: {got:?}", band.name);
    }
    let texts = b.set.scale_texts(0, [0.0, 0.0, width as f32, height as f32], 1.0);
    assert_eq!(texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["0–2.5", "2.5–4", "38–42", "52–56", "97–99", "99–100"]);
    // The picture is still there above it (a grey of 20 %, not a band).
    let got = pixel(&image, width, 300, 100);
    assert!(got[0] == got[1] && got[1] == got[2] && got[0] > 40, "the picture shows above the legend: {got:?}");

    // Off: no swatches, no text, and the picture takes the whole Panel.
    b.configure("off");
    let image = b.render(width, height);
    assert_eq!(pixel(&image, width, 50, height - 26 + 4)[0], pixel(&image, width, 50, 100)[0], "the picture runs to the bottom");
    assert!(b.set.scale_texts(0, [0.0, 0.0, width as f32, height as f32], 1.0).is_empty());

    // The label, with the legend off as well as on.
    for legend in ["on", "off"] {
        b.configure(legend);
        let label = b.set.labels()[0].clone();
        assert!(label.starts_with("ARRI-style False Colour"), "{label}");
        assert!(label.contains("not affiliated with ARRI"), "{label}");
        assert!(label.contains("published by third parties"), "{label}");
        assert!(label.contains("not ARRI's specification"), "{label}");
        assert!(label.contains("Rec.709 assumed"), "an unknown Colour Setup is said: {label}");
    }
}

#[test]
fn nothing_is_drawn_until_a_frame_has_arrived() {
    let b = Bench::new("empty");
    b.configure("on");
    let image = b.render(200, 100);
    assert!(image.chunks(4).all(|p| p[..3] == [0, 0, 0]));
    assert!(b.set.scale_texts(0, [0.0, 0.0, 200.0, 100.0], 1.0).is_empty());
}

#[test]
fn a_panel_too_short_for_the_legend_keeps_the_whole_picture() {
    let mut b = Bench::new("short");
    b.configure("on");
    b.play(200, 100, &columns(&vec![0.2f32; 200], 100));
    let image = b.render(200, 100);
    assert_eq!(pixel(&image, 200, 100, 97)[0], pixel(&image, 200, 100, 50)[0], "no legend strip in a short Panel");
    assert!(b.set.scale_texts(0, [0.0, 0.0, 200.0, 100.0], 1.0).is_empty());
}
