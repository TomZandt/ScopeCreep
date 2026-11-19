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
use scopecreep::settings::PanelSettings;
use scopecreep::project::{Answer, ProjectReader};
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
    fn draw(&self, _pass: &mut wgpu::RenderPass<'_>, _area: [f32; 4], _surface: [f32; 2], _scale: f32) {}
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
        Bench::make(Rig::with_colour_setup(name, colour_setup))
    }

    /// No manual Colour Setup; the project, read by script once Resolve stops, is Rec.709 from DaVinci WG.
    fn with_project(name: &str) -> Bench {
        let mut answer: Answer = vec![("result".into(), "ok".into()), ("project".into(), "scopecreep-test".into()), ("studio".into(), "1".into())];
        for (k, v) in [("colorScienceMode", "davinciYRGBColorManagedv2"), ("colorSpaceTimeline", "DaVinci WG/Intermediate"), ("colorSpaceOutput", "Rec.709 Gamma 2.4")] {
            answer.push((format!("setting.{k}"), v.to_string()));
        }
        Bench::make(Rig::build(name, 1024 * 64 * 16, None, Some(Box::new(Helper(answer)))))
    }

    fn make(mut rig: Rig) -> Bench {
        let frames = Arc::new(AtomicUsize::new(0));
        let mut set = DisplaySet::default();
        set.register(Arc::new(PictureView::new(&rig.device, &rig.queue, TARGET, false)));
        set.register(Arc::new(Counter(frames.clone())));
        let settings = Arc::new(Mutex::new(PanelSettings::default()));
        let mut feed = FeedLoop::new(set.clone(), settings.clone(), OutputConversion::new(&rig.device, &rig.queue));
        rig.step(&mut feed);
        Bench { rig, feed, set, settings, frames }
    }

    /// Resolve plays one frame of one colour, and the app has a moment to take it.
    fn play(&mut self, rgb: [f32; 3]) {
        let pixels = common::solid(W, H, rgb);
        let feed = &mut self.feed;
        self.rig.play(W, H, &pixels, |rig| {
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
        let image = self.rig.render(W, H, |pass| self.set.draw(pass, 0, [0.0, 0.0, W as f32, H as f32], [W as f32, H as f32], 1.0));
        let i = ((H / 2) as usize * W as usize + (W / 2) as usize) * 4;
        [image[i], image[i + 1], image[i + 2]]
    }

    fn counted(&self) -> usize {
        self.frames.load(Ordering::Relaxed)
    }
}

#[test]
fn a_setting_changed_with_resolve_stopped_measures_the_frame_on_screen_again_through_output_conversion() {
    let mut b = Bench::new("loop-setting", Some(DWG_REC709));
    b.play([0.5, 0.5, 0.5]);
    let converted = b.shown();
    assert!(converted[0].abs_diff(128) > 8, "the picture on screen is the converted one, not the frame as sent: {converted:?}");
    let before = b.counted();
    b.settings.lock().unwrap().set("Counter", "anything", "else");
    let changed = b.rig.step(&mut b.feed);
    assert_eq!(b.counted(), before + 1, "every display measures the frame on screen once more");
    assert!(changed.picture);
    assert_eq!(b.shown(), converted, "measured through Output Conversion again, not the raw frame");
}

#[test]
fn a_colour_setup_arriving_while_resolve_is_stopped_converts_the_frame_on_screen_and_measures_it_again() {
    let mut b = Bench::with_project("loop-setup");
    b.play([0.5, 0.5, 0.5]);
    let as_sent = b.shown();
    let before = b.counted();
    // Resolve stops rendering; the app reads the project, and its Colour Setup arrives with no new frame.
    let changed = b.stopped(1500);
    assert!(changed.picture, "the picture changed with no new frame");
    assert!(b.counted() > before, "the displays measured again");
    assert!(b.shown()[0].abs_diff(as_sent[0]) > 8, "the frame on screen is now converted");
}

#[test]
fn an_idle_poll_does_no_work() {
    let mut b = Bench::new("loop-idle", Some(DWG_REC709));
    b.play([0.5, 0.5, 0.5]);
    b.stopped(100);
    let (frames, scope_frames) = (b.counted(), b.feed.scope_frames);
    assert_eq!(b.stopped(200), Changed::default(), "nothing to redraw");
    assert_eq!((b.counted(), b.feed.scope_frames), (frames, scope_frames), "no display measured anything");
}
