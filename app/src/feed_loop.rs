//! The Feed loop (#109, spec #106): one step per poll of the Feed, run by the app's Feed thread and by the tests.
//! A step reads the Feed, puts a new frame on the GPU once, converts it once (Output Conversion), hands it to every
//! display, and measures the frame on screen again when something other than a frame changed what the displays
//! measure: a setting chosen in a menu, or a Colour Setup that arrived or changed with Resolve stopped. It then
//! remakes the labels when what they say changed. The thread only loops, sleeps and wakes the window.
//!
//! An idle step costs a few comparisons and allocates nothing (ADR 0003): the slower work happens only when the
//! Feed's status, the settings or a frame changed.
use crate::display::{DisplayContext, DisplaySet};
use crate::gpu_frame::FrameUploader;
use crate::gpu_memory;
use crate::monitor::{FeedState, Monitor, Status};
use crate::output_conversion::OutputConversion;
use crate::settings::PanelSettings;
use crate::window_text::PanelTitle;
use std::sync::{Arc, Mutex};

/// What the labels say, as the Feed loop last saw it.
#[derive(Clone, Default, PartialEq)]
pub struct Texts {
    /// Each Panel's title bar (name, tags, and the full label for its hover), in Panel order.
    pub titles: Vec<PanelTitle>,
    /// The Colour Setup line across the top.
    pub top_bar: String,
    /// The status line's choices, longest first (the window picks what fits), and its hover.
    pub status: Vec<String>,
    pub status_hover: String,
    pub notice: Option<String>,
    pub has_picture: bool,
}

/// What one step changed. Nothing set means nothing to redraw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changed {
    /// The displays measured a frame: a new one from the Feed, or the one on screen again.
    pub picture: bool,
    /// The labels say something different.
    pub labels: bool,
}

impl Changed {
    pub fn any(self) -> bool {
        self.picture || self.labels
    }
}

pub struct FeedLoop {
    displays: DisplaySet,
    /// Every Panel's choices, as the window changes them.
    settings: Arc<Mutex<PanelSettings>>,
    conversion: OutputConversion,
    texts: Arc<Mutex<Texts>>,
    /// Displays measure in the Colour Setup's Output Colour Space (the default Measured Space).
    context: DisplayContext,
    chosen: PanelSettings,
    seen_version: u64,
    seen_state_version: u64,
    /// The displays' generation when last looked at: a Panel made or removed configures and measures again.
    seen_generation: u64,
    /// What the Feed's status said when last looked at, and the texts made from it.
    seen: Option<Status>,
    status_text: Vec<String>,
    status_hover: String,
    top_bar: String,
    notice: Option<String>,
    /// Said while the last frame measured could not get all the GPU memory it needed; it comes before the Feed's.
    memory_notice: Option<String>,
    has_picture: bool,
    last: Texts,
    /// Frames put on the GPU and handed to the displays.
    pub scope_frames: u64,
}

impl FeedLoop {
    pub fn new(
        displays: DisplaySet,
        settings: Arc<Mutex<PanelSettings>>,
        conversion: OutputConversion,
    ) -> FeedLoop {
        let chosen = settings.lock().unwrap().clone();
        // Before any frame: every display has its settings, and measures in the default Measured Space.
        displays.configure(&DisplayContext::default(), &chosen);
        let seen_generation = displays.generation();
        FeedLoop {
            displays,
            conversion,
            texts: Arc::new(Mutex::new(Texts::default())),
            context: DisplayContext::default(),
            seen_version: chosen.version(),
            seen_state_version: chosen.state_version(),
            seen_generation,
            chosen,
            settings,
            seen: None,
            status_text: Vec::new(),
            status_hover: String::new(),
            top_bar: String::new(),
            notice: None,
            memory_notice: None,
            has_picture: false,
            last: Texts::default(),
            scope_frames: 0,
        }
    }

    /// The labels the window draws, kept current by [`FeedLoop::step`].
    pub fn texts(&self) -> Arc<Mutex<Texts>> {
        self.texts.clone()
    }

    pub fn displays(&self) -> &DisplaySet {
        &self.displays
    }

    /// Poll the Feed once at `now_ns` and do what that, the settings and the Colour Setup call for.
    pub fn step(
        &mut self,
        monitor: &mut Monitor,
        uploader: &mut FrameUploader,
        now_ns: u64,
    ) -> Changed {
        let (displays, conversion, scope_frames) =
            (&self.displays, &mut self.conversion, &mut self.scope_frames);
        let mut shown = false;
        monitor.poll(now_ns, &mut |frame| {
            uploader.upload(frame, |encoder, gpu_frame| {
                // Every display sees the picture after Output Conversion: one upload, one conversion.
                let Some(picture) = conversion.convert(encoder, gpu_frame) else {
                    return; // short of GPU memory: said in the notice, tried again with the next frame
                };
                displays.on_frame(encoder, picture);
                // Stops False Colour measures scene light: the frame before Output Conversion.
                displays.on_timeline_frame(encoder, gpu_frame);
                *scope_frames += 1;
            });
            shown = true;
        });
        // Idle costs a comparison, nothing more: the slower work below happens only when something it depends on
        // changed (the Feed's status, a menu choice, remembered state, a frame shown).
        let mut labels_stale = shown;
        let status = monitor.status();
        let status_changed = self.seen.as_ref() != Some(status);
        if status_changed {
            self.seen = Some(status.clone());
            labels_stale = true;
            let connected = status.feed == FeedState::Connected;
            self.status_text = if connected {
                status.status_candidates()
            } else {
                Vec::new()
            };
            self.status_hover = if connected {
                status.status_hover()
            } else {
                String::new()
            };
            self.top_bar = status.top_bar();
            self.notice = status.notice();
            self.has_picture = status.picture.is_some();
            let now_context = status.display_context();
            if now_context != self.context {
                self.context = now_context;
                self.displays.configure(&self.context, &self.chosen);
            }
        }
        // A setting changed in a menu measures the frame on screen again, even with Resolve stopped.
        let (version, state_version) = {
            let guard = self.settings.lock().unwrap();
            (guard.version(), guard.state_version())
        };
        if state_version != self.seen_state_version {
            self.seen_state_version = state_version;
            labels_stale = true;
        }
        let generation = self.displays.generation();
        if version != self.seen_version || generation != self.seen_generation {
            self.seen_version = version;
            self.seen_generation = generation;
            self.chosen = self.settings.lock().unwrap().clone();
            self.displays.configure(&self.context, &self.chosen);
            let displays = &self.displays;
            self.conversion.again(uploader.frame(), |encoder, picture| {
                measure(displays, encoder, picture, uploader.frame())
            });
            shown = true;
            labels_stale = true;
        }
        // A Colour Setup that arrives or changes while Resolve is stopped converts the frame on screen.
        if status_changed {
            let setup = status.colour_setup().map(|c| c.setup);
            let displays = &self.displays;
            if self
                .conversion
                .follow(setup.as_ref(), uploader.frame(), |encoder, picture| {
                    measure(displays, encoder, picture, uploader.frame())
                })
            {
                shown = true;
                labels_stale = true;
            }
        }
        // Whatever measured a frame this step made the GPU memory it needed, or says how much it could not get.
        if shown {
            let short = gpu_memory::take().notice();
            if short != self.memory_notice {
                self.memory_notice = short;
                labels_stale = true;
            }
        }
        let mut labels = false;
        if labels_stale {
            let now = Texts {
                titles: self.displays.titles(&self.context, &self.chosen),
                top_bar: self.top_bar.clone(),
                status: self.status_text.clone(),
                status_hover: self.status_hover.clone(),
                notice: self.memory_notice.clone().or_else(|| self.notice.clone()),
                has_picture: self.has_picture,
            };
            if now != self.last {
                *self.texts.lock().unwrap() = now.clone();
                self.last = now;
                labels = true;
            }
        }
        Changed {
            picture: shown,
            labels,
        }
    }
}

/// Hand the frame on screen, converted, to every display, and the Timeline Colour Space frame to those that
/// measure scene light.
fn measure(
    displays: &DisplaySet,
    encoder: &mut wgpu::CommandEncoder,
    picture: &crate::gpu_frame::GpuFrame,
    timeline: Option<&crate::gpu_frame::GpuFrame>,
) {
    displays.on_frame(encoder, picture);
    if let Some(t) = timeline {
        displays.on_timeline_frame(encoder, t);
    }
}
