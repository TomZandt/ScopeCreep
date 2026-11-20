//! The app's view of the Feed: attaches to it, keeps the Sender informed that an app is listening, decides which
//! frames the Picture View shows, and says in plain words what state the Feed is in.
//!
//! It also keeps the Colour Setup: the Sender's manual override arrives with each frame, and the project's is read
//! by script through a [`ProjectReader`], only while Resolve is not playing (ADR 0006).
//!
//! Everything here is driven by [`Monitor::poll`] with an explicit clock, so tests can drive it with synthetic
//! Feed frames and no window (app/tests/feed_seam.rs, app/tests/colour_setup_seam.rs).
use crate::colour_setup::{ColourSetup, KnownColourSetup, Provenance};
use crate::playback_rate::PlaybackRate;
use crate::project::{Outcome, ProjectReader, ProjectReading};
use crate::senders::{Arbiter, SenderKey};
use crate::viewer_gate::{Timing, ViewerGate};
use scopecreep_feed::{FeedReader, FeedStats, Frame, OpenError, SenderInfo};
use std::io::Write;
use std::path::{Path, PathBuf};

/// How often the slower checks run (looking for a Feed, is Resolve still there).
const CHECK_INTERVAL_NS: u64 = 250_000_000;
/// A Sender in another Resolve process is flagged while it rendered within this long.
const GUEST_ACTIVE_NS: u64 = 2_000_000_000;

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
    VersionMismatch {
        found: (u32, u32),
        supported: (u32, u32),
    },
    /// Another app is attached to the Feed.
    Busy {
        pid: u64,
    },
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

/// Project and timeline names of a Sender, when the scripting side has supplied them (ticket #16). A plugin
/// is told neither, so until then they are unknown and the Sender is named by its instance and process.
#[derive(Clone, Debug, PartialEq)]
pub struct SenderNames {
    pub project: String,
    pub timeline: String,
}

/// One Sender instance as the app names it: everything Resolve lets a plugin know about itself.
#[derive(Clone, Debug, PartialEq)]
pub struct SenderSummary {
    /// Process id of the Resolve that runs it.
    pub pid: u64,
    /// 1, 2, 3 ... in order of creation within that Resolve process.
    pub instance: u32,
    /// Host name and version, as the host reports them.
    pub host: String,
    /// Frames per second of the Sender's timeline, 0 = unknown.
    pub frame_rate: f64,
    pub names: Option<SenderNames>,
}

impl SenderSummary {
    /// "Sender 2 (process 4242)", or with names "Sender 2 in Project › Timeline (process 4242)".
    pub fn short_name(&self) -> String {
        match &self.names {
            Some(n) => format!(
                "Sender {} in {} › {} (process {})",
                self.instance, n.project, n.timeline, self.pid
            ),
            None => format!("Sender {} (process {})", self.instance, self.pid),
        }
    }

    /// "Sender 1 · DaVinci Resolve 21.2 · process 4242 · 24 fps timeline", with project and timeline names
    /// after the instance once they are known.
    pub fn long_name(&self) -> String {
        let mut parts = vec![format!("Sender {}", self.instance)];
        if let Some(n) = &self.names {
            parts.push(format!("{} › {}", n.project, n.timeline));
        }
        if !self.host.is_empty() {
            parts.push(self.host.clone());
        }
        parts.push(format!("process {}", self.pid));
        if self.frame_rate > 0.0 {
            parts.push(format!("{} fps timeline", fps_text(self.frame_rate)));
        }
        parts.join(" · ")
    }
}

/// 24 → "24", 23.976 → "23.976", 59.94 → "59.94".
fn fps_text(fps: f64) -> String {
    let s = format!("{fps:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
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
    /// Frames lost since the app attached: `dropped_app_slow + dropped_sender_busy`.
    pub dropped: u64,
    /// Frames the Sender overwrote before the app collected them: the app was too slow.
    pub dropped_app_slow: u64,
    /// Frames the Sender skipped because it could not take them without making Resolve wait.
    pub dropped_sender_busy: u64,
    pub feed_stats: Option<FeedStats>,
    /// Resolve's playback rate in frames per second, measured from the Sender's render times. None while paused.
    pub playback_rate: Option<f64>,
    /// The Sender whose frames the app shows.
    pub driving: Option<SenderSummary>,
    /// Other Senders rendering at the same time, whose frames are not shown.
    pub ignored: Vec<SenderSummary>,
    /// The project as last read by script: Colour Setup, project and timeline names.
    pub project: Option<ProjectReading>,
    pub project_state: ProjectState,
    /// Reads of the project requested so far (diagnostics).
    pub project_reads: u64,
}

impl Status {
    /// The Colour Setup of the picture on screen: the Sender's manual override if it sent one, else the project's.
    pub fn colour_setup(&self) -> Option<KnownColourSetup> {
        let sent = self
            .picture
            .as_ref()
            .and_then(|p| p.sent_colour_setup.clone());
        let project = self
            .project
            .as_ref()
            .and_then(|r| r.colour_setup.clone())
            .map(|setup| KnownColourSetup {
                setup,
                provenance: Provenance::Project,
            });
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

    /// What every display needs to know about the Colour Setup.
    pub fn display_context(&self) -> crate::display::DisplayContext {
        crate::display::DisplayContext {
            colour_setup: self.colour_setup(),
            colour_label: self.colour_label(),
            ..Default::default()
        }
    }

    /// The feeding timeline's name, once known (from the Sender, else the project as last read).
    pub fn timeline_name(&self) -> Option<String> {
        let sender = self
            .driving
            .as_ref()
            .and_then(|d| d.names.as_ref())
            .map(|n| n.timeline.clone());
        sender
            .or_else(|| self.project.as_ref().and_then(|p| p.timeline.clone()))
            .filter(|t| !t.is_empty())
    }

    fn rate_text(&self) -> String {
        match self.playback_rate {
            Some(r) => format!("{r:.2} fps"),
            None => "paused".to_string(),
        }
    }

    /// The drops, short, or with why.
    fn dropped_text(&self, why: bool) -> String {
        let (slow, busy) = (self.dropped_app_slow, self.dropped_sender_busy);
        match (slow, busy, why) {
            (0, 0, _) => "0 dropped".to_string(),
            (s, b, false) => format!("{} dropped", s + b),
            (n, 0, true) => format!("{n} dropped (app too slow)"),
            (0, n, true) => format!("{n} dropped (Sender busy)"),
            (s, b, true) => format!("{} dropped ({s} app too slow, {b} Sender busy)", s + b),
        }
    }

    /// The status line's choices, in order of priority: playback and performance only (rate, dropped frames and
    /// why, the feeding timeline). Lower-priority parts drop first (the timeline, then why frames dropped), whole
    /// parts only, so rate and drops stay visible at any width the window can have.
    pub fn status_candidates(&self) -> Vec<String> {
        let sep = crate::window_text::SEP;
        let (rate, why, short) = (
            self.rate_text(),
            self.dropped_text(true),
            self.dropped_text(false),
        );
        let mut all = Vec::new();
        if let Some(timeline) = self.timeline_name() {
            all.push(format!("{rate}{sep}{why}{sep}{timeline}"));
        }
        all.push(format!("{rate}{sep}{why}"));
        all.push(format!("{rate}{sep}{short}"));
        all.dedup();
        all
    }

    /// The status line: playback rate, dropped frames and why, and the timeline feeding the Window.
    pub fn status_line(&self) -> String {
        self.status_candidates().remove(0)
    }

    /// The status line's hover: what the line leaves out. The Sender driving the Feed with its instance, process,
    /// Resolve version and timeline rate, any others, the project, and the Feed.
    pub fn status_hover(&self) -> String {
        let mut lines = Vec::new();
        match &self.driving {
            Some(d) => lines.push(d.long_name()),
            None => lines.push("No Sender is driving the Feed".to_string()),
        }
        for other in &self.ignored {
            lines.push(format!("Not shown: {}", other.short_name()));
        }
        if let Some(p) = &self.project {
            lines.push(format!(
                "Project {}{}  ·  {}",
                p.project,
                p.timeline
                    .as_ref()
                    .map_or(String::new(), |t| format!(" › {t}")),
                p.product
            ));
        }
        lines.push(format!(
            "Feed {}  ·  {} shown, {} dropped",
            self.path.display(),
            self.viewer_frames,
            self.dropped
        ));
        lines.join("\n")
    }

    /// The Colour Setup line for the top of the Window.
    pub fn top_bar(&self) -> String {
        let unknown = match self.project_state {
            ProjectState::NotYetRead => "Colour Setup read when playback stops",
            _ => "Colour Setup unknown",
        };
        crate::window_text::top_bar(self.colour_setup().as_ref(), unknown)
    }

    /// One sentence when there is something the user needs to know, else nothing.
    pub fn notice(&self) -> Option<String> {
        match &self.feed {
            FeedState::NoSender => {
                Some("No Sender. Put ScopeCreep on the last timeline node in Resolve.".into())
            }
            FeedState::VersionMismatch { found, supported } => Some(format!(
                "The Sender speaks Feed version {}.{} and this app reads version {}.x. Install the Sender and the app from the same release.",
                found.0, found.1, supported.0
            )),
            FeedState::Busy { pid } => Some(format!(
                "Another scopecreep app (process {pid}) is already showing this Feed."
            )),
            FeedState::NotAFeed => Some(format!(
                "The file at {} is not a Feed.",
                self.path.display()
            )),
            FeedState::Unreadable(why) => Some(format!(
                "The Feed at {} cannot be opened: {why}.",
                self.path.display()
            )),
            FeedState::Connected => match (&self.sender_error, &self.picture) {
                (Some(e), _) => Some(format!("Sender: {e}")),
                (None, None) => self.conflict_notice().or_else(|| {
                    Some("Waiting for frames. Play or move the playhead in Resolve.".into())
                }),
                (None, Some(_)) => self.conflict_notice().or_else(|| {
                    match (&self.project_state, self.colour_setup()) {
                        (ProjectState::Problem(why), None) => Some(format!(
                            "Colour Setup unknown: {why}. Or set it on the Sender node in Resolve."
                        )),
                        _ => None,
                    }
                }),
            },
        }
    }
}

impl Status {
    fn conflict_notice(&self) -> Option<String> {
        let first = self.ignored.first()?;
        let Some(driving) = &self.driving else {
            // Only a guest is rendering: the Feed's own Resolve has sent nothing yet.
            let owner = self.sender.as_ref().map_or(0, |s| s.pid);
            return Some(format!(
                "A Sender in another Resolve (process {}) is active, but the Feed belongs to the Resolve that loaded its Sender first (process {owner}). Close the other Resolve or remove its Sender.",
                first.pid
            ));
        };
        let others: Vec<String> = self.ignored.iter().map(SenderSummary::short_name).collect();
        let lead = if others.len() == 1 {
            "Another Sender is active"
        } else {
            "Other Senders are active"
        };
        let advice = if self.ignored.iter().any(|s| s.pid != driving.pid) {
            "Only the Resolve that loaded its Sender first sends frames: close the other Resolve or remove its Sender."
        } else {
            "Keep one Sender, on the last timeline node."
        };
        Some(format!(
            "{lead}: {}. Showing {} only. {advice}",
            others.join(", "),
            driving.short_name()
        ))
    }
}

pub struct Monitor {
    reader: Option<FeedReader>,
    gate: ViewerGate<Frame>,
    status: Status,
    next_open_ns: u64,
    next_check_ns: u64,
    to_show: Vec<Frame>,
    rate: PlaybackRate,
    /// Sequence number of the last frame read, and frames the Feed lost since the last Viewer Frame.
    last_sequence: Option<u64>,
    lost_pending: u64,
    /// Drop counters as they were when the app attached to the current Feed file (overwritten, busy), and the
    /// drops counted in earlier Feed files since the app started.
    drops_baseline: Option<(u64, u64)>,
    drops_before: (u64, u64),
    arbiter: Arbiter,
    /// Senders in other Resolve processes (guests in the Feed's registry), as of the last slow check.
    guests: Vec<SenderSummary>,
    /// Diagnostics: one line per frame read and per frame shown.
    trace: Option<Box<dyn Write + Send>>,
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
                dropped_app_slow: 0,
                dropped_sender_busy: 0,
                feed_stats: None,
                playback_rate: None,
                driving: None,
                ignored: Vec::new(),
                project: None,
                project_reads: 0,
                project_state: if project_reader.is_some() {
                    ProjectState::NotYetRead
                } else {
                    ProjectState::NoReader
                },
            },
            next_open_ns: 0,
            next_check_ns: 0,
            to_show: Vec::new(),
            rate: PlaybackRate::default(),
            last_sequence: None,
            lost_pending: 0,
            drops_baseline: None,
            drops_before: (0, 0),
            arbiter: Arbiter::default(),
            guests: Vec::new(),
            trace: None,
            project_reader,
            read_started_ns: None,
            next_read_ns: 0,
            last_render_ns: 0,
            quiet_needed_ns: QUIET_BEFORE_READ_NS,
        }
    }

    /// Write one line for every frame read from the Feed and every frame shown (diagnostics: how the Sender's
    /// renders were paced and what the app made of them).
    pub fn set_trace(&mut self, trace: Box<dyn Write + Send>) {
        self.trace = Some(trace);
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
                self.rate.reset();
                self.arbiter.reset();
                self.guests.clear();
                self.last_sequence = None;
                if let (Some(base), Some(stats)) =
                    (self.drops_baseline.take(), self.status.feed_stats)
                {
                    // Counted drops stay counted; the next Feed file is measured from its own starting values.
                    self.drops_before.0 += stats.drops_overwritten.saturating_sub(base.0);
                    self.drops_before.1 += stats.drops_busy.saturating_sub(base.1);
                }
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
                        OpenError::VersionMismatch { found, supported } => {
                            FeedState::VersionMismatch { found, supported }
                        }
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
            self.status.feed = if reader.sender_alive() {
                FeedState::Connected
            } else {
                FeedState::NoSender
            };
            self.status.sender_error = reader.sender_error();
            // Guests: registered instances of other Resolve processes that rendered lately and still run.
            let owner = self.status.sender.as_ref().map_or(0, |s| s.pid);
            self.guests = reader
                .registry()
                .into_iter()
                .filter(|e| {
                    e.pid != owner
                        && e.last_render_ns != 0
                        && now_ns.saturating_sub(e.last_render_ns) < GUEST_ACTIVE_NS
                })
                .filter(|e| scopecreep_feed::process_alive(e.pid))
                .map(|e| SenderSummary {
                    pid: e.pid,
                    instance: e.instance,
                    host: e.host,
                    frame_rate: e.frame_rate,
                    names: None,
                })
                .collect();
        }
        let sender_pid = self.status.sender.as_ref().map_or(0, |s| s.pid);
        while let Some(frame) = reader.next_frame() {
            if let Some(prev) = self.last_sequence {
                self.lost_pending += frame.sequence().saturating_sub(prev + 1);
            }
            self.last_sequence = Some(frame.sequence());
            let key = SenderKey {
                pid: sender_pid,
                instance: frame.sender_instance(),
            };
            let decision = self
                .arbiter
                .frame(key, frame.render_ns(), frame.frame_rate());
            if let Some(trace) = &mut self.trace {
                let _ = writeln!(
                    trace,
                    "frame seq={} instance={} time={} rate={} render_ns={} publish_ns={} read_ns={now_ns} drives={}",
                    frame.sequence(),
                    frame.sender_instance(),
                    frame.frame_time(),
                    frame.frame_rate(),
                    frame.render_ns(),
                    frame.publish_ns(),
                    decision.drives
                );
            }
            if decision.driver_changed {
                self.gate.reset(); // a frame held back from the previous driver is not shown
                self.rate.reset();
            }
            if !decision.drives {
                continue; // the frame's slot goes back to the Sender here
            }
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
                    ColourSetup::from_feed(&sent).map(|setup| KnownColourSetup {
                        setup,
                        provenance: sent.provenance,
                    })
                }),
            });
            if let Some(trace) = &mut self.trace {
                let _ = writeln!(
                    trace,
                    "shown seq={} time={} at_ns={now_ns}",
                    frame.sequence(),
                    frame.frame_time()
                );
            }
            self.rate.viewer_frame(
                frame.render_ns(),
                frame.frame_time(),
                frame.frame_rate(),
                std::mem::take(&mut self.lost_pending),
            );
            show(&frame);
        }
        let stats = reader.stats();
        let counts = self.gate.counts();
        let base = *self
            .drops_baseline
            .get_or_insert((stats.drops_overwritten, stats.drops_busy));
        self.status.dropped_app_slow =
            self.drops_before.0 + stats.drops_overwritten.saturating_sub(base.0);
        self.status.dropped_sender_busy =
            self.drops_before.1 + stats.drops_busy.saturating_sub(base.1);
        self.status.dropped = self.status.dropped_app_slow + self.status.dropped_sender_busy;
        self.status.feed_stats = Some(stats);
        self.status.viewer_frames = counts.viewer_frames;
        self.status.repeat_renders = counts.repeat_renders;
        self.status.cache_renders_ignored = counts.cache_renders_ignored;
        self.status.playback_rate = self.rate.rate(now_ns);
        let host = self
            .status
            .sender
            .as_ref()
            .map(|s| s.host.clone())
            .unwrap_or_default();
        let summary = |key: SenderKey, arbiter: &Arbiter| SenderSummary {
            pid: key.pid,
            instance: key.instance,
            host: host.clone(),
            frame_rate: arbiter.frame_rate(key).unwrap_or(0.0),
            names: None,
        };
        self.status.driving = self.arbiter.driver().map(|k| summary(k, &self.arbiter));
        // The scripting helper reads the project and timeline open in Resolve, which is what the driving Sender
        // renders (Resolve renders only the timeline on screen).
        if let (Some(d), Some(p)) = (&mut self.status.driving, &self.status.project)
            && let Some(timeline) = &p.timeline
        {
            d.names = Some(SenderNames {
                project: p.project.clone(),
                timeline: timeline.clone(),
            });
        }
        self.status.ignored = self
            .arbiter
            .flagged(now_ns)
            .into_iter()
            .map(|k| summary(k, &self.arbiter))
            .collect();
        self.status.ignored.extend(self.guests.iter().cloned());
        self.last_render_ns = self.last_render_ns.max(stats.last_render_ns);
        let sender_here = self.status.feed == FeedState::Connected;
        self.poll_project(now_ns, sender_here);
    }

    /// Collect a finished read, give up on a stuck one, and start a new one when it is safe: a Sender is here (so
    /// its renders tell whether Resolve is playing) and it has rendered nothing for [`QUIET_BEFORE_READ_NS`].
    fn poll_project(&mut self, now_ns: u64, sender_here: bool) {
        let Some(project_reader) = &mut self.project_reader else {
            return;
        };
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
                self.status.project_state =
                    ProjectState::Problem("Resolve did not answer by script".to_string());
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
