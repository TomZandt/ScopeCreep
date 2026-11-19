//! Zoom on the clean picture (#165, spec #163), driven at the Feed seam: synthetic frames go in as Resolve's would,
//! the Panel draws into a target and the tests read the pixels back; the wheel and the zoom chip are what the window
//! gives the Panel. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::{DisplayContext, DisplaySet, Pointer};
use scopecreep::picture_view::PictureView;
use scopecreep::settings::{Menu, MenuRow, PanelSettings};
use scopecreep::window_text::{self, panel_title};
use std::sync::Arc;

const W: u32 = 800;
const H: u32 = 600;
/// The Panel's picture area: 600 x 400 at (20, 20).
const AREA: [f32; 4] = [20.0, 20.0, 600.0, 400.0];
const NAME: &str = "Clean picture";
/// 800 x 450 in 600 x 400 is Fit 75 %.
const FRAME: [u32; 2] = [800, 450];

struct Bench {
    rig: Rig,
    set: DisplaySet,
    settings: PanelSettings,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let mut set = DisplaySet::default();
        set.register(Arc::new(PictureView::new(&rig.device, &rig.queue, TARGET, false)));
        let mut bench = Bench { rig, set, settings: PanelSettings::default() };
        let frame = busy(FRAME[0], FRAME[1]);
        bench.play(FRAME[0], FRAME[1], &frame);
        bench
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let set = &self.set;
        self.rig.play(width, height, pixels, |rig| rig.poll(|encoder, frame| set.on_frame(encoder, frame)));
    }

    fn wheel(&self, at: [f32; 2], lines: f32) -> bool {
        self.set.pointer(0, AREA, 1.0, Pointer::Wheel { at, lines }).used
    }

    fn render(&self) -> Vec<u8> {
        self.rig.render(W, H, |pass| self.set.draw(pass, 0, AREA, [W as f32, H as f32], 1.0))
    }

    fn chip(&self) -> String {
        self.set.toolbar(0, &self.settings, AREA).chips[0].text.clone()
    }
}

fn px(image: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * W as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// A frame whose neighbouring pixels all differ.
fn busy(width: u32, height: u32) -> Vec<f32> {
    let code = |x: u32, y: u32| (((3 * x + 5 * y) % 200) + 20) as f32 / 255.0;
    (0..height).flat_map(|y| (0..width).flat_map(move |x| [code(x, y), code(x, y), code(x, y), 1.0])).collect()
}

/// Lengths of the runs of equal values, less the first and last (which the Panel's edge cuts short).
fn runs(values: &[u8]) -> Vec<usize> {
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

fn middle() -> [f32; 2] {
    [AREA[0] + AREA[2] / 2.0, AREA[1] + AREA[3] / 2.0]
}

/// Scroll in until the chip says `text`.
fn wheel_to(b: &Bench, text: &str) {
    for _ in 0..30 {
        if b.chip() == text {
            return;
        }
        b.wheel(middle(), 1.0);
    }
    panic!("never reached {text}: at {}", b.chip());
}

#[test]
fn the_wheel_over_the_panel_zooms_from_fit_and_the_picture_changes() {
    let b = Bench::new("clean-zoom-wheel");
    let fit = b.render();
    assert!(b.wheel(middle(), 1.0), "the Panel takes the wheel, with the Loupe off");
    assert_ne!(b.render(), fit, "the picture is drawn bigger");
    assert_eq!(b.chip(), "94 %");
    assert!(b.wheel(middle(), -1.0));
    assert_eq!(b.chip(), "Fit 75 %");
    assert_eq!(b.render(), fit, "back at Fit it is drawn as before");
}

#[test]
fn zoomed_in_every_picture_pixel_is_an_exact_square_and_the_wheel_stops_at_400() {
    let b = Bench::new("clean-zoom-squares");
    for (text, k) in [("100 %", 1usize), ("200 %", 2), ("300 %", 3), ("400 %", 4)] {
        wheel_to(&b, text);
        let image = b.render();
        let y = (AREA[1] + AREA[3] / 2.0) as u32;
        let row: Vec<u8> = (AREA[0] as u32..(AREA[0] + AREA[2]) as u32).map(|x| px(&image, x, y)[0]).collect();
        let r = runs(&row);
        assert!(r.len() > 20 && r.iter().all(|n| *n == k), "{text}: runs {r:?}");
    }
    for _ in 0..5 {
        b.wheel(middle(), 1.0);
    }
    assert_eq!(b.chip(), "400 %", "no further than 400 %");
}

#[test]
fn zoomed_in_the_panel_is_full_and_nothing_is_drawn_outside_it() {
    let b = Bench::new("clean-zoom-clip");
    wheel_to(&b, "200 %");
    let image = b.render();
    assert!(px(&image, 25, 25) != [0, 0, 0] && px(&image, 615, 415) != [0, 0, 0], "the picture fills the Panel's corners");
    for (x, y) in [(10, 300), (625, 300), (300, 10), (300, 425), (5, 5), (700, 500)] {
        assert_eq!(px(&image, x, y), [0, 0, 0], "outside the Panel at {x}, {y}");
    }
}

// ---- the toolbar seam

#[test]
fn the_clean_picture_shows_a_zoom_chip_that_reads_fit_by_default() {
    let b = Bench::new("clean-zoom-chip");
    let toolbar = b.set.toolbar(0, &b.settings, AREA);
    assert_eq!(toolbar.chips[0].text, "Fit 75 %");
    assert!(!toolbar.on[0], "not lit at Fit");
    assert_eq!(toolbar.click(0), Some((scopecreep::zoom::ZOOM_KEY, "")), "a click asks for the slider");
    assert_eq!(toolbar.chips.len(), 6, "the chip, then the channel and Loupe chips");
    wheel_to(&b, "200 %");
    let zoomed = b.set.toolbar(0, &b.settings, AREA);
    assert!(zoomed.on[0], "lit while zoomed in");
}

#[test]
fn a_scope_has_no_zoom_chip() {
    let rig = Rig::new("clean-zoom-scope");
    let (set, _) = DisplaySet::standard_with_handles(&rig.device, &rig.queue, TARGET, false);
    let waveform = (0..set.len()).find(|i| set.name(*i) == "Luma waveform").unwrap();
    assert!(set.zoom(waveform, AREA).is_none());
}

#[test]
fn the_popover_slider_runs_fit_to_400_and_sets_the_zoom_snapping_above_100() {
    let b = Bench::new("clean-zoom-slider");
    let readout = b.set.zoom(0, AREA).unwrap();
    let menu = Menu::zoom(0, &readout);
    let MenuRow::Slider { amount, value, .. } = &menu.rows[1] else { panic!("{:?}", menu.rows) };
    assert_eq!((amount.min, amount.max, *value), (75.0, 400.0, readout.percent));
    for (set, shows) in [(60.0, "60 %"), (100.0, "100 %"), (240.0, "200 %"), (260.0, "300 %"), (400.0, "400 %")] {
        let set = if set < 75.0 { 75.0 } else { set };
        b.set.set_zoom(0, AREA, set);
        let shown = b.chip();
        if set == 75.0 {
            assert_eq!(shown, "Fit 75 %", "the bottom of the slider is Fit ({shows})");
        } else {
            assert_eq!(shown, shows, "slider at {set}");
        }
    }
    b.set.set_zoom(0, AREA, 0.0);
    assert_eq!(b.chip(), "Fit 75 %");
    let mut shown = Menu::zoom(0, &b.set.zoom(0, AREA).unwrap());
    shown.show_value(scopecreep::zoom::ZOOM_KEY, 200.0);
    assert!(matches!(shown.rows[1], MenuRow::Slider { value: 200.0, .. }));
}

#[test]
fn the_chip_stays_on_a_small_panel_and_shortens_to_its_number() {
    let b = Bench::new("clean-zoom-small");
    let title = panel_title(NAME, &DisplayContext::default(), String::new());
    for scale in [1.0f32, 2.0] {
        let small = scopecreep::panel_grid::PanelLayout::new([0.0, 0.0, 190.0 * scale, 200.0 * scale], scale).title;
        let mut toolbar = b.set.toolbar(0, &b.settings, AREA);
        toolbar.fit_to(small[2], true, scale);
        assert_eq!(toolbar.chips[0].text, "75 %", "the word Fit goes first when there is no room");
        let (l, _, _) = window_text::lay_out_toolbar_title(&title, &toolbar.chips, true, small, scale);
        let chip = l.chips[0];
        assert!(chip[0] >= small[0] && chip[0] + chip[2] <= small[0] + small[2] + 0.01, "{chip:?} stays inside {small:?}");
        let mut roomy = b.set.toolbar(0, &b.settings, AREA);
        roomy.fit_to(400.0 * scale, true, scale);
        assert_eq!(roomy.chips[0].text, "Fit 75 %", "with room it says Fit");
    }
}

// ---- the settings seam

#[test]
fn zoom_is_not_saved_and_the_next_session_opens_at_fit() {
    let mut b = Bench::new("clean-zoom-unsaved");
    wheel_to(&b, "300 %");
    b.set.set_zoom(0, AREA, 200.0);
    b.settings.set(NAME, "channel", "luma");
    let text = b.settings.to_text();
    assert!(!text.contains("zoom") && !text.contains("200") && !text.contains("300"), "nothing about zoom is saved: {text}");
    assert_eq!(PanelSettings::from_text(&text), b.settings);
    // The rig holds the GPU, so the session ends before the next one starts.
    drop(b);
    let next = Bench::new("clean-zoom-next-session");
    assert_eq!(next.chip(), "Fit 75 %");
}
