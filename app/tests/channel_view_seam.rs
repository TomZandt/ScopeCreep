//! The Channel view and its Loupe (#55), driven at the Feed seam: synthetic frames go in through the synthetic
//! writer exactly as Resolve's would, the displays draw into a target, and the tests read back what was drawn,
//! the label, and what the Loupe reports it drew. Needs a GPU.
mod common;
use common::{Rig, TARGET, solid};
use scopecreep::display::{ChannelViewDrawn, DisplayContext, DisplaySet, Pointer};
use scopecreep::settings::PanelSettings;

const W: u32 = 800;
const H: u32 = 600;
/// The Panel the display is drawn into, in device pixels of the target.
const AREA: [f32; 4] = [20.0, 20.0, 600.0, 400.0];

struct Bench {
    rig: Rig,
    set: DisplaySet,
    index: usize,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let set = DisplaySet::standard(&rig.device, &rig.queue, TARGET, false);
        let index = set.names().iter().position(|n| *n == "Channel view").expect("the Channel view is registered");
        Bench { rig, set, index }
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let set = &self.set;
        self.rig.play(width, height, pixels, |rig| rig.poll(|encoder, frame| set.on_frame(encoder, frame)));
    }

    fn pointer(&self, event: Pointer) -> scopecreep::display::PointerResult {
        self.set.pointer(self.index, AREA, 1.0, event)
    }

    fn drawn(&self) -> ChannelViewDrawn {
        *self.set.read_back(self.index).and_then(|r| r.downcast::<ChannelViewDrawn>().ok()).expect("the Channel view reports what it drew")
    }

    /// Draw the Channel view into [`AREA`] of a W x H target and read it back (RGBA8, rows top first).
    fn render(&self) -> Vec<u8> {
        self.rig.render(W, H, |pass| self.set.draw(pass, self.index, AREA, [W as f32, H as f32], 1.0))
    }
}

fn px(image: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * W as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// A frame whose every neighbouring pixel differs in every channel.
fn busy(width: u32, height: u32) -> Vec<f32> {
    let code = |x: u32, y: u32, shift: u32| (((3 * x + 5 * y + shift) % 200) + 20) as f32 / 255.0;
    (0..height).flat_map(|y| (0..width).flat_map(move |x| [code(x, y, 0), code(x, y, 100), code(x, y, 57), 1.0])).collect()
}

fn centre(rect: [f32; 4]) -> [f32; 2] {
    [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0]
}

/// The lengths of the runs of equal pixels along a line, leaving out the first and last (cut by the Loupe's edge).
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

fn with_setting(b: &Bench, key: &str, value: &str) -> PanelSettings {
    let mut s = PanelSettings::default();
    s.set(b.set.name(b.index), key, value);
    s
}

#[test]
fn each_channel_choice_draws_exactly_that_channels_values_as_grey_and_blue_is_the_default() {
    let mut b = Bench::new("channels");
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    // A point in the Panel's picture but away from the Loupe, which starts in the middle.
    let (x, y) = (AREA[0] as u32 + 40, AREA[1] as u32 + 90);
    let grey = |v: u8| [v, v, v];
    assert_eq!(px(&b.render(), x, y), grey(204), "blue by default (0.8 of 255)");
    b.set.configure(&DisplayContext::default(), &with_setting(&b, "channel", "red"));
    assert_eq!(px(&b.render(), x, y), grey(102));
    b.set.configure(&DisplayContext::default(), &with_setting(&b, "channel", "green"));
    assert_eq!(px(&b.render(), x, y), grey(153));
    b.set.configure(&DisplayContext::default(), &with_setting(&b, "channel", "blue"));
    assert_eq!(px(&b.render(), x, y), grey(204));
}

#[test]
fn the_label_says_which_channel_is_shown() {
    let mut b = Bench::new("label");
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    let label = b.set.labels()[b.index].clone();
    assert!(label.starts_with("Blue channel as grey"), "{label}");
    b.set.configure(&DisplayContext::default(), &with_setting(&b, "channel", "red"));
    assert!(b.set.labels()[b.index].starts_with("Red channel as grey"), "{}", b.set.labels()[b.index]);
    assert!(b.set.labels()[b.index].contains("Loupe 200 %"), "{}", b.set.labels()[b.index]);
}

/// Every complete run of equal pixels across the middle row and down the middle column of the Loupe is `side` long.
fn assert_blocks(b: &Bench, side: usize) {
    let image = b.render();
    let rect = b.drawn().loupe;
    let c = centre(rect);
    // Stay inside the Loupe's own outline.
    let (left, right) = (rect[0] as u32 + 4, (rect[0] + rect[2]) as u32 - 4);
    let (top, bottom) = (rect[1] as u32 + 4, (rect[1] + rect[3]) as u32 - 4);
    let row: Vec<u8> = (left..right).map(|x| px(&image, x, c[1] as u32)[2]).collect();
    let column: Vec<u8> = (top..bottom).map(|y| px(&image, c[0] as u32, y)[2]).collect();
    assert!(runs(&row).iter().all(|n| *n == side), "across: {:?}", runs(&row));
    assert!(runs(&column).iter().all(|n| *n == side), "down: {:?}", runs(&column));
    assert!(runs(&row).len() > 5 && runs(&column).len() > 5);
}

#[test]
fn at_200_percent_each_source_pixel_is_2_by_2_screen_pixels_and_at_400_percent_4_by_4() {
    let mut b = Bench::new("blocks");
    b.play(192, 108, &busy(192, 108));
    b.render();
    assert_eq!(b.drawn().zoom, 200, "starts at 200 %");
    assert_blocks(&b, 2);
    let at = centre(b.drawn().loupe);
    b.pointer(Pointer::Wheel { at, lines: 2.0 });
    assert_eq!(b.drawn().zoom, 400);
    assert_blocks(&b, 4);
    let at = centre(b.drawn().loupe);
    b.pointer(Pointer::Wheel { at, lines: -3.0 });
    assert_eq!(b.drawn().zoom, 100);
    assert_blocks(&b, 1);
}

#[test]
fn scroll_zoom_stops_at_100_and_800_percent_and_only_acts_over_the_loupe() {
    let mut b = Bench::new("zoom-limits");
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    b.render();
    let at = centre(b.drawn().loupe);
    for _ in 0..20 {
        assert!(b.pointer(Pointer::Wheel { at, lines: 1.0 }).used);
    }
    assert_eq!(b.drawn().zoom, 800);
    for _ in 0..20 {
        b.pointer(Pointer::Wheel { at, lines: -1.0 });
    }
    assert_eq!(b.drawn().zoom, 100);
    let away = [AREA[0] + 2.0, AREA[1] + 2.0];
    let result = b.pointer(Pointer::Wheel { at: away, lines: 1.0 });
    assert!(!result.used && result.remember.is_empty());
    assert_eq!(b.drawn().zoom, 100, "the wheel off the Loupe does nothing");
}

#[test]
fn a_wheel_tick_that_changes_no_zoom_remembers_nothing() {
    let mut b = Bench::new("zoom-nothing-to-remember");
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    b.render();
    let at = centre(b.drawn().loupe);
    for _ in 0..20 {
        b.pointer(Pointer::Wheel { at, lines: 1.0 });
    }
    assert_eq!(b.drawn().zoom, 800);
    let at_limit = b.pointer(Pointer::Wheel { at, lines: 1.0 });
    assert!(at_limit.used && at_limit.remember.is_empty(), "already at the limit: {at_limit:?}");
    let part = b.pointer(Pointer::Wheel { at, lines: -0.2 });
    assert!(part.used && part.remember.is_empty(), "a part of a notch: {part:?}");
    assert!(!b.pointer(Pointer::Wheel { at, lines: -1.0 }).remember.is_empty(), "a real change is remembered");
}

#[test]
fn dragging_moves_the_magnified_area_and_the_loupe_stays_inside_the_panel() {
    let mut b = Bench::new("drag");
    // Dark on the left half, bright on the right.
    let split: Vec<f32> = (0..36).flat_map(|_| (0..64).flat_map(|x| if x < 32 { [0.2, 0.2, 0.2, 1.0] } else { [0.8, 0.8, 0.8, 1.0] })).collect();
    b.play(64, 36, &split);
    b.render();
    let before = b.drawn().loupe;
    // Not on the Loupe: not taken.
    assert!(!b.pointer(Pointer::Down([AREA[0] + 2.0, AREA[1] + 2.0])).used);
    // Grab it by its middle and move 30 px left and 10 down: it follows exactly.
    let c = centre(before);
    assert!(b.pointer(Pointer::Down(c)).used);
    b.pointer(Pointer::Move([c[0] - 30.0, c[1] + 10.0]));
    b.render();
    let moved = b.drawn().loupe;
    assert!((moved[0] - (before[0] - 30.0)).abs() < 0.6 && (moved[1] - (before[1] + 10.0)).abs() < 0.6, "{before:?} -> {moved:?}");
    // Far to the left of the Panel: the Loupe stops at the Panel's edge, still wholly inside, and shows dark.
    b.pointer(Pointer::Move([-500.0, c[1]]));
    let image = b.render();
    let left = b.drawn().loupe;
    assert!(inside(left, AREA) && (left[0] - AREA[0]).abs() < 0.6, "{left:?}");
    let m = centre(left);
    assert_eq!(px(&image, m[0] as u32, m[1] as u32), [51, 51, 51], "dark under the Loupe on the left");
    // Far to the right and below: inside the Panel again, and now it shows the bright half.
    b.pointer(Pointer::Move([5000.0, 5000.0]));
    let image = b.render();
    let right = b.drawn().loupe;
    assert!(inside(right, AREA), "{right:?}");
    b.pointer(Pointer::Up);
    let m = centre(right);
    assert_eq!(px(&image, m[0] as u32, m[1] as u32), [204, 204, 204], "bright under the Loupe on the right");
}

#[test]
fn the_loupe_size_is_small_medium_or_large_and_never_bigger_than_the_panel() {
    let mut b = Bench::new("sizes");
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    let defs = b.set.setting_defs(b.index);
    assert_eq!(defs.iter().find(|d| d.key == "size").unwrap().choices.iter().map(|c| c.value).collect::<Vec<_>>(), ["medium", "small", "large"]);
    let side = |size: &str| {
        b.set.configure(&DisplayContext::default(), &with_setting(&b, "size", size));
        b.render();
        b.drawn().loupe
    };
    let (small, medium, large) = (side("small"), side("medium"), side("large"));
    assert!(small[2] < medium[2] && medium[2] < large[2], "{small:?} {medium:?} {large:?}");
    assert!(large[2] <= AREA[3], "the large Loupe fits a Panel that is 400 high");
    assert_eq!(large[2], large[3], "square");
}

#[test]
fn position_zoom_and_size_are_remembered_between_sessions() {
    let mut first = Bench::new("remember-1");
    first.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    let mut settings = with_setting(&first, "size", "large");
    let name = first.set.name(first.index);
    first.set.configure(&DisplayContext::default(), &settings);
    first.render();
    let c = centre(first.drawn().loupe);
    let remember = |result: scopecreep::display::PointerResult, settings: &mut PanelSettings| {
        for (key, value) in result.remember {
            settings.set_state(name, key, &value);
        }
    };
    remember(first.pointer(Pointer::Down(c)), &mut settings);
    remember(first.pointer(Pointer::Move([c[0] + 120.0, c[1] - 60.0])), &mut settings);
    remember(first.pointer(Pointer::Up), &mut settings);
    first.render();
    let at = centre(first.drawn().loupe);
    remember(first.pointer(Pointer::Wheel { at, lines: 3.0 }), &mut settings);
    first.render();
    let (rect, zoom) = (first.drawn().loupe, first.drawn().zoom);
    assert_eq!(zoom, 500);
    let saved = settings.to_text();
    drop(first);

    let mut second = Bench::new("remember-2");
    second.set.configure(&DisplayContext::default(), &PanelSettings::from_text(&saved));
    second.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    second.render();
    let again = second.drawn();
    assert_eq!(again.zoom, 500);
    assert!(again.loupe.iter().zip(rect).all(|(a, b)| (a - b).abs() < 0.6), "{:?} vs {rect:?}", again.loupe);
}

#[test]
fn remembered_rubbish_falls_back_to_the_defaults() {
    let mut b = Bench::new("rubbish");
    let name = b.set.name(b.index);
    let text = format!("{name}\tstate:loupe_zoom\t9999\n{name}\tstate:loupe_x\tbanana\n{name}\tchannel\tpurple\n");
    b.set.configure(&DisplayContext::default(), &PanelSettings::from_text(&text));
    b.play(64, 36, &solid(64, 36, [0.4, 0.6, 0.8]));
    b.render();
    assert_eq!(b.drawn().zoom, 200);
    assert!(b.set.labels()[b.index].starts_with("Blue channel"));
}
