//! The Feed between two processes, as the Sender (in Resolve) and the app meet: `feed-synth` writes in its own
//! process and this test reads. The same on every platform; only where the Feed lives differs (ADR 0043).
mod common;
use common::{temp_feed, wait_for};
use scopecreep_feed::{FeedReader, OpenError};
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// `feed-synth` playing a small test card into the Feed at `path`.
struct Synth(Child);
impl Synth {
    fn start(path: &Path) -> Synth {
        Synth(
            Command::new(env!("CARGO_BIN_EXE_feed-synth"))
                .args(["--size", "16x16", "--fps", "50", "--path"])
                .arg(path)
                .stdout(Stdio::null())
                .spawn()
                .expect("feed-synth starts"),
        )
    }
    fn pid(&self) -> u64 {
        self.0.id() as u64
    }
    fn kill(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Drop for Synth {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Open the Feed at `path` once the writer has published one.
fn attach(path: &Path) -> FeedReader {
    let mut reader = None;
    wait_for("the writer publishes a Feed", || {
        match FeedReader::open(path) {
            Ok(r) => reader = Some(r),
            Err(OpenError::NoFeed) => {}
            Err(e) => panic!("{e}"),
        }
        reader.is_some()
    });
    reader.unwrap()
}

/// Follow the writer to the Feed with its ring (it makes one once a reader is attached) and take a frame.
fn next_frame_time(path: &Path, reader: &mut FeedReader) -> f64 {
    let mut time = None;
    wait_for("a frame arrives", || {
        if reader.retired() || reader.replaced() {
            *reader = attach(path);
        }
        reader.heartbeat(scopecreep_feed::now_ns());
        time = reader.next_frame().map(|f| f.frame_time());
        time.is_some()
    });
    time.unwrap()
}

#[test]
fn frames_written_in_one_process_are_read_in_another() {
    let path = temp_feed("two-processes");
    let synth = Synth::start(&path);
    let mut reader = attach(&path);
    assert_eq!(reader.sender().pid, synth.pid());
    let first = next_frame_time(&path, &mut reader);
    let later = next_frame_time(&path, &mut reader);
    assert!(
        later > first,
        "the test card moves on: {first} then {later}"
    );
}

#[test]
fn the_reader_notices_when_the_writer_process_has_gone() {
    let path = temp_feed("vanished");
    let mut synth = Synth::start(&path);
    let mut reader = attach(&path);
    next_frame_time(&path, &mut reader);
    assert!(reader.sender_alive());
    synth.kill();
    assert!(
        !reader.sender_alive(),
        "the Sender's process is gone, though its Feed is still mapped"
    );
}
