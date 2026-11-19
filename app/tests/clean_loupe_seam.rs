//! The clean picture's Loupe (#161), driven at the Feed seam: synthetic frames go in as Resolve's would, the Panel
//! draws into a target, and the tests read back the pixels and what the Loupe reports it drew. Needs a GPU.
mod common;
use common::{Rig, TARGET, solid};
use scopecreep::display::{ChannelViewDrawn, DisplayContext, DisplaySet, Pointer, PointerResult};
use scopecreep::picture_view::PictureView;
use scopecreep::settings::PanelSettings;
use std::sync::Arc;

const W: u32 = 800;
const H: u32 = 600;
const AREA: [f32; 4] = [20.0, 20.0, 600.0, 400.0];
const NAME: &str = "Clean picture";

struct Bench {
    rig: Rig,
    set: DisplaySet,
    view: Arc<PictureView>,
    settings: PanelSettings,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let mut set = DisplaySet::default();
        let view = Arc::new(PictureView::new(&rig.device, &rig.queue, TARGET, false));
        set.register(view.clone());
        Bench { rig, set, view, settings: PanelSettings::default() }
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let set = &self.set;
        self.rig.play(width, height, pixels, |rig| rig.poll(|encoder, frame| set.on_frame(encoder, frame)));
    }

    fn set(&mut self, key: &str, value: &str) {
        self.settings.set(NAME, key, value);
        self.set.configure(&DisplayContext::default(), &self.settings);
    }

    fn pointer(&self, event: Pointer) -> PointerResult {
        self.set.pointer(0, AREA, 1.0, event)
    }

    fn drawn(&self) -> Option<ChannelViewDrawn> {
        self.view.drawn()
    }

    fn render(&self) -> Vec<u8> {
        self.rig.render(W, H, |pass| self.set.draw(pass, 0, AREA, [W as f32, H as f32], 1.0))
    }
}

fn px(image: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * W as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

fn busy(width: u32, height: u32) -> Vec<f32> {
    let code = |x: u32, y: u32, shift: u32| (((3 * x + 5 * y + shift) % 200) + 20) as f32 / 255.0;
    (0..height).flat_map(|y| (0..width).flat_map(move |x| [code(x, y, 0), code(x, y, 100), code(x, y, 57), 1.0])).collect()
}

fn centre(rect: [f32; 4]) -> [f32; 2] {
    [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0]
}

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

fn inside(inner: [f32; 4], outer: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.01 && inner[1] >= outer[1] - 0.01 && inner[0] + inner[2] <= outer[0] + outer[2] + 0.01 && inner[1] + inner[3] <= outer[1] + outer[3] + 0.01
}

#[test]
fn with_the_loupe_off_nothing_is_drawn_over_the_picture_and_no_pointer_event_is_used() {
    let mut b = Bench::new("clean-loupe-off");
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    b.render();
    assert!(b.drawn().is_none(), "no Loupe drawn");
    let c = [AREA[0] + AREA[2] / 2.0, AREA[1] + AREA[3] / 2.0];
    assert_eq!(b.pointer(Pointer::Down(c)), PointerResult::default());
    assert_eq!(b.pointer(Pointer::Move([c[0] + 30.0, c[1]])), PointerResult::default());
    assert_eq!(b.pointer(Pointer::Wheel { at: c, lines: 3.0 }), PointerResult::default());
    assert_eq!(b.pointer(Pointer::Up), PointerResult::default());
}

#[test]
fn the_loupe_magnifies_colour_and_each_channel_as_the_panel_shows_it() {
    let mut b = Bench::new("clean-loupe-matches");
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    b.set("loupe", "on");
    let away = (AREA[0] as u32 + 40, AREA[1] as u32 + 90);
    for channel in ["off", "red", "green", "blue", "luma"] {
        b.set("channel", channel);
        let image = b.render();
        let c = centre(b.drawn().expect("drawn with the Loupe on").loupe);
        let (in_loupe, in_panel) = (px(&image, c[0] as u32, c[1] as u32), px(&image, away.0, away.1));
        assert!(in_loupe.iter().zip(in_panel).all(|(a, b)| (*a as i32 - b as i32).abs() <= 1), "{channel}: Loupe {in_loupe:?} vs Panel {in_panel:?}");
    }
    b.set("channel", "off");
    let image = b.render();
    let c = centre(b.drawn().unwrap().loupe);
    let p = px(&image, c[0] as u32, c[1] as u32);
    assert!(p[0] < p[2], "colour in the Loupe: {p:?}");
}

fn assert_blocks(b: &Bench, side: usize) {
    let image = b.render();
    let rect = b.drawn().unwrap().loupe;
    let c = centre(rect);
    let (left, right) = (rect[0] as u32 + 4, (rect[0] + rect[2]) as u32 - 4);
    let (top, bottom) = (rect[1] as u32 + 4, (rect[1] + rect[3]) as u32 - 4);
    let row: Vec<u8> = (left..right).map(|x| px(&image, x, c[1] as u32)[2]).collect();
    let column: Vec<u8> = (top..bottom).map(|y| px(&image, c[0] as u32, y)[2]).collect();
    assert!(runs(&row).iter().all(|n| *n == side), "across: {:?}", runs(&row));
    assert!(runs(&column).iter().all(|n| *n == side), "down: {:?}", runs(&column));
    assert!(runs(&row).len() > 5 && runs(&column).len() > 5);
}

#[test]
fn the_loupe_zooms_100_to_800_percent_in_exact_whole_pixel_blocks() {
    let mut b = Bench::new("clean-loupe-blocks");
    b.play(192, 108, &busy(192, 108));
    b.set("loupe", "on");
    b.render();
    assert_eq!(b.drawn().unwrap().zoom, 200);
    assert_blocks(&b, 2);
    let at = centre(b.drawn().unwrap().loupe);
    assert!(b.pointer(Pointer::Wheel { at, lines: 2.0 }).used);
    assert_eq!(b.drawn().unwrap().zoom, 400);
    assert_blocks(&b, 4);
    for _ in 0..20 {
        b.pointer(Pointer::Wheel { at, lines: 1.0 });
    }
    assert_eq!(b.drawn().unwrap().zoom, 800);
    for _ in 0..20 {
        b.pointer(Pointer::Wheel { at, lines: -1.0 });
    }
    assert_eq!(b.drawn().unwrap().zoom, 100);
    assert_blocks(&b, 1);
    let away = [AREA[0] + 2.0, AREA[1] + 2.0];
    assert!(!b.pointer(Pointer::Wheel { at: away, lines: 1.0 }).used, "the wheel off the Loupe is not taken");
}

#[test]
fn the_loupe_drags_and_stays_inside_the_panel() {
    let mut b = Bench::new("clean-loupe-drag");
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    b.set("loupe", "on");
    b.render();
    let before = b.drawn().unwrap().loupe;
    assert!(!b.pointer(Pointer::Down([AREA[0] + 2.0, AREA[1] + 2.0])).used);
    let c = centre(before);
    assert!(b.pointer(Pointer::Down(c)).used);
    b.pointer(Pointer::Move([c[0] - 30.0, c[1] + 10.0]));
    b.render();
    let moved = b.drawn().unwrap().loupe;
    assert!((moved[0] - (before[0] - 30.0)).abs() < 0.6 && (moved[1] - (before[1] + 10.0)).abs() < 0.6, "{before:?} -> {moved:?}");
    b.pointer(Pointer::Move([-500.0, -500.0]));
    b.render();
    assert!(inside(b.drawn().unwrap().loupe, AREA));
    b.pointer(Pointer::Move([5000.0, 5000.0]));
    b.render();
    assert!(inside(b.drawn().unwrap().loupe, AREA));
    b.pointer(Pointer::Up);
}

#[test]
fn the_loupe_size_is_a_popover_choice_and_position_and_zoom_are_remembered() {
    let mut first = Bench::new("clean-loupe-remember-1");
    first.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    first.set("loupe", "on");
    first.set("size", "small");
    first.render();
    let small = first.drawn().unwrap().loupe;
    first.set("size", "large");
    first.render();
    let large = first.drawn().unwrap().loupe;
    assert!(small[2] < large[2], "{small:?} {large:?}");
    let c = centre(large);
    let remembered = |result: PointerResult, settings: &mut PanelSettings| {
        for (key, value) in result.remember {
            settings.set_state(NAME, key, &value);
        }
    };
    let mut settings = first.settings.clone();
    remembered(first.pointer(Pointer::Down(c)), &mut settings);
    remembered(first.pointer(Pointer::Move([c[0] + 120.0, c[1] - 60.0])), &mut settings);
    remembered(first.pointer(Pointer::Up), &mut settings);
    first.render();
    let at = centre(first.drawn().unwrap().loupe);
    remembered(first.pointer(Pointer::Wheel { at, lines: 3.0 }), &mut settings);
    first.render();
    let (rect, zoom) = (first.drawn().unwrap().loupe, first.drawn().unwrap().zoom);
    assert_eq!(zoom, 500);
    let saved = settings.to_text();
    drop(first);

    let mut second = Bench::new("clean-loupe-remember-2");
    second.set.configure(&DisplayContext::default(), &PanelSettings::from_text(&saved));
    second.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    second.render();
    let again = second.drawn().expect("the Loupe is on again");
    assert_eq!(again.zoom, 500);
    assert!(again.loupe.iter().zip(rect).all(|(a, b)| (a - b).abs() < 0.6), "{:?} vs {rect:?}", again.loupe);
}
