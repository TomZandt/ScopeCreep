//! Resolve's live playback rate, measured from the Sender's own clock.
//!
//! Each Viewer Frame carries the monotonic time the Sender's render call began (`render_ns`). In steady playback
//! Resolve renders one Viewer Frame per frame period, so the rate is the slope of "how many frames so far"
//! against those times. It is a stopwatch held by the Sender, not by the app: the app being late to collect a
//! frame does not change the reading, and frames the Feed lost on the way (counted from gaps in the sequence
//! numbers) are added back, because Resolve did render them.
//!
//! The slope is a least-squares fit over the last few seconds of one unbroken run of playback, which averages
//! out the jitter of single render calls (a few milliseconds) far below the ±0.1 fps the app promises. A run
//! breaks when no Viewer Frame comes for two frame periods (a pause, a stop, a jump); while paused there is no
//! rate.

use std::collections::VecDeque;

/// The fit uses at most this much of the newest playback.
const WINDOW_NS: u64 = 4_000_000_000;
/// No rate is reported until the settled part of the run is at least this long: Resolve 21.2 renders now and
/// then 10-30 ms off the frame clock, too much to promise ±0.1 fps from a shorter span.
const MIN_SPAN_NS: u64 = 2_000_000_000;
/// With no Viewer Frame for this long, playback has stopped (or the Sender has gone quiet).
const STOPPED_NS: u64 = 500_000_000;
/// The longest gap between two Viewer Frames that still counts as the same run of playback.
const MAX_HITCH_NS: u64 = 250_000_000;
/// The first part of a run is left out of the fit: Resolve starts playback with renders out of order and in
/// bursts (Resolve 21.2, #19).
const SETTLE_NS: u64 = 500_000_000;

#[derive(Default)]
pub struct PlaybackRate {
    /// (render time, frames rendered since the run began) for the current run, oldest first.
    samples: VecDeque<(u64, u64)>,
    /// When the current run began.
    run_start_ns: u64,
    last_frame_time: Option<f64>,
}

impl PlaybackRate {
    /// A Viewer Frame was shown. `lost_before` is how many frames the Feed lost just before it.
    pub fn viewer_frame(&mut self, render_ns: u64, frame_time: f64, frame_rate: f64, lost_before: u64) {
        if self.last_frame_time == Some(frame_time) {
            return; // a repeat render of the same frame, not a new one
        }
        self.last_frame_time = Some(frame_time);
        let period = (1e9 / if frame_rate >= 1.0 { frame_rate } else { 24.0 }) as u64;
        // A run continues through jitter, renders bunched together and short hitches (Resolve at 60 fps stalls
        // about 50 ms once a second, and those stalls are part of its real rate). It starts again after a pause.
        let longest_gap = (2 * period).max(MAX_HITCH_NS) * (1 + lost_before);
        match self.samples.back().copied() {
            Some((t, n)) if render_ns >= t && render_ns - t <= longest_gap => {
                self.samples.push_back((render_ns, n + 1 + lost_before));
            }
            _ => {
                self.samples.clear();
                self.samples.push_back((render_ns, 0));
                self.run_start_ns = render_ns;
            }
        }
        // Keep the newest few seconds, after the run has settled.
        let keep_from = self.run_start_ns + SETTLE_NS;
        while self.samples.len() > 1 && self.samples.front().is_some_and(|&(t, _)| render_ns - t > WINDOW_NS || t < keep_from) {
            self.samples.pop_front();
        }
    }

    /// Forget the current run (the Feed or the driving Sender changed).
    pub fn reset(&mut self) {
        self.samples.clear();
        self.last_frame_time = None;
    }

    /// Frames per second, if Resolve is playing at `now_ns` and the run is long enough to measure.
    pub fn rate(&self, now_ns: u64) -> Option<f64> {
        let &(first, _) = self.samples.front()?;
        let &(last, _) = self.samples.back()?;
        if now_ns.saturating_sub(last) > STOPPED_NS || last - first < MIN_SPAN_NS {
            return None;
        }
        // Least-squares slope of frames against seconds, in coordinates relative to the first sample.
        let k = self.samples.len() as f64;
        let (mut st, mut sn, mut stt, mut stn) = (0.0, 0.0, 0.0, 0.0);
        for &(t, n) in &self.samples {
            let (t, n) = ((t - first) as f64 * 1e-9, n as f64);
            st += t;
            sn += n;
            stt += t * t;
            stn += t * n;
        }
        let denom = k * stt - st * st;
        (denom > 0.0).then(|| (k * stn - st * sn) / denom)
    }
}
