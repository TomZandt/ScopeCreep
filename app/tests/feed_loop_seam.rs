//! The Feed loop (#109, spec #106), driven at the Feed: the same step the app's Feed thread runs, with a stand-in
//! Resolve writing frames. A setting chosen in a menu or a Colour Setup arriving with Resolve stopped measures the
//! frame on screen again through Output Conversion, and an idle poll does nothing. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::{Display, DisplayContext, DisplaySet};
use scopecreep::feed_loop::{Changed, FeedLoop};
use scopecreep::gpu_frame::GpuFrame;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::picture_view::PictureView;
use scopecreep::project::{Answer, ProjectReader};
use scopecreep::settings::PanelSettings;
use scopecreep::waveform::Waveform;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const W: u32 = 16;
const H: u32 = 9;
const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";

/// A scripting helper that answers every read at once with the same project.
struct Helper(Answer);

impl ProjectReader for Helper {
    fn request(&mut self) {}
    fn take_answer(&mut self) -> Option<Answer> {
        Some(self.0.clone())
    }
    fn abandon(&mut self) {}
}

/// A display that only counts the frames it is handed.
struct Counter(Arc<AtomicUsize>);

impl Display for Counter {
    fn name(&self) -> &'static str {
        "Counter"
    }
    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, _frame: &GpuFrame) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    fn configure(&self, _context: &DisplayContext) {}
    fn label(&self) -> String {
        "Counter".into()
    }
    fn draw(
        &self,
        _pass: &mut wgpu::RenderPass<'_>,
        _area: [f32; 4],
        _surface: [f32; 2],
        _scale: f32,
    ) {
    }
}

struct Bench {
    rig: Rig,
    feed: FeedLoop,
    set: DisplaySet,
    settings: Arc<Mutex<PanelSettings>>,
    /// Frames the counting display was handed.
    frames: Arc<AtomicUsize>,
}

impl Bench {
    /// The Picture View (index 0) and a counting display, behind the app's Feed loop. `colour_setup`: the Sender's
    /// manual Colour Setup, or none.
    fn new(name: &str, colour_setup: Option<&str>) -> Bench {
        Bench::make(Rig::with_colour_setup(name, colour_setup), false)
    }

    /// As [`Bench::new`] with a luma waveform as a third display.
    fn with_waveform(name: &str) -> Bench {
        Bench::make(Rig::with_colour_setup(name, Some(DWG_REC709)), true)
    }

    /// No manual Colour Setup; the project, read by script once Resolve stops, is Rec.709 from DaVinci WG.
    fn with_project(name: &str) -> Bench {
        let mut answer: Answer = vec![
            ("result".into(), "ok".into()),
            ("project".into(), "scopecreep-test".into()),
            ("studio".into(), "1".into()),
        ];
        for (k, v) in [
            ("colorScienceMode", "davinciYRGBColorManagedv2"),
            ("colorSpaceTimeline", "DaVinci WG/Intermediate"),
            ("colorSpaceOutput", "Rec.709 Gamma 2.4"),
        ] {
            answer.push((format!("setting.{k}"), v.to_string()));
        }
        Bench::make(
            Rig::build(name, 1024 * 64 * 16, None, Some(Box::new(Helper(answer)))),
            false,
        )
    }

    fn make(mut rig: Rig, waveform: bool) -> Bench {
        let frames = Arc::new(AtomicUsize::new(0));
        let mut set = DisplaySet::default();
        set.register(Arc::new(PictureView::new(
            &rig.device,
            &rig.queue,
            TARGET,
            false,
        )));
        set.register(Arc::new(Counter(frames.clone())));
        if waveform {
            set.register(Arc::new(Waveform::new(&rig.device, &rig.queue, TARGET)));
        }
        let settings = Arc::new(Mutex::new(PanelSettings::default()));
        let mut feed = FeedLoop::new(
            set.clone(),
            settings.clone(),
            OutputConversion::new(&rig.device, &rig.queue),
        );
        rig.step(&mut feed);
        Bench {
            rig,
            feed,
            set,
            settings,
            frames,
        }
    }

    /// Resolve plays one frame of one colour, and the app has a moment to take it.
    fn play(&mut self, rgb: [f32; 3]) {
        self.play_wide(W, rgb);
    }

    /// As [`Bench::play`], with a frame `width` pixels wide.
    fn play_wide(&mut self, width: u32, rgb: [f32; 3]) {
        let pixels = common::solid(width, H, rgb);
        let feed = &mut self.feed;
        self.rig.play(width, H, &pixels, |rig| {
            rig.step(feed);
        });
    }

    /// Resolve stays stopped for `ms` milliseconds; whether any step changed anything.
    fn stopped(&mut self, ms: u64) -> Changed {
        let (feed, mut changed) = (&mut self.feed, Changed::default());
        self.rig.wait(ms, |rig| {
            let step = rig.step(feed);
            changed.picture |= step.picture;
            changed.labels |= step.labels;
        });
        changed
    }

    /// The middle pixel of the Picture View's Panel.
    fn shown(&self) -> [u8; 3] {
        let image = self.rig.render(W, H, |pass| {
            self.set.draw(
                pass,
                0,
                [0.0, 0.0, W as f32, H as f32],
                [W as f32, H as f32],
                1.0,
            )
        });
        let i = ((H / 2) as usize * W as usize + (W / 2) as usize) * 4;
        [image[i], image[i + 1], image[i + 2]]
    }

    fn counted(&self) -> usize {
        self.frames.load(Ordering::Relaxed)
    }
}

#[test]
fn a_setting_changed_with_resolve_stopped_measures_the_frame_on_screen_again_through_output_conversion()
 {
    let mut b = Bench::new("loop-setting", Some(DWG_REC709));
    b.play([0.5, 0.5, 0.5]);
    let converted = b.shown();
    assert!(
        converted[0].abs_diff(128) > 8,
        "the picture on screen is the converted one, not the frame as sent: {converted:?}"
    );
    let before = b.counted();
    b.settings
        .lock()
        .unwrap()
        .set("Counter", "anything", "else");
    let changed = b.rig.step(&mut b.feed);
    assert_eq!(
        b.counted(),
        before + 1,
        "every display measures the frame on screen once more"
    );
    assert!(changed.picture);
    assert_eq!(
        b.shown(),
        converted,
        "measured through Output Conversion again, not the raw frame"
    );
}

#[test]
fn a_colour_setup_arriving_while_resolve_is_stopped_converts_the_frame_on_screen_and_measures_it_again()
 {
    let mut b = Bench::with_project("loop-setup");
    b.play([0.5, 0.5, 0.5]);
    let as_sent = b.shown();
    let before = b.counted();
    // Resolve stops rendering; the app reads the project, and its Colour Setup arrives with no new frame.
    let changed = b.stopped(1500);
    assert!(changed.picture, "the picture changed with no new frame");
    assert!(b.counted() > before, "the displays measured again");
    assert!(
        b.shown()[0].abs_diff(as_sent[0]) > 8,
        "the frame on screen is now converted"
    );
}

#[test]
fn an_idle_poll_does_no_work() {
    let mut b = Bench::new("loop-idle", Some(DWG_REC709));
    b.play([0.5, 0.5, 0.5]);
    b.stopped(100);
    let (frames, scope_frames) = (b.counted(), b.feed.scope_frames);
    assert_eq!(b.stopped(200), Changed::default(), "nothing to redraw");
    assert_eq!(
        (b.counted(), b.feed.scope_frames),
        (frames, scope_frames),
        "no display measured anything"
    );
}

/// The notice the Window shows.
fn notice(b: &Bench) -> Option<String> {
    b.feed.texts().lock().unwrap().notice.clone()
}

// The GPU memory cap (`gpu_memory::set_limit`) is per thread, and every test runs on its own thread.

#[test]
fn when_the_gpu_has_no_memory_for_the_picture_the_window_says_so_and_carries_on_when_it_frees_up() {
    let mut b = Bench::new("loop-short-picture", Some(DWG_REC709));
    b.play([0.5, 0.5, 0.5]);
    let before = (b.counted(), b.shown());
    scopecreep::gpu_memory::set_limit(Some(1_000)); // smaller than any frame-sized allocation
    // A frame of another size: the picture needs new textures.
    b.play_wide(W * 2, [0.8, 0.8, 0.8]);
    assert_eq!(
        notice(&b).as_deref(),
        Some(
            "Not enough GPU memory for the picture (needs about 1 MB). Close another GPU app; ScopeCreep will carry on when memory frees up."
        ),
    );
    assert_eq!(
        (b.counted(), b.shown()),
        before,
        "nothing was measured, and the last picture stays"
    );
    scopecreep::gpu_memory::set_limit(None); // memory frees up
    b.play_wide(W * 2, [0.8, 0.8, 0.8]);
    assert_eq!(notice(&b), None, "the notice goes with the next frame");
    assert_eq!(b.counted(), before.0 + 1, "and the displays measure it");
    assert_ne!(b.shown(), before.1, "the new picture is shown");
}

#[test]
fn when_the_gpu_has_no_memory_for_the_waveform_the_window_says_so_and_the_picture_carries_on() {
    let mut b = Bench::with_waveform("loop-short-scopes");
    b.play([0.5, 0.5, 0.5]);
    assert_eq!(notice(&b), None);
    let before = b.shown();
    // Room for the picture (a few KB at 16 × 9) but not the waveform's counts (16 columns × 1026 rows × 4 bytes).
    scopecreep::gpu_memory::set_limit(Some(20_000));
    b.play([0.8, 0.8, 0.8]);
    assert_eq!(
        notice(&b),
        None,
        "the waveform's buffers were made before: nothing new is needed"
    );
    // A frame of another width needs new buffers.
    b.play_wide(W * 2, [0.8, 0.8, 0.8]);
    let said = notice(&b).unwrap_or_default();
    assert!(
        said.starts_with("Not enough GPU memory for the scopes (needs about "),
        "{said}"
    );
    assert_ne!(b.shown(), before, "the picture still follows the Feed");
    scopecreep::gpu_memory::set_limit(None);
    b.play([0.3, 0.3, 0.3]);
    assert_eq!(
        notice(&b),
        None,
        "the next frame makes the buffers and the notice goes"
    );
}

#[test]
fn panels_on_a_page_that_is_not_current_are_not_measured_or_drawn() {
    let mut b = Bench::new("loop-pages", Some(DWG_REC709));
    b.play([0.5, 0.5, 0.5]);
    let grey = b.shown();
    assert_ne!(
        grey,
        [0, 0, 0],
        "the Picture View draws while it is on the page"
    );
    // Only the counting display (Panel 1) is on the page shown.
    b.set.set_shown(&[false, true]);
    b.rig.step(&mut b.feed);
    let before = b.counted();
    b.play([0.8, 0.8, 0.8]);
    assert_eq!(b.counted(), before + 1, "a Panel on the page is measured");
    assert_eq!(b.shown(), [0, 0, 0], "the Picture View is not drawn");
    // Nothing on the page: no display measures a new frame.
    b.set.set_shown(&[false, false]);
    b.rig.step(&mut b.feed);
    let before = b.counted();
    b.play([0.3, 0.3, 0.3]);
    assert_eq!(
        b.counted(),
        before,
        "a Panel on another page is not measured"
    );
    // The Picture View's page comes back: it measures the frame on screen again, so it shows the latest picture.
    b.set.set_shown(&[true, true]);
    let changed = b.rig.step(&mut b.feed);
    assert!(changed.picture, "the Panels that came back measured");
    let latest = b.shown();
    assert_ne!(latest, [0, 0, 0]);
    assert_ne!(
        latest, grey,
        "the picture is the 0.3 grey played while it was away"
    );
    b.play([0.3, 0.3, 0.3]);
    assert_eq!(b.shown(), latest, "as a frame arriving would have made it");
}

#[test]
fn a_sender_short_of_gpu_memory_is_shown_in_the_window() {
    let mut b = Bench::new("loop-short-sender", Some(DWG_REC709));
    b.play([0.5, 0.5, 0.5]);
    b.rig.writer.set_sender_error(
        "Not enough GPU memory in Resolve to send frames (needs about 133 MB). Close another GPU app; the Sender tries again every few seconds.",
    );
    b.stopped(300);
    assert_eq!(
        notice(&b).as_deref(),
        Some(
            "Sender: Not enough GPU memory in Resolve to send frames (needs about 133 MB). Close another GPU app; the Sender tries again every few seconds."
        )
    );
}
