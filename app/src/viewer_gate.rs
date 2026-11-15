//! Which Feed frames are **Viewer Frames**: the pacing gate.
//!
//! Resolve flags thumbnail renders (the Sender never feeds those) but not background cache renders or repeat
//! renders, and a plugin cannot see the playhead (Stage 0, ticket #11). So the Sender feeds every other render
//! and this gate decides from pacing alone, on the Sender's clock (`render_ns`). It is the doorman watching people
//! arrive: a steady queue is playback, one person turning up is a scrub or a step, and a coach party pouring
//! through the door in numbered order, faster than any queue can move, is a cache fill.
//!
//! Frames are grouped into **runs**: frames whose numbers follow on (one to three apart, in one direction) from
//! the same Sender instance with no gap longer than a hitch. What gives a cache fill away is its average pace:
//! it renders as fast as the GPU allows (in Resolve 21.2 at HD, 5 to 12 ms a frame, often two frames at once
//! from two threads), while playback never runs faster than the timeline rate and a user stepping with a held
//! arrow key steps no faster than key repeat (about 30 ms). A run whose average interval over its last
//! [`WINDOW`] frames (at least [`MIN_CACHE_FILL_RUN`]) is below both 0.83 of a period and [`FILL_INTERVAL_NS`] is
//! being a cache fill, provided the run is also **ahead of its own clock**: its newest frame came more than a
//! period earlier than any frame before it in the run said it was due. Playback can run late and then catch up
//! (frames bunched after a stall), but never ahead. The judgement is about the recent frames, not the whole run, and it can be withdrawn: a
//! cold playback start in Resolve 21.2 renders its first second out of order and in bursts that look like a fill,
//! then settles; once the last [`WINDOW`] frames came at the frame rate (at least 0.9 of a period each on
//! average) the run is playback again and shown.
//!
//! - **Repeat render** (same Sender instance, same frame number as the previous render): the picture is updated,
//!   because a grade may have changed, but it is not a new Viewer Frame.
//! - **A new run is verified first**: its newest frame is held back (nothing is shown) until the run has lasted
//!   [`verify_ns`] without being a cache fill, its recent frames arriving one at a time (then it is playback, and from then on every frame is shown at once),
//!   or it ends (then it was the user, a step or a scrub, and its newest frame is shown), or it turns out to be a
//!   cache fill (then none of it is shown and the Picture View keeps the last Viewer Frame).
//! - A run ends when a frame does not follow on. When nothing follows for [`hold_ns`] its held frame is shown (a
//!   step or the end of a scrub), but the run is still judged if it carries on.
//!
//! - **Fragments of a fill.** Resolve fills its cache from two threads: every so often one renders a few frames
//!   a little way back, which cuts the fill's run short. A run that starts within [`FRAGMENT_GAP_NS`] of a
//!   frame judged to be a fill, and ends without being verified as playback, was part of the fill too: its held
//!   frame is not shown.
//!
//! The only delay the gate adds is at the start of a run: up to [`verify_ns`] (three periods, at least 60 ms)
//! for playback, [`hold_ns`] (a period, at least 40 ms) for a single step.
//!
//! Known limit: a cache fill that renders no faster than about 0.83 of real time (heavy grades, UHD at 50 or
//! 60 fps) cannot be told from playback by pacing; it is shown. Telling them apart there needs the playhead,
//! which only the scripting side can supply (ticket #16).

/// A cache fill is recognised once its run is at least this many frames long.
pub const MIN_CACHE_FILL_RUN: u64 = 4;
/// A cache fill averages less than this per frame (and less than 0.83 of a period).
pub const FILL_INTERVAL_NS: u64 = 20_000_000;
/// The longest gap inside a run (Resolve stalls about 50 ms now and then at 60 fps).
pub const MAX_HITCH_NS: u64 = 250_000_000;
/// A run that starts this soon after a fill's frame, and is never verified as playback, is part of the fill.
pub const FRAGMENT_GAP_NS: u64 = 100_000_000;
/// How many of a run's newest frames its pace is judged on.
pub const WINDOW: usize = 8;

/// What the gate needs to know about a frame.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub sender_instance: u32,
    pub frame_time: f64,
    pub frame_rate: f64,
    pub render_ns: u64,
}

/// Counts of what the gate has decided so far.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GateCounts {
    /// Distinct frames shown.
    pub viewer_frames: u64,
    /// Renders of a frame that had just been rendered.
    pub repeat_renders: u64,
    /// Renders kept out of the Picture View because they were, or looked like the start of, a cache fill.
    pub cache_renders_ignored: u64,
}

fn period_ns(frame_rate: f64) -> u64 {
    (1e9 / if frame_rate >= 1.0 { frame_rate } else { 24.0 }) as u64
}

/// How long a new run is watched before it counts as playback.
pub fn verify_ns(period: u64) -> u64 {
    (3 * period).max(60_000_000)
}

/// How long a run waits for its next frame before it is over.
pub fn hold_ns(period: u64) -> u64 {
    period.max(40_000_000)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    /// New: its newest frame is held back.
    Verifying,
    /// Playback (or a user's run already shown): every frame shown at once.
    Showing,
    /// A cache fill: nothing shown.
    Fill,
}

struct Run {
    instance: u32,
    period: u64,
    first_time: f64,
    first_ns: u64,
    /// The run's clock: the earliest of (render time minus frames since the run's first frame times the period)
    /// over the run's first frame and the frames older than the recent window. A frame more than a period below it is ahead of the clock.
    clock: f64,
    /// (frame number, render time) of the newest frames, oldest first, at most [`WINDOW`].
    recent: std::collections::VecDeque<(f64, u64)>,
    last_time: f64,
    last_ns: u64,
    /// +1 or -1 once known.
    direction: f64,
    state: State,
    /// Started right after a fill's frame: unless it is verified as playback, it is part of the fill.
    fragment: bool,
}

impl Run {
    /// Does a frame follow on from this run?
    fn continues(&self, t: &Timing) -> bool {
        let step = (t.frame_time - self.last_time) * if self.direction == 0.0 { 1.0 } else { self.direction };
        let step = if self.direction == 0.0 { step.abs() } else { step };
        t.sender_instance == self.instance
            && (1.0..=3.0).contains(&step)
            && t.render_ns >= self.last_ns
            && t.render_ns - self.last_ns <= (2 * self.period).max(MAX_HITCH_NS)
    }

    /// Frames covered and time taken by the recent frames.
    fn recent_pace(&self) -> (f64, f64) {
        let (&(t0, n0), &(t1, n1)) = (self.recent.front().unwrap(), self.recent.back().unwrap());
        ((t1 - t0).abs(), n1.saturating_sub(n0) as f64)
    }

    fn offset(&self, frame_time: f64, render_ns: u64) -> f64 {
        render_ns as f64 - (frame_time - self.first_time).abs() * self.period as f64
    }

    /// Are the recent frames coming faster than any playback, and ahead of the run's clock?
    fn is_fill(&self) -> bool {
        let (frames, elapsed) = self.recent_pace();
        let limit = (self.period as f64 * 0.83).min(FILL_INTERVAL_NS as f64);
        frames >= (MIN_CACHE_FILL_RUN - 1) as f64
            && elapsed < frames * limit
            && self.offset(self.last_time, self.last_ns) < self.clock - self.period as f64
    }

    /// Have the last [`WINDOW`] frames come at the frame rate, one at a time? A fill renders in pairs (two
    /// frames under a few milliseconds apart) even where its average pace drops to real time; playback does not.
    fn is_steady(&self) -> bool {
        let (frames, elapsed) = self.recent_pace();
        self.recent.len() == WINDOW && elapsed >= frames * 0.9 * self.period as f64 && self.one_at_a_time()
    }

    /// No two of the recent frames came less than a quarter period apart.
    fn one_at_a_time(&self) -> bool {
        self.recent.iter().zip(self.recent.iter().skip(1)).all(|(a, b)| b.1.saturating_sub(a.1) >= self.period / 4)
    }

    fn push(&mut self, frame_time: f64, render_ns: u64) {
        self.last_time = frame_time;
        self.last_ns = render_ns;
        if self.recent.len() == WINDOW
            && let Some((t, n)) = self.recent.pop_front()
        {
            self.clock = self.clock.min(self.offset(t, n));
        }
        self.recent.push_back((frame_time, render_ns));
    }
}

/// Holds at most one frame of type `T` back at a time.
pub struct ViewerGate<T> {
    run: Option<Run>,
    /// The newest frame of a run being verified.
    held: Option<T>,
    /// Render time of the newest frame judged to be part of a cache fill.
    last_fill_ns: Option<u64>,
    counts: GateCounts,
}

impl<T> Default for ViewerGate<T> {
    fn default() -> Self {
        ViewerGate { run: None, held: None, last_fill_ns: None, counts: GateCounts::default() }
    }
}

impl<T> ViewerGate<T> {
    pub fn counts(&self) -> GateCounts {
        self.counts
    }

    /// Forget the pacing history and let go of any frame held back (the Feed or the driving Sender changed).
    pub fn reset(&mut self) {
        self.run = None;
        self.held = None;
        self.last_fill_ns = None;
    }

    /// A frame arrived from the Feed. Frames to show are pushed onto `show`, oldest first.
    pub fn offer(&mut self, timing: Timing, frame: T, _now_ns: u64, show: &mut Vec<T>) {
        if let Some(run) = &mut self.run
            && run.instance == timing.sender_instance
            && run.last_time == timing.frame_time
        {
            self.counts.repeat_renders += 1;
            run.last_ns = run.last_ns.max(timing.render_ns);
            match run.state {
                State::Verifying => self.held = Some(frame),
                State::Showing => show.push(frame),
                State::Fill => self.counts.cache_renders_ignored += 1,
            }
            return;
        }

        if self.run.as_ref().is_some_and(|r| r.continues(&timing)) {
            let run = self.run.as_mut().unwrap();
            if run.direction == 0.0 {
                run.direction = (timing.frame_time - run.last_time).signum();
            }
            run.push(timing.frame_time, timing.render_ns);
            if run.state == State::Fill && run.is_steady() {
                run.state = State::Showing; // it settled into playback
            }
            if run.state != State::Fill && run.is_fill() {
                // Every frame of the run so far (none of them shown, as it was still being verified) was the fill.
                if run.state == State::Verifying {
                    self.held = None;
                    self.counts.cache_renders_ignored += (run.last_time - run.first_time).abs() as u64;
                }
                run.state = State::Fill;
            }
            match run.state {
                State::Fill => {
                    self.counts.cache_renders_ignored += 1;
                    self.last_fill_ns = Some(timing.render_ns);
                }
                State::Showing => {
                    self.counts.viewer_frames += 1;
                    show.push(frame);
                }
                State::Verifying => {
                    // A newer frame of the same run replaces the one held back (not counted: it may be playback).
                    self.held = Some(frame);
                    if run.last_ns - run.first_ns >= verify_ns(run.period) && run.one_at_a_time() {
                        run.state = State::Showing; // playback
                        self.release(show);
                    }
                }
            }
            return;
        }

        self.end_run(show);
        let period = period_ns(timing.frame_rate);
        let fragment = self.last_fill_ns.is_some_and(|f| timing.render_ns.saturating_sub(f) <= FRAGMENT_GAP_NS);
        self.run = Some(Run {
            instance: timing.sender_instance,
            period,
            first_time: timing.frame_time,
            first_ns: timing.render_ns,
            recent: std::collections::VecDeque::from([(timing.frame_time, timing.render_ns)]),
            clock: timing.render_ns as f64,
            last_time: timing.frame_time,
            last_ns: timing.render_ns,
            direction: 0.0,
            state: State::Verifying,
            fragment,
        });
        self.held = Some(frame);
    }

    /// Time has passed (`now_ns`, the same clock as `render_ns`): a run with nothing new for a while is over.
    pub fn tick(&mut self, now_ns: u64, show: &mut Vec<T>) {
        if self.held.is_some() && self.run.as_ref().is_some_and(|r| r.state == State::Verifying && now_ns >= r.last_ns + hold_ns(r.period)) {
            // The run paused without being a cache fill: the user stepped or scrubbed. Show where it got to. The
            // run is still watched: if it carries on it is held and judged again.
            self.release_or_drop_fragment(show);
        }
    }

    fn release(&mut self, show: &mut Vec<T>) {
        if let Some(frame) = self.held.take() {
            self.counts.viewer_frames += 1;
            show.push(frame);
        }
    }

    /// A frame that does not follow on has arrived: the current run is over.
    fn end_run(&mut self, show: &mut Vec<T>) {
        if self.run.as_ref().is_some_and(|r| r.state == State::Verifying) {
            self.release_or_drop_fragment(show); // a run the user made, cut short: show where it got to
        }
        self.run = None;
    }

    /// A run ended or paused before it was verified: show its held frame, unless it was a fragment of a fill.
    fn release_or_drop_fragment(&mut self, show: &mut Vec<T>) {
        match &self.run {
            Some(r) if r.fragment => {
                if self.held.take().is_some() {
                    self.counts.cache_renders_ignored += 1;
                }
                self.last_fill_ns = Some(r.last_ns); // the fill carries on through its fragments
            }
            _ => self.release(show),
        }
    }
}
