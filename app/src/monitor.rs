//! The app's view of the Feed: attaches to it, keeps the Sender informed that an app is listening, decides which
//! frames the Picture View shows, and says in plain words what state the Feed is in.
//!
//! It also keeps the Colour Setup: the Sender's manual override arrives with each frame, and the project's is read
//! by script through a [`ProjectReader`], only while Resolve is not playing (ADR 0006).
//!
//! Everything here is driven by [`Monitor::poll`] with an explicit clock, so tests can drive it with synthetic
//! Feed frames and no window (app/tests/feed_seam.rs, app/tests/colour_setup_seam.rs).
use crate::colour_setup::{ColourSetup, KnownColourSetup, Provenance};
use crate::project::{Outcome, ProjectReader, ProjectReading};
use crate::viewer_gate::{Timing, ViewerGate};
use scopecreep_feed::{FeedReader, FeedStats, Frame, OpenError, SenderInfo};
use std::path::{Path, PathBuf};

/// How often the slower checks run (looking for a Feed, is Resolve still there).
const CHECK_INTERVAL_NS: u64 = 250_000_000;

/// Resolve is taken to be stopped when the Sender has rendered nothing for this long. Only then is it asked
/// anything by script: scripting calls made during playback block and starve Resolve (Stage 0, ADR 0005).
pub const QUIET_BEFORE_READ_NS: u64 = 1_000_000_000;
/// While stopped, the project is read again this often, so a changed setting shows within a few seconds.
pub const REREAD_INTERVAL_NS: u64 = 2_000_000_000;
/// A read not answered in this time is abandoned (the helper is killed and started again).
pub const READ_TIMEOUT_NS: u64 = 5_000_000_000;
/// When the Sender rendered while a read was in flight (playback started under it), the next read waits for this
/// much quiet instead: in Resolve testing such a collision was followed by a render stall of over a second in the
/// middle of playback, which [`QUIET_BEFORE_READ_NS`] alone would take for a stop.
pub const QUIET_AFTER_COLLISION_NS: u64 = 10_000_000_000;
/// After a failed or abandoned read, wait this long before asking again.
pub const RETRY_AFTER_PROBLEM_NS: u64 = 5_000_000_000;

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
    /// The Colour Setup the Sender sent with this frame (its manual override), if any.
    pub sent_colour_setup: Option<KnownColourSetup>,
}

/// Where reading the project by script stands.
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectState {
    /// This app has no way to read the project (no reader).
    NoReader,
    /// Not read yet: it is read once Resolve stops playing.
    NotYetRead,
    /// The last read succeeded.
    Read,
    /// The last read failed; the message says why.
    Problem(String),
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
    /// The project as last read by script: Colour Setup, project and timeline names.
    pub project: Option<ProjectReading>,
    pub project_state: ProjectState,
    /// Reads of the project requested so far (diagnostics).
    pub project_reads: u64,
}

impl Status {
    /// The Colour Setup of the picture on screen: the Sender's manual override if it sent one, else the project's.
    pub fn colour_setup(&self) -> Option<KnownColourSetup> {
        let sent = self.picture.as_ref().and_then(|p| p.sent_colour_setup.clone());
        let project = self.project.as_ref().and_then(|r| r.colour_setup.clone()).map(|setup| KnownColourSetup { setup, provenance: Provenance::Project });
        match sent {
            Some(s) if s.provenance == Provenance::Manual => Some(s),
            s => project.or(s),
        }
    }

    /// The Colour Setup part of the label.
    fn colour_label(&self) -> String {
        match (self.colour_setup(), &self.project_state) {
            (Some(c), _) => c.label(),
            (None, ProjectState::NotYetRead) => "Colour Setup read when playback stops".to_string(),
            (None, _) => "Colour Setup unknown".to_string(),
        }
    }

    /// The quiet label of the Picture View.
    pub fn picture_label(&self) -> String {
        match &self.picture {
            // The values are shown exactly as the Sender received them; until the Colour Setup is known
            // (ticket #16) that means Timeline Colour Space values, and the label must not pretend otherwise.
            Some(p) => format!("Clean picture  ·  {} × {}  ·  {}  ·  as sent, no Output Conversion", p.width, p.height, self.colour_label()),
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
                (None, Some(_)) => match (&self.project_state, self.colour_setup()) {
                    (ProjectState::Problem(why), None) => Some(format!("Colour Setup unknown: {why}. Or set it on the Sender node in Resolve.")),
                    _ => None,
                },
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
    project_reader: Option<Box<dyn ProjectReader>>,
    /// When the read in flight was requested.
    read_started_ns: Option<u64>,
    next_read_ns: u64,
    /// The latest render the app has seen evidence of (Feed header or frame).
    last_render_ns: u64,
    /// Quiet needed before the next read: [`QUIET_BEFORE_READ_NS`], or longer after a collision with playback.
    quiet_needed_ns: u64,
}

impl Monitor {
    /// Watch the Feed at `path`. Nothing is opened until the first [`Monitor::poll`].
    pub fn new(path: impl AsRef<Path>) -> Monitor {
        Self::make(path, None)
    }

    /// As [`Monitor::new`], reading the project's Colour Setup with `reader` while Resolve is stopped.
    pub fn with_project_reader(path: impl AsRef<Path>, reader: Box<dyn ProjectReader>) -> Monitor {
        Self::make(path, Some(reader))
    }

    fn make(path: impl AsRef<Path>, project_reader: Option<Box<dyn ProjectReader>>) -> Monitor {
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
                project: None,
                project_reads: 0,
                project_state: if project_reader.is_some() { ProjectState::NotYetRead } else { ProjectState::NoReader },
            },
            next_open_ns: 0,
            next_check_ns: 0,
            to_show: Vec::new(),
            project_reader,
            read_started_ns: None,
            next_read_ns: 0,
            last_render_ns: 0,
            quiet_needed_ns: QUIET_BEFORE_READ_NS,
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
                    // Until now the Sender may have been an identity that Resolve does not render, so its silence
                    // says nothing about playback yet: start the quiet period from here.
                    self.last_render_ns = self.last_render_ns.max(now_ns);
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
        let Some(reader) = &mut self.reader else {
            self.poll_project(now_ns, false);
            return;
        };
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
            self.last_render_ns = self.last_render_ns.max(frame.render_ns());
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
                sent_colour_setup: frame.colour_setup().and_then(|sent| {
                    ColourSetup::from_feed(&sent).map(|setup| KnownColourSetup { setup, provenance: sent.provenance })
                }),
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
        self.last_render_ns = self.last_render_ns.max(stats.last_render_ns);
        let sender_here = self.status.feed == FeedState::Connected;
        self.poll_project(now_ns, sender_here);
    }

    /// Collect a finished read, give up on a stuck one, and start a new one when it is safe: a Sender is here (so
    /// its renders tell whether Resolve is playing) and it has rendered nothing for [`QUIET_BEFORE_READ_NS`].
    fn poll_project(&mut self, now_ns: u64, sender_here: bool) {
        let Some(project_reader) = &mut self.project_reader else { return };
        if let Some(started) = self.read_started_ns {
            if self.last_render_ns > started {
                self.quiet_needed_ns = QUIET_AFTER_COLLISION_NS; // Resolve rendered while being asked
            }
            if let Some(answer) = project_reader.take_answer() {
                self.read_started_ns = None;
                match Outcome::from_answer(&answer) {
                    Outcome::Read(reading) => {
                        self.status.project = Some(*reading);
                        self.status.project_state = ProjectState::Read;
                        self.next_read_ns = now_ns + REREAD_INTERVAL_NS;
                    }
                    Outcome::Gone(why) => {
                        self.status.project = None;
                        self.status.project_state = ProjectState::Problem(why);
                        self.next_read_ns = now_ns + RETRY_AFTER_PROBLEM_NS;
                    }
                    Outcome::Failed(why) => {
                        self.status.project_state = ProjectState::Problem(why);
                        self.next_read_ns = now_ns + RETRY_AFTER_PROBLEM_NS;
                    }
                }
            } else if now_ns.saturating_sub(started) > READ_TIMEOUT_NS {
                project_reader.abandon();
                self.read_started_ns = None;
                self.status.project_state = ProjectState::Problem("Resolve did not answer by script".to_string());
                self.next_read_ns = now_ns + RETRY_AFTER_PROBLEM_NS;
            }
            return;
        }
        let quiet = now_ns.saturating_sub(self.last_render_ns) >= self.quiet_needed_ns;
        if sender_here && quiet && now_ns >= self.next_read_ns {
            self.quiet_needed_ns = QUIET_BEFORE_READ_NS;
            project_reader.request();
            self.status.project_reads += 1;
            self.read_started_ns = Some(now_ns);
        }
    }
}
