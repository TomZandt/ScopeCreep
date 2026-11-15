//! What the app shows and reports, driven at the Feed: the single test seam (spec #2, "Testing Decisions").
//! A synthetic frame writer stands in for Resolve and the Sender; no Resolve is needed.
use scopecreep::monitor::{FeedState, Monitor};
use scopecreep_feed::{FeedWriter, FrameMeta, now_ns, synthetic};
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
}

impl Bench {
    fn new(name: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-app-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        Bench { monitor: Monitor::new(&path), path, writer: None, now: now_ns(), shown: Vec::new() }
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
        let meta = FrameMeta { width: 16, height: 9, frame_time: frame, frame_rate: 24.0, render_ns: self.now, sender_instance: 1 };
        let _ = self.writer.as_mut().unwrap().write_frame(&meta, &synthetic::solid(16, 9, [value; 4]));
        self.poll();
    }
    fn render(&mut self, frame: f64) {
        self.render_valued(frame, frame as f32);
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
