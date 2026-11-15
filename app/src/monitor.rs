//! The app's view of the Feed: attaches to it, keeps the Sender informed that an app is listening, decides which
//! frames the Picture View shows, and says in plain words what state the Feed is in.
//!
//! Everything here is driven by [`Monitor::poll`] with an explicit clock, so tests can drive it with synthetic
//! Feed frames and no window (app/tests/feed_seam.rs).
use crate::viewer_gate::{Timing, ViewerGate};
use scopecreep_feed::{FeedReader, FeedStats, Frame, OpenError, SenderInfo};
use std::path::{Path, PathBuf};

/// How often the slower checks run (looking for a Feed, is Resolve still there).
const CHECK_INTERVAL_NS: u64 = 250_000_000;

/// Whether the app has a Feed, and if not, why not.
#[derive(Clone, Debug, PartialEq)]
pub enum FeedState {
    /// No Feed, or the Sender's host is no longer running.
    NoSender,
    /// Attached to a Sender's Feed. Frames may or may not be flowing; no frames is not an error.
    Connected,
    VersionMismatch { found: (u32, u32), supported: (u32, u32) },
    /// Another app is attached to the Feed.
    Busy { pid: u64 },
    NotAFeed,
    Unreadable(String),
}

/// The frame the Picture View is showing.
#[derive(Clone, Debug, PartialEq)]
pub struct PictureInfo {
    pub width: u32,
    pub height: u32,
    pub frame_time: f64,
    pub frame_rate: f64,
    pub sender_instance: u32,
    pub sequence: u64,
    pub render_ns: u64,
}

/// What the app knows about the Feed right now. Labels are derived from this and nothing else.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub path: PathBuf,
    pub feed: FeedState,
    pub sender: Option<SenderInfo>,
    pub sender_error: Option<String>,
    pub picture: Option<PictureInfo>,
    /// Distinct Viewer Frames shown.
    pub viewer_frames: u64,
    /// Renders of a frame that had just been rendered (shown, not counted as new frames).
    pub repeat_renders: u64,
    /// Renders kept out of the Picture View as background cache renders.
    pub cache_renders_ignored: u64,
    /// Frames the Sender gave up on because the app was not keeping up.
    pub dropped: u64,
    pub feed_stats: Option<FeedStats>,
}

impl Status {
    /// The quiet label of the Picture View.
    pub fn picture_label(&self) -> String {
        match &self.picture {
            // The values are shown exactly as the Sender received them; until the Colour Setup is known
            // (ticket #16) that means Timeline Colour Space values, and the label must not pretend otherwise.
            Some(p) => format!("Clean picture  ·  {} × {}  ·  as sent, no Output Conversion", p.width, p.height),
            None => "Clean picture".to_string(),
        }
    }

    /// One sentence when there is something the user needs to know, else nothing.
    pub fn notice(&self) -> Option<String> {
        match &self.feed {
            FeedState::NoSender => Some("No Sender. Put the scopecreep Sender on the last timeline node in Resolve.".into()),
            FeedState::VersionMismatch { found, supported } => Some(format!(
                "The Sender speaks Feed version {}.{} and this app reads version {}.x. Install the Sender and the app from the same release.",
                found.0, found.1, supported.0
            )),
            FeedState::Busy { pid } => Some(format!("Another scopecreep app (process {pid}) is already showing this Feed.")),
            FeedState::NotAFeed => Some(format!("The file at {} is not a Feed.", self.path.display())),
            FeedState::Unreadable(why) => Some(format!("The Feed at {} cannot be opened: {why}.", self.path.display())),
            FeedState::Connected => match (&self.sender_error, &self.picture) {
                (Some(e), _) => Some(format!("Sender: {e}")),
                (None, None) => Some("Waiting for frames. Play or move the playhead in Resolve.".into()),
                (None, Some(_)) => None,
            },
        }
    }
}

pub struct Monitor {
    reader: Option<FeedReader>,
    gate: ViewerGate<Frame>,
    status: Status,
    next_open_ns: u64,
    next_check_ns: u64,
    to_show: Vec<Frame>,
}

impl Monitor {
    /// Watch the Feed at `path`. Nothing is opened until the first [`Monitor::poll`].
    pub fn new(path: impl AsRef<Path>) -> Monitor {
        Monitor {
            reader: None,
            gate: ViewerGate::default(),
            status: Status {
                path: path.as_ref().to_path_buf(),
                feed: FeedState::NoSender,
                sender: None,
                sender_error: None,
                picture: None,
                viewer_frames: 0,
                repeat_renders: 0,
                cache_renders_ignored: 0,
                dropped: 0,
                feed_stats: None,
            },
            next_open_ns: 0,
            next_check_ns: 0,
            to_show: Vec::new(),
        }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Do everything that is due at `now_ns` (the monotonic clock, [`scopecreep_feed::now_ns`]): attach or
    /// re-attach, renew the heartbeat, and take new frames. `show` is called, oldest first, with each frame the
    /// Picture View should now display; the frame's slot goes back to the Sender when `show` returns.
    /// Call this often (every millisecond or so): the Feed's ring is short.
    pub fn poll(&mut self, now_ns: u64, show: &mut dyn FnMut(&Frame)) {
        let slow_check = now_ns >= self.next_check_ns;
        if slow_check {
            self.next_check_ns = now_ns + CHECK_INTERVAL_NS;
        }
        if let Some(reader) = &self.reader {
            reader.heartbeat(now_ns);
            if reader.retired() || (slow_check && reader.replaced()) {
                self.reader = None;
                self.gate.reset();
                self.next_open_ns = 0;
            }
        }
        if self.reader.is_none() && now_ns >= self.next_open_ns {
            self.next_open_ns = now_ns + CHECK_INTERVAL_NS;
            match FeedReader::open(&self.status.path) {
                Ok(reader) => {
                    reader.heartbeat(now_ns);
                    self.status.feed = FeedState::Connected;
                    self.status.sender = Some(reader.sender());
                    self.reader = Some(reader);
                    self.next_check_ns = now_ns; // look at the new Feed's details straight away
                }
                Err(e) => {
                    self.status.sender = None;
                    self.status.sender_error = None;
                    self.status.feed = match e {
                        OpenError::NoFeed => FeedState::NoSender,
                        OpenError::NotAFeed => FeedState::NotAFeed,
                        OpenError::VersionMismatch { found, supported } => FeedState::VersionMismatch { found, supported },
                        OpenError::Busy { pid } => FeedState::Busy { pid },
                        OpenError::Io(e) => FeedState::Unreadable(e.to_string()),
                    };
                }
            }
        }
        let Some(reader) = &mut self.reader else { return };
        if slow_check || now_ns >= self.next_check_ns {
            self.next_check_ns = now_ns + CHECK_INTERVAL_NS;
            self.status.feed = if reader.sender_alive() { FeedState::Connected } else { FeedState::NoSender };
            self.status.sender_error = reader.sender_error();
        }
        while let Some(frame) = reader.next_frame() {
            let timing = Timing {
                sender_instance: frame.sender_instance(),
                frame_time: frame.frame_time(),
                frame_rate: frame.frame_rate(),
                render_ns: frame.render_ns(),
            };
            self.gate.offer(timing, frame, now_ns, &mut self.to_show);
        }
        self.gate.tick(now_ns, &mut self.to_show);
        for frame in self.to_show.drain(..) {
            self.status.picture = Some(PictureInfo {
                width: frame.width(),
                height: frame.height(),
                frame_time: frame.frame_time(),
                frame_rate: frame.frame_rate(),
                sender_instance: frame.sender_instance(),
                sequence: frame.sequence(),
                render_ns: frame.render_ns(),
            });
            show(&frame);
        }
        let stats = reader.stats();
        let counts = self.gate.counts();
        self.status.dropped = stats.dropped();
        self.status.feed_stats = Some(stats);
        self.status.viewer_frames = counts.viewer_frames;
        self.status.repeat_renders = counts.repeat_renders;
        self.status.cache_renders_ignored = counts.cache_renders_ignored;
    }
}
