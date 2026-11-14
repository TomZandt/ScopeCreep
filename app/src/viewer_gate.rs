//! Which Feed frames are **Viewer Frames**: the pacing gate.
//!
//! Resolve flags thumbnail renders (the Sender never feeds those) but not background cache renders or repeat
//! renders, and a plugin cannot see the playhead (Stage 0, ticket #11). So the Sender feeds every other render
//! and this gate decides from pacing alone. It is the doorman watching people arrive: a steady queue is playback,
//! one person turning up is a scrub or a step, and a coach party pouring through the door in numbered order is a
//! cache fill.
//!
//! The rules, with `period` the timeline's frame period and `fast` = half a period:
//!
//! - **Repeat render** (same Sender instance, same frame number as the previous render): the picture is updated,
//!   because a grade may have changed, but it is not a new Viewer Frame.
//! - **Cache fill**: a run of frames, each exactly one frame number after the last and each arriving less than
//!   `fast` after the last. A run of [`MIN_CACHE_FILL_RUN`] or more is a cache fill: none of it is shown and the
//!   Picture View keeps the last Viewer Frame. A shorter run is a quick scrub or key taps: its last frame is shown.
//! - **Steady playback** (the previous frame was shown, and this one came between `fast` and two periods after
//!   it): shown at once.
//! - **Anything else** (a step, a scrub, the first frame after a pause): held for `fast` to see whether it is the
//!   start of a cache fill, then shown. This is the only delay the gate adds.
//!
//! Known limit: at timeline rates where a cache fill is no faster than half a frame period (it rendered at about
//! 12 ms a frame at HD in Stage 0, so 50 and 60 fps timelines), pacing cannot tell it from playback. Telling
//! them apart there needs the playhead, which only the scripting side can supply (ticket #16).

/// A run of at least this many fast, consecutive frames is a cache fill.
pub const MIN_CACHE_FILL_RUN: u32 = 4;

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

struct Last {
    instance: u32,
    frame_time: f64,
    render_ns: u64,
    shown: bool,
}

/// Holds at most one frame of type `T` back at a time.
pub struct ViewerGate<T> {
    last: Option<Last>,
    /// A frame waiting to see whether it starts a cache fill, and when it is released.
    held: Option<(T, u64)>,
    /// Frames in the current fast, consecutive run (0 = no run).
    run: u32,
    /// The newest frame of the current run, and when the run is over if nothing follows.
    candidate: Option<(T, u64)>,
    counts: GateCounts,
}

impl<T> Default for ViewerGate<T> {
    fn default() -> Self {
        ViewerGate { last: None, held: None, run: 0, candidate: None, counts: GateCounts::default() }
    }
}

impl<T> ViewerGate<T> {
    pub fn counts(&self) -> GateCounts {
        self.counts
    }

    /// Forget the pacing history and let go of any frame held back (the Feed was replaced).
    pub fn reset(&mut self) {
        self.last = None;
        self.held = None;
        self.candidate = None;
        self.run = 0;
    }

    /// A frame arrived from the Feed at `now_ns`. Frames to show are pushed onto `show`, oldest first.
    pub fn offer(&mut self, timing: Timing, frame: T, now_ns: u64, show: &mut Vec<T>) {
        let period = (1e9 / if timing.frame_rate >= 1.0 { timing.frame_rate } else { 24.0 }) as u64;
        let fast = period / 2;

        if let Some(last) = &self.last
            && last.instance == timing.sender_instance
            && last.frame_time == timing.frame_time
        {
            self.counts.repeat_renders += 1;
            if let Some(held) = &mut self.held {
                held.0 = frame;
            } else if let Some(candidate) = &mut self.candidate {
                candidate.0 = frame;
            } else if last.shown {
                show.push(frame);
            } else {
                self.counts.cache_renders_ignored += 1;
            }
            return;
        }

        let since_last = self.last.as_ref().map(|l| timing.render_ns.saturating_sub(l.render_ns));
        let follows_fast = self.last.as_ref().is_some_and(|l| {
            l.instance == timing.sender_instance && timing.frame_time == l.frame_time + 1.0 && since_last.unwrap() < fast
        });
        let mut shown = false;
        if follows_fast {
            if self.held.take().is_some() {
                self.counts.cache_renders_ignored += 1; // it was the first frame of this run
                self.run = 1;
            }
            self.run = self.run.max(1) + 1;
            if self.candidate.replace((frame, now_ns + fast)).is_some() {
                self.counts.cache_renders_ignored += 1;
            }
        } else {
            self.end_run(show);
            if let Some((held, _)) = self.held.take() {
                self.counts.viewer_frames += 1; // its successor is not a cache fill, so neither was it
                show.push(held);
                if let Some(l) = &mut self.last {
                    l.shown = true;
                }
            }
            let steady = self.last.as_ref().is_some_and(|l| l.shown) && since_last.is_some_and(|dt| dt >= fast && dt <= 2 * period);
            if steady {
                self.counts.viewer_frames += 1;
                show.push(frame);
                shown = true;
            } else {
                self.held = Some((frame, now_ns + fast));
            }
        }
        self.last = Some(Last { instance: timing.sender_instance, frame_time: timing.frame_time, render_ns: timing.render_ns, shown });
    }

    /// Time has passed: release whatever has waited long enough.
    pub fn tick(&mut self, now_ns: u64, show: &mut Vec<T>) {
        if self.held.as_ref().is_some_and(|h| now_ns >= h.1) {
            let (frame, _) = self.held.take().unwrap();
            self.counts.viewer_frames += 1;
            show.push(frame);
            if let Some(l) = &mut self.last {
                l.shown = true;
            }
        }
        if self.candidate.as_ref().is_some_and(|c| now_ns >= c.1) {
            self.end_run(show);
        }
    }

    /// The current run is over: a long one was a cache fill, a short one was the user.
    fn end_run(&mut self, show: &mut Vec<T>) {
        if let Some((frame, _)) = self.candidate.take() {
            if self.run >= MIN_CACHE_FILL_RUN {
                self.counts.cache_renders_ignored += 1;
            } else {
                // The earlier frames of a short run were counted as ignored; that stands (they were never shown).
                self.counts.viewer_frames += 1;
                show.push(frame);
                if let Some(l) = &mut self.last {
                    l.shown = true;
                }
            }
        }
        self.run = 0;
    }
}
