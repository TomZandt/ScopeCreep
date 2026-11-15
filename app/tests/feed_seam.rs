//! What the app shows and reports, driven at the Feed: the single test seam (spec #2, "Testing Decisions").
//! A synthetic frame writer stands in for Resolve and the Sender; no Resolve is needed.
use scopecreep::monitor::{FeedState, Monitor};
use scopecreep_feed::{FeedWriter, FrameMeta, SenderIdentity, now_ns, synthetic};
use std::path::{Path, PathBuf};

const MS: u64 = 1_000_000;
/// One frame period at 24 fps.
const FRAME_24: u64 = 1_000_000_000 / 24;

/// A stand-in Resolve (the synthetic writer), the app's Feed monitor, a clock, and a record of what the Picture
/// View was asked to show.
struct Bench {
    path: PathBuf,
    writer: Option<FeedWriter>,
    monitor: Monitor,
    now: u64,
    /// (frame number, value of the first pixel) of every picture shown, in order.
    shown: Vec<(f64, f32)>,
    /// The timeline rate the stand-in Resolve plays at.
    fps: f64,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-app-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        Bench { monitor: Monitor::new(&path), path, writer: None, now: now_ns(), shown: Vec::new(), fps: 24.0 }
    }
    fn with_sender(name: &str) -> Bench {
        let mut b = Bench::new(name);
        b.sender_starts(4, 16 * 9 * 16);
        b.poll();
        b
    }
    fn sender_starts(&mut self, slots: usize, slot_bytes: usize) {
        self.writer = Some(FeedWriter::create(&self.path, slots, slot_bytes).unwrap());
    }
    fn poll(&mut self) {
        let shown = &mut self.shown;
        self.monitor.poll(self.now, &mut |f| shown.push((f.frame_time(), f.pixels()[0])));
    }
    /// Let time pass, polling as the app does.
    fn wait(&mut self, ns: u64) {
        let end = self.now + ns;
        while self.now < end {
            self.now = (self.now + MS).min(end);
            self.poll();
        }
    }
    /// Resolve renders `frame` now; its pixels all have the value `value`.
    fn render_valued(&mut self, frame: f64, value: f32) {
        self.render_by(1, frame, value);
    }
    /// Sender instance `instance` renders `frame` now; its pixels all have the value `value`.
    fn render_by(&mut self, instance: u32, frame: f64, value: f32) {
        let meta = FrameMeta { width: 16, height: 9, frame_time: frame, frame_rate: self.fps, render_ns: self.now, sender_instance: instance };
        let _ = self.writer.as_mut().unwrap().write_frame(&meta, &synthetic::solid(16, 9, [value; 4]));
        self.poll();
    }
    /// Values of the pictures shown, in order.
    fn shown_values(&self) -> Vec<f32> {
        self.shown.iter().map(|s| s.1).collect()
    }
    fn render(&mut self, frame: f64) {
        self.render_valued(frame, frame as f32);
    }
    /// Resolve plays `count` frames from `first` at `fps`, each render call `jitter_ns(i)` off the exact time.
    fn play(&mut self, first: u64, count: u64, jitter_ns: impl Fn(u64) -> i64) {
        let start = self.now;
        for i in 0..count {
            let due = start + (i as f64 * 1e9 / self.fps) as u64;
            let at = (due as i64 + jitter_ns(i)) as u64;
            if at > self.now {
                self.wait(at - self.now);
            }
            self.render((first + i) as f64);
        }
    }
    fn status_line(&self) -> String {
        self.monitor.status().status_line()
    }
    fn shown_frames(&self) -> Vec<f64> {
        self.shown.iter().map(|s| s.0).collect()
    }
    fn label(&self) -> String {
        self.monitor.status().picture_label()
    }
    fn notice(&self) -> String {
        self.monitor.status().notice().unwrap_or_default()
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn first_pixels_bits(path: &Path, pixels: &[f32], poll_for_ns: u64) -> (Vec<u32>, Vec<u32>) {
    let mut writer = FeedWriter::create(path, 4, pixels.len() * 4).unwrap();
    let mut monitor = Monitor::new(path);
    let mut now = now_ns();
    monitor.poll(now, &mut |_| {});
    let meta = FrameMeta { width: 64, height: 36, frame_time: 86400.0, frame_rate: 24.0, render_ns: now, sender_instance: 1 };
    writer.write_frame(&meta, pixels).unwrap();
    let mut got = Vec::new();
    let end = now + poll_for_ns;
    while now < end {
        now += MS;
        monitor.poll(now, &mut |f| got = f.pixels().iter().map(|v| v.to_bits()).collect());
    }
    (got, pixels.iter().map(|v| v.to_bits()).collect())
}

// ------------------------------------------------------------------ the picture

#[test]
fn the_picture_view_shows_a_feed_frame_unmodified() {
    let path = PathBuf::from(format!("/dev/shm/scopecreep-app-test-{}-unmodified", std::process::id()));
    let (got, want) = first_pixels_bits(&path, &synthetic::awkward_values(64, 36), 100 * MS);
    let _ = std::fs::remove_file(&path);
    assert_eq!(got, want, "every value reaches the Picture View bit for bit: negative, over-range, denormal");
}

#[test]
fn the_label_says_what_the_picture_is() {
    let mut b = Bench::with_sender("label");
    b.render(86400.0);
    b.wait(100 * MS);
    assert_eq!(b.label(), "Clean picture  ·  16 × 9  ·  Colour Setup unknown  ·  as sent, no Output Conversion");
    assert_eq!(b.notice(), "", "nothing to report while the picture is live");
}

#[test]
fn with_no_sender_the_app_says_so_and_shows_nothing() {
    let mut b = Bench::new("nosender");
    b.wait(600 * MS);
    assert_eq!(b.monitor.status().feed, FeedState::NoSender);
    assert_eq!(b.notice(), "No Sender. Put the scopecreep Sender on the last timeline node in Resolve.");
    assert_eq!(b.label(), "Clean picture");
    assert!(b.shown.is_empty());
}

#[test]
fn a_sender_with_no_frames_is_not_an_error() {
    let mut b = Bench::with_sender("noframes");
    b.wait(3_000 * MS);
    assert_eq!(b.monitor.status().feed, FeedState::Connected);
    assert_eq!(b.notice(), "Waiting for frames. Play or move the playhead in Resolve.");
}

#[test]
fn when_frames_stop_the_last_picture_stays_without_complaint() {
    // Shift-D, a disabled node, the Fusion page or simply a pause: Resolve stops rendering.
    let mut b = Bench::with_sender("frozen");
    b.render(10.0);
    b.wait(5_000 * MS);
    assert_eq!(b.shown_frames(), [10.0]);
    assert_eq!(b.monitor.status().picture.as_ref().unwrap().frame_time, 10.0);
    assert_eq!(b.notice(), "");
}

// ------------------------------------------------------- playback, pause, scrubbing

#[test]
fn every_frame_of_playback_is_shown_in_order() {
    let mut b = Bench::with_sender("playback");
    for f in 0..24 {
        b.render(f as f64);
        b.wait(FRAME_24);
    }
    assert_eq!(b.shown_frames(), (0..24).map(|f| f as f64).collect::<Vec<_>>());
    assert_eq!(b.monitor.status().viewer_frames, 24);
}

#[test]
fn stepping_one_frame_while_paused_shows_that_frame() {
    let mut b = Bench::with_sender("step");
    b.render(10.0);
    b.wait(2_000 * MS);
    b.render(11.0);
    b.wait(2_000 * MS);
    b.render(10.0);
    b.wait(2_000 * MS);
    assert_eq!(b.shown_frames(), [10.0, 11.0, 10.0]);
}

#[test]
fn repeat_renders_of_one_frame_count_as_one_viewer_frame() {
    // Resolve renders a frame one to three times on a first visit.
    let mut b = Bench::with_sender("repeats");
    b.render(10.0);
    b.wait(3 * MS);
    b.render(10.0);
    b.wait(3 * MS);
    b.render(10.0);
    b.wait(500 * MS);
    let status = b.monitor.status();
    assert_eq!(status.viewer_frames, 1);
    assert_eq!(status.repeat_renders, 2);
    assert_eq!(status.picture.as_ref().unwrap().frame_time, 10.0);
}

#[test]
fn a_grade_change_on_a_paused_frame_updates_the_picture() {
    let mut b = Bench::with_sender("regrade");
    b.render_valued(10.0, 0.25);
    b.wait(1_000 * MS);
    b.render_valued(10.0, 0.75); // same frame, new grade
    b.wait(1_000 * MS);
    assert_eq!(b.shown.last(), Some(&(10.0, 0.75)));
}

#[test]
fn scrubbing_ends_on_the_frame_the_playhead_stopped_at() {
    let mut b = Bench::with_sender("scrub");
    b.render(10.0);
    b.wait(1_000 * MS);
    for (f, gap) in [(14.0, 9), (22.0, 12), (23.0, 8), (41.0, 11), (57.0, 30), (56.0, 7), (60.0, 10)] {
        b.render(f);
        b.wait(gap * MS);
    }
    b.wait(1_000 * MS);
    assert_eq!(b.shown.last().unwrap().0, 60.0);
    assert!(b.shown.len() >= 4, "the picture follows the scrub, not just its end: {:?}", b.shown_frames());
}

#[test]
fn a_quick_tap_through_a_few_frames_ends_on_the_last_one() {
    let mut b = Bench::with_sender("tap");
    b.render(10.0);
    b.wait(1_000 * MS);
    for f in [11.0, 12.0, 13.0] {
        b.render(f);
        b.wait(12 * MS);
    }
    b.wait(1_000 * MS);
    assert_eq!(b.shown.last().unwrap().0, 13.0);
}

// ----------------------------------------------------------- background cache renders

#[test]
fn a_background_cache_fill_does_not_change_the_picture() {
    // Stage 0 (#11): 96 renders, one per frame 0..95, about 12 ms apart, while the playhead stayed on frame 10.
    let mut b = Bench::with_sender("cachefill");
    b.render(10.0);
    b.wait(2_000 * MS);
    for f in 0..96 {
        b.render(f as f64);
        b.wait(12 * MS);
    }
    b.wait(2_000 * MS);
    assert_eq!(b.shown_frames(), [10.0], "the Picture View keeps the last Viewer Frame");
    let status = b.monitor.status();
    assert_eq!(status.picture.as_ref().unwrap().frame_time, 10.0);
    assert_eq!(status.cache_renders_ignored, 96);
    assert_eq!(status.viewer_frames, 1);
}

/// A background cache fill while paused on frame 10 at `fps`: Resolve renders frames 0..96 `gap_ms` apart.
fn cache_fill_while_paused(name: &str, fps: f64, gap_ms: u64) -> Bench {
    let mut b = Bench::with_sender(name);
    b.fps = fps;
    b.render(10.0);
    b.wait(2_000 * MS);
    for f in 0..96 {
        b.render(f as f64);
        b.wait(gap_ms * MS);
    }
    b.wait(2_000 * MS);
    b
}

#[test]
fn a_cache_fill_at_50_and_60_fps_does_not_change_the_picture() {
    // A cache fill renders as fast as the GPU allows (about 12 ms a frame at HD in Stage 0), which at 50 and
    // 60 fps is more than half a frame period: it is told apart because it runs ahead of the clock.
    for fps in [50.0, 59.94, 60.0] {
        let b = cache_fill_while_paused(&format!("fill{fps}"), fps, 12);
        assert_eq!(b.shown_frames().first(), Some(&10.0));
        let after: Vec<f64> = b.shown_frames().into_iter().skip(1).collect();
        assert!(after.len() <= 1, "{fps} fps: at most the first render may slip through, got {after:?}");
        assert_eq!(b.monitor.status().picture.as_ref().unwrap().frame_time, 10.0, "{fps} fps: the picture ends on the viewer frame");
    }
}

#[test]
fn a_cache_fill_that_jumps_back_and_forth_does_not_change_the_picture() {
    // Resolve 21.2 at 50 and 60 fps (#19): the fill ran at 1.5-2x real time, in pairs, and every so often a
    // second thread re-rendered a few frames a little way back. Each jump cuts the run short.
    for fps in [50.0, 60.0] {
        let mut b = Bench::with_sender(&format!("jumpy{fps}"));
        b.fps = fps;
        b.render(11.0);
        b.wait(2_000 * MS);
        let mut f = 0u64;
        while f < 480 {
            b.render(f as f64);
            b.wait(MS);
            b.render(f as f64 + 1.0);
            b.wait(if fps == 50.0 { 18 } else { 22 } * MS);
            if f % 24 == 22 {
                for back in [7, 6] {
                    b.render((f - back) as f64);
                    b.wait(5 * MS);
                }
            }
            f += 2;
        }
        b.wait(2_000 * MS);
        assert_eq!(b.shown_frames(), [11.0], "{fps} fps");
        // and a step after the fill has finished is shown
        b.render(12.0);
        b.wait(500 * MS);
        assert_eq!(b.shown_frames(), [11.0, 12.0], "{fps} fps");
    }
}

#[test]
fn a_cache_fill_that_slows_to_real_time_in_places_stays_hidden() {
    // Resolve 21.2 at 50 fps (#19): averaged over a few frames the fill sometimes slowed to the frame rate, but
    // it still rendered in pairs. Playback renders one frame at a time.
    let mut b = Bench::with_sender("slowfill");
    b.fps = 50.0;
    b.render(11.0);
    b.wait(2_000 * MS);
    let mut f = 0u64;
    while f < 300 {
        let gap = if (f / 20).is_multiple_of(2) { 18 } else { 39 }; // stretches at twice and at about once real time
        b.render(f as f64);
        b.wait(MS);
        b.render(f as f64 + 1.0);
        b.wait(gap * MS);
        f += 2;
    }
    b.wait(2_000 * MS);
    assert_eq!(b.shown_frames(), [11.0]);
}

#[test]
fn a_ragged_playback_start_does_not_hide_the_playback_that_follows() {
    // Resolve 21.2, HD 24 fps, cold start (#19): the first second renders out of order and in bursts, then
    // playback settles at the frame rate. The burst may be held back, but once playback is steady it is shown.
    let mut b = Bench::with_sender("ragged");
    b.render(0.0);
    b.wait(1_000 * MS);
    let start: [(f64, f64); 30] = [
        (1.0, 191.4), (3.0, 58.2), (2.0, 21.1), (4.0, 21.2), (6.0, 7.4), (7.0, 35.1), (5.0, 59.1), (8.0, 2.3), (11.0, 7.8),
        (10.0, 176.3), (9.0, 8.2), (12.0, 2.8), (13.0, 3.3), (14.0, 51.8), (16.0, 46.5), (15.0, 47.5), (18.0, 27.1),
        (17.0, 23.0), (20.0, 80.5), (19.0, 29.7), (21.0, 14.0), (24.0, 3.2), (22.0, 16.9), (25.0, 2.4), (23.0, 20.9),
        (26.0, 10.4), (27.0, 42.3), (28.0, 43.1), (29.0, 57.5), (30.0, 26.9),
    ];
    for (f, gap_ms) in start {
        b.wait((gap_ms * 1e6) as u64);
        b.render(f);
    }
    let t = b.now;
    for f in 31..120u64 {
        b.wait((t + (f - 30) * FRAME_24).saturating_sub(b.now));
        b.render(f as f64);
    }
    b.wait(500 * MS);
    let shown = b.shown_frames();
    assert_eq!(shown.last(), Some(&119.0));
    let steady: Vec<f64> = shown.iter().copied().filter(|&f| f >= 45.0).collect();
    assert_eq!(steady, (45..120).map(|f| f as f64).collect::<Vec<_>>(), "steady playback is all shown");
}

#[test]
fn a_cache_fill_rendered_two_frames_at_a_time_does_not_change_the_picture() {
    // Resolve 21.2 at 50 and 60 fps (#19): the fill came in pairs, two renders under a millisecond apart, then
    // a gap of about 20 ms (about 10 ms a frame on average).
    for fps in [50.0, 60.0] {
        let mut b = Bench::with_sender(&format!("pairs{fps}"));
        b.fps = fps;
        b.render(11.0);
        b.wait(2_000 * MS);
        for f in (0..400).step_by(2) {
            b.render(f as f64);
            b.wait(MS);
            b.render(f as f64 + 1.0);
            b.wait(19 * MS);
        }
        b.wait(1_000 * MS);
        assert_eq!(b.shown_frames(), [11.0], "{fps} fps");
        assert_eq!(b.monitor.status().cache_renders_ignored, 400, "{fps} fps");
    }
}

#[test]
fn late_frames_that_arrive_bunched_during_playback_are_all_shown() {
    // The Sender (or a busy machine) falls behind for a moment and then catches up: frames come late and
    // close together, but never ahead of the clock. They are playback, not a cache fill (#15 saw this at UHD).
    for fps in [25.0, 50.0] {
        let mut b = Bench::with_sender(&format!("bunched{fps}"));
        b.fps = fps;
        let period = (1e9 / fps) as u64;
        let start = b.now;
        let mut f = 0u64;
        while f < 200 {
            if f % 40 == 20 {
                // stalled for three periods, then four frames 3 ms apart
                b.wait((start + (f + 3) * period).saturating_sub(b.now));
                for k in 0..4 {
                    b.render((f + k) as f64);
                    b.wait(3 * MS);
                }
                f += 4;
                continue;
            }
            b.wait((start + f * period).saturating_sub(b.now));
            b.render(f as f64);
            f += 1;
        }
        b.wait(500 * MS);
        assert_eq!(b.monitor.status().cache_renders_ignored, 0, "{fps} fps");
        let shown = b.shown_frames();
        assert!(shown[0] <= 4.0, "{fps} fps: playback is confirmed within its first frames: {shown:?}");
        assert_eq!(shown, (shown[0] as u64..200).map(|f| f as f64).collect::<Vec<_>>(), "{fps} fps: from then on every frame is shown");
    }
}

#[test]
fn holding_an_arrow_key_steps_the_picture_with_it() {
    // Key repeat steps a frame about every 33 ms: faster than 24 fps playback, but the user is driving it.
    let mut b = Bench::with_sender("keyrepeat");
    b.render(10.0);
    b.wait(1_000 * MS);
    for f in 11..41 {
        b.render(f as f64);
        b.wait(33 * MS);
    }
    b.wait(500 * MS);
    assert_eq!(b.shown.last().unwrap().0, 40.0);
    assert!(b.shown.len() >= 10, "the picture follows while the key is held: {:?}", b.shown_frames());
}

#[test]
fn playback_at_60_fps_with_hitches_shows_every_frame() {
    let mut b = Bench::with_sender("hitch60");
    b.fps = 60.0;
    let mut at = b.now;
    for f in 0..240u64 {
        at += if f % 60 == 59 { 50_000_000 } else { 16_666_667 } + [0, 2_000_000, 0, 0, 3_500_000, 0][f as usize % 6] - 1_000_000;
        b.wait(at - b.now);
        b.render(f as f64);
    }
    b.wait(500 * MS);
    let shown = b.shown_frames();
    assert!(shown[0] <= 4.0, "playback is confirmed within its first frames: {shown:?}");
    assert_eq!(shown, (shown[0] as u64..240).map(|f| f as f64).collect::<Vec<_>>(), "from then on every frame is shown");
    assert_eq!(b.monitor.status().cache_renders_ignored, 0);
}

#[test]
fn playback_after_a_cache_fill_is_shown_again() {
    let mut b = Bench::with_sender("afterfill");
    b.render(10.0);
    b.wait(2_000 * MS);
    for f in 0..48 {
        b.render(f as f64);
        b.wait(12 * MS);
    }
    b.wait(1_000 * MS);
    for f in 10..20 {
        b.render(f as f64);
        b.wait(FRAME_24);
    }
    assert_eq!(b.shown_frames()[1..], (10..20).map(|f| f as f64).collect::<Vec<_>>()[..]);
}

// ------------------------------------------------------------------ playback rate

fn rate(b: &Bench) -> f64 {
    b.monitor.status().playback_rate.expect("a playback rate while playing")
}

#[test]
fn steady_playback_shows_resolves_playback_rate() {
    let mut b = Bench::with_sender("rate24");
    b.play(0, 72, |_| 0);
    assert!((rate(&b) - 24.0).abs() < 0.1, "{}", rate(&b));
    assert!(b.status_line().starts_with("24.00 fps"), "{}", b.status_line());
}

#[test]
fn the_rate_is_within_a_tenth_of_a_frame_despite_render_jitter() {
    // Resolve's render calls wander by a few milliseconds around the frame clock.
    for fps in [23.976, 25.0, 50.0, 59.94, 60.0] {
        let mut b = Bench::with_sender(&format!("jitter{fps}"));
        b.fps = fps;
        b.play(0, (fps * 3.0) as u64, |i| [0, 3_000_000, -2_000_000, 4_000_000, -4_000_000, 1_000_000][i as usize % 6]);
        assert!((rate(&b) - fps).abs() < 0.1, "{fps} fps timeline measured as {}", rate(&b));
    }
}

#[test]
fn frames_lost_in_the_feed_still_count_towards_the_rate() {
    // The app is late every so often and the Sender overwrites unread frames: Resolve still rendered them.
    let mut b = Bench::with_sender("ratelost");
    let start = b.now;
    for i in 0..96u64 {
        let due = start + i * FRAME_24;
        let meta = FrameMeta { width: 16, height: 9, frame_time: i as f64, frame_rate: 24.0, render_ns: due, sender_instance: 1 };
        b.writer.as_mut().unwrap().write_frame(&meta, &synthetic::solid(16, 9, [0.5; 4])).unwrap();
        b.now = due;
        if i % 8 < 3 {
            b.poll(); // for five periods in every eight the app looks away: four slots, so a frame is lost
        }
    }
    b.wait(10 * MS);
    assert!(b.monitor.status().dropped > 0, "the test must lose frames");
    assert!((rate(&b) - 24.0).abs() < 0.1, "{}", rate(&b));
}

#[test]
fn when_playback_stops_the_rate_gives_way_to_paused() {
    let mut b = Bench::with_sender("ratepause");
    b.play(0, 72, |_| 0);
    assert!(b.monitor.status().playback_rate.is_some());
    b.wait(1_000 * MS);
    assert_eq!(b.monitor.status().playback_rate, None);
    assert!(b.status_line().starts_with("paused"), "{}", b.status_line());
}

#[test]
fn a_hitch_in_playback_lowers_the_rate_instead_of_hiding_it() {
    // Resolve 21.2, HD 60 fps timeline (#19): steady 16.7 ms renders, but once a second a 50 ms hitch, so
    // Resolve really shows about 58 frames a second. The reading must stay up and say so.
    let mut b = Bench::with_sender("hitch");
    b.fps = 60.0;
    let mut at = b.now;
    for f in 0..600u64 {
        at += if f % 60 == 59 { 50_000_000 } else { 16_666_667 };
        b.wait(at - b.now);
        b.render(f as f64);
        if f > 150 {
            let r = rate(&b);
            let want = 600.0 / 10.0 * 60.0 / (59.0 + 3.0);
            assert!((r - want).abs() < 0.3, "frame {f}: {r} (about {want})");
        }
    }
}

#[test]
fn renders_that_wander_in_steady_playback_do_not_interrupt_the_rate() {
    // Resolve 21.2 at 24 fps (#19): now and then a render comes 10-20 ms after the previous one, and the next
    // late. The reading must stay up throughout.
    let mut b = Bench::with_sender("wander");
    let start = b.now;
    for f in 0..240u64 {
        let jitter: i64 = match f % 37 { 11 => -30_000_000, 12 => 15_000_000, _ => 0 };
        b.wait(((start + f * FRAME_24) as i64 + jitter) as u64 - b.now.min(((start + f * FRAME_24) as i64 + jitter) as u64));
        b.render(f as f64);
        if f >= 72 {
            let r = rate(&b);
            assert!((r - 24.0).abs() < 0.1, "frame {f}: {r}");
        }
    }
}

#[test]
fn the_bunched_renders_that_start_playback_do_not_skew_the_rate() {
    // Resolve 21.2 (#19): playback starts with a few renders a millisecond or two apart, then settles.
    let mut b = Bench::with_sender("bunch");
    b.render(0.0);
    b.wait(1_000 * MS);
    for (f, gap) in [(2.0, 30), (1.0, 1), (3.0, 10), (4.0, 4)] {
        b.wait(gap * MS);
        b.render(f);
    }
    let start = b.now;
    for f in 5..100u64 {
        b.wait((start + (f - 4) * FRAME_24).saturating_sub(b.now));
        b.render(f as f64);
        if let Some(r) = b.monitor.status().playback_rate {
            assert!((r - 24.0).abs() < 0.1, "frame {f}: {r}");
        }
    }
    assert!(b.monitor.status().playback_rate.is_some());
}

#[test]
fn a_cache_fill_while_paused_is_not_reported_as_playback() {
    let mut b = Bench::with_sender("ratecache");
    b.render(10.0);
    b.wait(2_000 * MS);
    for f in 0..96 {
        b.render(f as f64);
        b.wait(12 * MS);
    }
    assert_eq!(b.monitor.status().playback_rate, None);
}

// ------------------------------------------------------------------- which Sender

#[test]
fn the_status_line_names_the_sender_driving_the_feed() {
    let mut b = Bench::with_sender("named");
    b.play(0, 24, |_| 0);
    let pid = std::process::id();
    assert!(
        b.status_line().ends_with(&format!("  ·  Sender 1 · synthetic frame writer · process {pid} · 24 fps timeline")),
        "{}",
        b.status_line()
    );
    assert_eq!(b.notice(), "");
}

/// Two Senders on one timeline (say one on a clip node and one on the timeline node): both render every frame.
/// Sender 1's pictures have the value 1, Sender 2's the value 2. `second_first` decides who renders first.
fn two_senders_on_one_timeline(name: &str, second_first: bool) -> Bench {
    let mut b = Bench::with_sender(name);
    let start = b.now;
    for f in 0..48u64 {
        let due = start + f * FRAME_24;
        b.wait(due.saturating_sub(b.now));
        let order: [u32; 2] = if second_first { [2, 1] } else { [1, 2] };
        for (k, instance) in order.into_iter().enumerate() {
            b.wait(k as u64 * 2 * MS);
            b.render_by(instance, f as f64, instance as f32);
        }
    }
    b.wait(500 * MS);
    b
}

#[test]
fn with_two_senders_rendering_the_same_frames_exactly_one_drives_the_feed() {
    for second_first in [false, true] {
        let b = two_senders_on_one_timeline(&format!("two{second_first}"), second_first);
        let values = b.shown_values();
        assert!(values[2..].iter().all(|&v| v == 1.0), "only Sender 1 is shown after the first frames: {values:?}");
        assert_eq!(b.monitor.status().driving.as_ref().unwrap().instance, 1);
    }
}

#[test]
fn the_other_active_sender_is_flagged() {
    let b = two_senders_on_one_timeline("flagged", true);
    let status = b.monitor.status();
    assert_eq!(status.ignored.iter().map(|s| s.instance).collect::<Vec<_>>(), [2]);
    let pid = std::process::id();
    assert_eq!(
        b.notice(),
        format!("Another Sender is active: Sender 2 (process {pid}). Showing Sender 1 (process {pid}) only. Keep one Sender, on the last timeline node.")
    );
    assert!(b.status_line().contains("Sender 1 · "), "{}", b.status_line());
}

#[test]
fn the_rate_follows_the_driving_sender_only() {
    let mut b = two_senders_on_one_timeline("tworate", false);
    let start = b.now;
    for f in 48..120u64 {
        let due = start + (f - 48) * FRAME_24;
        b.wait(due.saturating_sub(b.now));
        b.render_by(1, f as f64, 1.0);
        b.wait(MS);
        b.render_by(2, f as f64, 2.0);
    }
    assert!((rate(&b) - 24.0).abs() < 0.1, "{}", rate(&b));
}

#[test]
fn switching_timelines_follows_the_new_sender_at_once() {
    // Each timeline has its own Sender instance; only the timeline on screen renders.
    let mut b = Bench::with_sender("switch");
    b.play(0, 24, |_| 0);
    b.wait(150 * MS); // stop, switch timeline
    b.render_by(2, 500.0, 2.0);
    b.wait(200 * MS);
    assert_eq!(b.shown.last(), Some(&(500.0, 2.0)));
    assert_eq!(b.monitor.status().driving.as_ref().unwrap().instance, 2);
    assert!(b.monitor.status().ignored.is_empty(), "a Sender that stopped before the other started is not a conflict");
    b.render_by(1, 23.0, 1.0); // and back again
    b.wait(200 * MS);
    assert_eq!(b.shown.last(), Some(&(23.0, 1.0)));
    assert_eq!(b.notice(), "");
}

#[test]
fn when_the_extra_sender_is_removed_the_flag_clears() {
    let mut b = two_senders_on_one_timeline("removed", false);
    b.play(48, 72, |_| 0); // Sender 2 deleted: only Sender 1 renders
    assert!(b.monitor.status().ignored.is_empty());
    assert_eq!(b.notice(), "");
}

#[test]
fn when_the_driving_sender_is_removed_the_other_takes_over() {
    let mut b = two_senders_on_one_timeline("takeover", false);
    let start = b.now;
    for f in 48..72u64 {
        b.wait((start + (f - 48) * FRAME_24).saturating_sub(b.now));
        b.render_by(2, f as f64, 2.0); // Sender 1 deleted
    }
    b.wait(100 * MS);
    assert_eq!(b.shown.last(), Some(&(71.0, 2.0)));
    assert_eq!(b.monitor.status().driving.as_ref().unwrap().instance, 2);
}

#[test]
fn the_first_frame_of_a_second_sender_does_not_flash_up() {
    // Resolve 21.2, two Senders on one timeline (#19): at a cold playback start Sender 1 renders frame 0, Sender 2
    // two milliseconds later, then nothing for 200 ms. Sender 2's frame must not be shown in that pause.
    let mut b = Bench::with_sender("firstflash");
    b.render_by(1, 0.0, 1.0);
    b.wait(2 * MS);
    b.render_by(2, 0.0, 2.0);
    b.wait(200 * MS);
    for f in 1..48u64 {
        b.render_by(1, f as f64, 1.0);
        b.wait(2 * MS);
        b.render_by(2, f as f64, 2.0);
        b.wait(FRAME_24 - 2 * MS);
    }
    assert!(b.shown_values().iter().all(|&v| v == 1.0), "{:?}", b.shown_values());
    assert_eq!(b.monitor.status().ignored.iter().map(|s| s.instance).collect::<Vec<_>>(), [2]);
}

#[test]
fn stepping_with_two_senders_on_one_timeline_shows_the_same_sender_every_step() {
    let mut b = Bench::with_sender("twostep");
    for step in 0..6 {
        let f = 100.0 + step as f64;
        b.render_by(2, f, 2.0); // Sender 2 happens to render first
        b.wait(2 * MS);
        b.render_by(1, f, 1.0);
        b.wait(1_500 * MS);
        if step >= 1 {
            assert_eq!(b.shown.last(), Some(&(f, 1.0)), "step {step}: {:?}", b.shown);
            assert_eq!(b.monitor.status().ignored.len(), 1);
        }
    }
    b.wait(3_000 * MS);
    assert!(b.monitor.status().ignored.is_empty(), "the flag fades once both have stopped for a while");
}

/// Another Resolve: a real, running process whose pid a guest Sender can claim.
struct OtherResolve(std::process::Child);
impl OtherResolve {
    fn start() -> OtherResolve {
        OtherResolve(std::process::Command::new("sleep").arg("30").spawn().unwrap())
    }
    fn pid(&self) -> u64 {
        self.0.id() as u64
    }
}
impl Drop for OtherResolve {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_sender_in_a_second_resolve_is_flagged_and_never_shown() {
    let mut b = Bench::with_sender("guest");
    let other = OtherResolve::start();
    let mut guest = FeedWriter::start(&b.path, SenderIdentity { pid: other.pid(), host: "Resolve B".into() }).unwrap();
    assert!(guest.is_guest());
    guest.register(1, 25.0);
    let start = b.now;
    for f in 0..48u64 {
        b.wait((start + f * FRAME_24).saturating_sub(b.now));
        b.render(f as f64);
        let meta = FrameMeta { width: 16, height: 9, frame_time: 900.0 + f as f64, frame_rate: 25.0, render_ns: b.now, sender_instance: 1 };
        let _ = guest.write_frame(&meta, &synthetic::solid(16, 9, [-7.0; 4]));
    }
    b.wait(300 * MS);
    let status = b.monitor.status();
    assert!(b.shown.iter().all(|s| s.1 != -7.0), "no frame of the guest is shown");
    assert_eq!(status.driving.as_ref().unwrap().pid, std::process::id() as u64);
    let ignored: Vec<_> = status.ignored.iter().map(|s| (s.pid, s.instance, s.host.as_str(), s.frame_rate)).collect();
    assert_eq!(ignored, [(other.pid(), 1, "Resolve B", 25.0)]);
    let me = std::process::id();
    assert_eq!(
        b.notice(),
        format!(
            "Another Sender is active: Sender 1 (process {}). Showing Sender 1 (process {me}) only. Only the Resolve that loaded its Sender first sends frames: close the other Resolve or remove its Sender.",
            other.pid()
        )
    );
    b.wait(3_000 * MS); // the second Resolve stops
    assert!(b.monitor.status().ignored.is_empty());
}

#[test]
fn a_guest_is_flagged_even_before_the_first_resolve_sends_a_frame() {
    let mut b = Bench::with_sender("guestonly");
    let other = OtherResolve::start();
    let mut guest = FeedWriter::start(&b.path, SenderIdentity { pid: other.pid(), host: "Resolve B".into() }).unwrap();
    guest.register(1, 25.0);
    guest.note_render(1, 5.0, b.now);
    b.wait(300 * MS);
    let me = std::process::id();
    assert_eq!(
        b.notice(),
        format!(
            "A Sender in another Resolve (process {}) is active, but the Feed belongs to the Resolve that loaded its Sender first (process {me}). Close the other Resolve or remove its Sender.",
            other.pid()
        )
    );
}

// ------------------------------------------------------------------- feed health

#[test]
fn frames_the_app_was_too_slow_for_are_counted_as_dropped() {
    let mut b = Bench::with_sender("drops");
    let w = b.writer.as_mut().unwrap();
    for f in 0..10 {
        let meta = FrameMeta { width: 16, height: 9, frame_time: f as f64, frame_rate: 24.0, render_ns: b.now + f * FRAME_24, sender_instance: 1 };
        w.write_frame(&meta, &synthetic::solid(16, 9, [0.0; 4])).unwrap(); // the app is not polling
    }
    b.wait(100 * MS);
    assert_eq!(b.monitor.status().dropped, 6, "10 frames into 4 slots");
    assert!(b.status_line().contains("  ·  6 dropped (app too slow)"), "{}", b.status_line());
}

#[test]
fn frames_the_sender_could_not_take_without_waiting_are_counted_as_dropped() {
    let mut b = Bench::with_sender("busy");
    b.writer.as_ref().unwrap().skip_frame();
    b.writer.as_ref().unwrap().skip_frame();
    b.wait(10 * MS);
    assert_eq!(b.monitor.status().dropped, 2);
    assert!(b.status_line().contains("  ·  2 dropped (Sender busy)"), "{}", b.status_line());
}

#[test]
fn both_kinds_of_drop_are_told_apart() {
    let mut b = Bench::with_sender("bothdrops");
    let w = b.writer.as_mut().unwrap();
    for f in 0..10 {
        let meta = FrameMeta { width: 16, height: 9, frame_time: f as f64, frame_rate: 24.0, render_ns: b.now + f * FRAME_24, sender_instance: 1 };
        w.write_frame(&meta, &synthetic::solid(16, 9, [0.0; 4])).unwrap();
    }
    w.skip_frame();
    w.skip_frame();
    b.wait(10 * MS);
    assert!(b.status_line().contains("  ·  8 dropped (6 app too slow, 2 Sender busy)"), "{}", b.status_line());
}

#[test]
fn with_nothing_dropped_the_status_line_says_so() {
    let mut b = Bench::with_sender("nodrops");
    b.play(0, 48, |_| 0);
    assert!(b.status_line().contains("  ·  0 dropped"), "{}", b.status_line());
}

#[test]
fn drops_are_counted_from_when_the_app_attached() {
    let mut b = Bench::new("dropsbefore");
    b.sender_starts(4, 16 * 9 * 16);
    b.writer.as_ref().unwrap().skip_frame(); // no app yet
    b.wait(10 * MS);
    assert_eq!(b.monitor.status().dropped, 0);
    b.writer.as_ref().unwrap().skip_frame();
    b.wait(10 * MS);
    assert_eq!(b.monitor.status().dropped, 1);
}

#[test]
fn drops_still_count_after_the_sender_replaces_its_feed() {
    let mut b = Bench::with_sender("dropsreplace");
    b.writer.as_ref().unwrap().skip_frame();
    b.wait(10 * MS);
    b.sender_starts(4, 16 * 9 * 16 * 4); // a larger frame: a new Feed file, counters start again
    b.wait(300 * MS);
    b.writer.as_ref().unwrap().skip_frame();
    b.wait(10 * MS);
    assert_eq!(b.monitor.status().dropped, 2);
}

#[test]
fn the_sender_is_told_an_app_is_listening_only_while_one_is() {
    let mut b = Bench::new("listening");
    b.sender_starts(4, 4096);
    assert!(!b.writer.as_ref().unwrap().reader_present(b.now), "no app has looked yet");
    b.poll();
    assert!(b.writer.as_ref().unwrap().reader_present(b.now));
    b.wait(5_000 * MS);
    assert!(b.writer.as_ref().unwrap().reader_present(b.now), "the app keeps its heartbeat fresh");
    b.monitor = Monitor::new("/dev/shm/scopecreep-app-test-nowhere"); // the app quits
    assert!(!b.writer.as_ref().unwrap().reader_present(b.now));
}

#[test]
fn a_restarted_app_picks_the_feed_up_again() {
    let mut b = Bench::with_sender("restart");
    b.render(1.0);
    b.wait(500 * MS);
    b.monitor = Monitor::new(&b.path); // killed and started again
    b.poll();
    b.render(2.0);
    b.wait(500 * MS);
    assert_eq!(b.shown_frames(), [1.0, 2.0]);
}

#[test]
fn a_feed_version_mismatch_is_reported_not_crashed_on() {
    let mut b = Bench::new("mismatch");
    b.writer = Some(FeedWriter::create_with_version(&b.path, 4, 4096, 2, 3).unwrap());
    b.wait(600 * MS);
    assert_eq!(b.monitor.status().feed, FeedState::VersionMismatch { found: (2, 3), supported: (1, scopecreep_feed::VERSION_MINOR) });
    assert_eq!(
        b.notice(),
        "The Sender speaks Feed version 2.3 and this app reads version 1.x. Install the Sender and the app from the same release."
    );
    assert!(b.shown.is_empty());
    // the matching Sender is installed and Resolve restarted
    b.sender_starts(4, 16 * 9 * 16);
    b.wait(600 * MS);
    b.render(5.0);
    b.wait(500 * MS);
    assert_eq!(b.monitor.status().feed, FeedState::Connected);
    assert_eq!(b.shown_frames(), [5.0]);
}

#[test]
fn the_app_follows_the_sender_to_a_new_feed_file() {
    // The Sender starts with no ring and makes one the first time it is asked for a frame; later a larger frame
    // makes it replace the ring again.
    let mut b = Bench::new("replace");
    b.sender_starts(0, 0);
    b.wait(300 * MS);
    assert_eq!(b.monitor.status().feed, FeedState::Connected);
    b.sender_starts(4, 16 * 9 * 16);
    b.wait(50 * MS);
    assert!(b.writer.as_ref().unwrap().reader_present(b.now), "the app re-attached to the new Feed");
    b.render(3.0);
    b.wait(500 * MS);
    assert_eq!(b.shown_frames(), [3.0]);
}

#[test]
fn a_sender_error_is_passed_on() {
    let mut b = Bench::with_sender("error");
    b.writer.as_ref().unwrap().set_sender_error("cudaHostRegister: out of memory");
    b.wait(300 * MS);
    assert_eq!(b.notice(), "Sender: cudaHostRegister: out of memory");
}

#[test]
fn a_file_that_is_not_a_feed_is_reported() {
    let mut b = Bench::new("notafeed");
    std::fs::write(&b.path, vec![7u8; 8192]).unwrap();
    b.wait(600 * MS);
    assert_eq!(b.monitor.status().feed, FeedState::NotAFeed);
    assert!(b.notice().contains("is not a Feed"));
}
