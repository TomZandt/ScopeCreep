//! Zoom on the clean picture (#165, spec #163), driven at the Feed seam: synthetic frames go in as Resolve's would,
//! the Panel draws into a target and the tests read the pixels back; the wheel and the zoom chip are what the window
//! gives the Panel. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::{DisplayContext, DisplaySet, Pointer};
use scopecreep::picture_view::PictureView;
use scopecreep::settings::PanelSettings;
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
        set.register(Arc::new(PictureView::new(
            &rig.device,
            &rig.queue,
            TARGET,
            false,
        )));
        let mut bench = Bench {
            rig,
            set,
            settings: PanelSettings::default(),
        };
        let frame = busy(FRAME[0], FRAME[1]);
        bench.play(FRAME[0], FRAME[1], &frame);
        bench
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let set = &self.set;
        self.rig.play(width, height, pixels, |rig| {
            rig.poll(|encoder, frame| set.on_frame(encoder, frame))
        });
    }

    fn wheel(&self, at: [f32; 2], lines: f32) -> bool {
        self.set
            .pointer(0, AREA, 1.0, Pointer::Wheel { at, lines })
            .used
    }

    fn render(&self) -> Vec<u8> {
        self.rig.render(W, H, |pass| {
            self.set.draw(pass, 0, AREA, [W as f32, H as f32], 1.0)
        })
    }

    fn chip(&self) -> String {
        self.set.zoom_tag(0, AREA, 1.0).expect("a zoom tag")
    }
}

fn px(image: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * W as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// A frame whose neighbouring pixels all differ.
fn busy(width: u32, height: u32) -> Vec<f32> {
    let code = |x: u32, y: u32| (((3 * x + 5 * y) % 200) + 20) as f32 / 255.0;
    (0..height)
        .flat_map(|y| (0..width).flat_map(move |x| [code(x, y), code(x, y), code(x, y), 1.0]))
        .collect()
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
    assert!(
        b.wheel(middle(), 1.0),
        "the Panel takes the wheel, with the Loupe off"
    );
    assert_ne!(b.render(), fit, "the picture is drawn bigger");
    assert_eq!(b.chip(), "94 %");
    assert!(b.wheel(middle(), -1.0));
    assert_eq!(b.chip(), "Fit 75 %");
    assert_eq!(b.render(), fit, "back at Fit it is drawn as before");
}

#[test]
fn zoomed_in_every_picture_pixel_is_an_exact_square_and_the_wheel_stops_at_800() {
    let b = Bench::new("clean-zoom-squares");
    for (text, k) in [
        ("100 %", 1usize),
        ("200 %", 2),
        ("300 %", 3),
        ("400 %", 4),
        ("800 %", 8),
    ] {
        wheel_to(&b, text);
        let image = b.render();
        let y = (AREA[1] + AREA[3] / 2.0) as u32;
        let row: Vec<u8> = (AREA[0] as u32..(AREA[0] + AREA[2]) as u32)
            .map(|x| px(&image, x, y)[0])
            .collect();
        let r = runs(&row);
        assert!(
            r.len() > 20 && r.iter().all(|n| *n == k),
            "{text}: runs {r:?}"
        );
    }
    for _ in 0..5 {
        b.wheel(middle(), 1.0);
    }
    assert_eq!(b.chip(), "800 %", "no further than 800 %");
}

#[test]
fn zoomed_in_the_panel_is_full_and_nothing_is_drawn_outside_it() {
    let b = Bench::new("clean-zoom-clip");
    wheel_to(&b, "200 %");
    let image = b.render();
    assert!(
        px(&image, 25, 25) != [0, 0, 0] && px(&image, 615, 415) != [0, 0, 0],
        "the picture fills the Panel's corners"
    );
    for (x, y) in [
        (10, 300),
        (625, 300),
        (300, 10),
        (300, 425),
        (5, 5),
        (700, 500),
    ] {
        assert_eq!(px(&image, x, y), [0, 0, 0], "outside the Panel at {x}, {y}");
    }
}

// ---- the toolbar seam

#[test]
fn the_clean_picture_shows_a_read_only_zoom_tag_that_reads_fit_by_default_and_no_zoom_chip() {
    let b = Bench::new("clean-zoom-chip");
    assert_eq!(b.chip(), "Fit 75 %");
    let toolbar = b.set.toolbar(0, &b.settings);
    assert_eq!(
        toolbar.chips.len(),
        5,
        "the channel and Loupe chips only: zoom and Link are in the view strip"
    );
    assert!(
        toolbar
            .chips
            .iter()
            .all(|c| !c.text.contains('%') && c.text != "Link")
    );
    wheel_to(&b, "200 %");
    assert_eq!(b.chip(), "200 %", "the tag follows the zoom");
}

#[test]
fn a_scope_has_no_zoom_tag() {
    let rig = Rig::new("clean-zoom-scope");
    let (set, _) = DisplaySet::standard_with_handles(&rig.device, &rig.queue, TARGET, false);
    let waveform = (0..set.len())
        .find(|i| set.name(*i) == "Luma waveform")
        .unwrap();
    assert!(set.zoom(waveform, AREA, 1.0).is_none());
    assert!(set.zoom_tag(waveform, AREA, 1.0).is_none());
}

#[test]
fn fit_in_the_view_strip_goes_back_to_fit() {
    let b = Bench::new("clean-zoom-click-fit");
    let fit = b.render();
    wheel_to(&b, "300 %");
    assert_eq!(b.chip(), "300 %");
    b.set.fit_all(&[AREA], 1.0);
    assert_eq!(b.chip(), "Fit 75 %");
    assert_eq!(b.render(), fit, "drawn as at Fit");
}

#[test]
fn the_zoom_tag_gives_way_to_the_hover_on_a_small_panel_and_never_to_a_chip() {
    let b = Bench::new("clean-zoom-small");
    let mut title = panel_title(NAME, &DisplayContext::default(), String::new());
    title.tags.push(b.chip());
    for scale in [1.0f32, 2.0] {
        let small = scopecreep::panel_grid::PanelLayout::new(
            [0.0, 0.0, 190.0 * scale, 200.0 * scale],
            scale,
        )
        .title;
        let mut toolbar = b.set.toolbar(0, &b.settings);
        toolbar.fit_to(small[2], true, scale);
        let (l, tags, _) = window_text::lay_out_toolbar_title(
            &title,
            &toolbar.chips,
            &toolbar.groups,
            true,
            small,
            scale,
        );
        assert!(
            l.chips
                .iter()
                .all(|c| c[0] + c[2] <= small[0] + small[2] + 0.01)
        );
        assert!(
            l.tags.is_some() || l.tags_in_tooltip || tags.is_empty() || l.name_cut,
            "the tag is shown or in the hover"
        );
        let wide = [0.0, 0.0, 700.0 * scale, 24.0 * scale];
        let (_, tags, shown) =
            window_text::lay_out_toolbar_title(&title, &[], &[], true, wide, scale);
        assert!(
            shown && tags.ends_with("Fit 75 %"),
            "with room it says it: {tags}"
        );
    }
}

// ---- the settings seam

#[test]
fn zoom_is_not_saved_and_the_next_session_opens_at_fit() {
    let mut b = Bench::new("clean-zoom-unsaved");
    wheel_to(&b, "300 %");
    b.set.set_zoom(0, AREA, 1.0, 200.0);
    b.settings.set(NAME, "channel", "luma");
    let text = b.settings.to_text();
    assert!(
        !text.contains("zoom") && !text.contains("200") && !text.contains("300"),
        "nothing about zoom is saved: {text}"
    );
    assert_eq!(PanelSettings::from_text(&text), b.settings);
    // The rig holds the GPU, so the session ends before the next one starts.
    drop(b);
    let next = Bench::new("clean-zoom-next-session");
    assert_eq!(next.chip(), "Fit 75 %");
}

// ---- pan with the middle button (#166)

impl Bench {
    fn pointer(&self, event: Pointer) -> bool {
        self.set.pointer(0, AREA, 1.0, event).used
    }
}

/// A frame split down the middle: dark on the left, bright on the right.
fn split(width: u32, height: u32) -> Vec<f32> {
    (0..height)
        .flat_map(|_| {
            (0..width).flat_map(move |x| {
                if x < width / 2 {
                    [0.1, 0.1, 0.1, 1.0]
                } else {
                    [0.9, 0.9, 0.9, 1.0]
                }
            })
        })
        .collect()
}

#[test]
fn a_middle_drag_over_the_zoomed_picture_moves_what_is_drawn() {
    let mut b = Bench::new("clean-zoom-pan");
    b.play(FRAME[0], FRAME[1], &split(FRAME[0], FRAME[1]));
    wheel_to(&b, "200 %");
    let y = (AREA[1] + AREA[3] / 2.0) as u32;
    let first = b.render();
    // Zoomed on the middle of the frame: the split sits at the middle of the Panel.
    let (left, right) = (px(&first, 100, y)[0], px(&first, 540, y)[0]);
    assert!(
        left < 80 && right > 200,
        "dark left of the split, bright right of it: {left} {right}"
    );
    // Drag 200 px to the left: the picture follows the pointer, so the split moves left by 200 px.
    let from = middle();
    assert!(b.pointer(Pointer::MiddleDown(from)));
    assert!(b.pointer(Pointer::Move([from[0] - 100.0, from[1]])));
    assert!(b.pointer(Pointer::Move([from[0] - 200.0, from[1]])));
    assert!(b.pointer(Pointer::MiddleUp));
    let panned = b.render();
    assert_ne!(panned, first);
    assert!(
        px(&panned, 300, y)[0] > 200 && px(&panned, 100, y)[0] < 80,
        "the split moved with the pointer"
    );
    assert!(
        px(&panned, 110, y)[0] < 80 && px(&panned, 130, y)[0] > 200,
        "split at about x = 120"
    );
    // Letting go ends the drag.
    assert!(!b.pointer(Pointer::Move([from[0] + 100.0, from[1]])));
    assert_eq!(b.render(), panned);
}

#[test]
fn a_middle_drag_at_fit_moves_nothing_and_the_left_button_and_right_click_are_as_before() {
    let b = Bench::new("clean-zoom-pan-fit");
    let fit = b.render();
    b.pointer(Pointer::MiddleDown(middle()));
    b.pointer(Pointer::Move([middle()[0] - 90.0, middle()[1]]));
    b.pointer(Pointer::MiddleUp);
    assert_eq!(b.render(), fit);
    assert!(
        !b.pointer(Pointer::Down(middle())),
        "the left button is not the Panel's with the Loupe off"
    );
    assert!(!b.pointer(Pointer::Up));
    assert!(
        !b.pointer(Pointer::Move(middle())),
        "no drag going: a move is nobody's"
    );
}
