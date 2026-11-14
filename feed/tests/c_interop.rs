//! The two implementations of the Feed protocol, tested against each other: the C header the Sender uses
//! (`feed/include/scopecreep_feed.h`, driven through `feed/c/feed_c_tool.c`) and this Rust crate.
mod common;
use common::temp_feed;
use scopecreep_feed::{FeedReader, FeedWriter, FrameMeta, OpenError, now_ns, synthetic};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Build the C tool once per test run.
fn c_tool() -> &'static Path {
    static TOOL: OnceLock<PathBuf> = OnceLock::new();
    TOOL.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("feed_c_tool");
        let status = Command::new("cc")
            .args(["-std=c11", "-D_GNU_SOURCE", "-O1", "-Wall", "-Wextra", "-Werror", "-I"])
            .arg(dir.join("include"))
            .arg(dir.join("c/feed_c_tool.c"))
            .arg("-o")
            .arg(&out)
            .status()
            .expect("a C compiler (cc) is needed for the interop tests");
        assert!(status.success(), "feed_c_tool.c did not compile");
        out
    })
}

fn run(args: &[&str]) -> String {
    let out = Command::new(c_tool()).args(args).output().unwrap();
    assert!(out.status.success(), "feed_c_tool {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ok() {
        assert!(start.elapsed() < Duration::from_secs(5), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn the_header_compiles_as_cpp_for_the_sender() {
    let header = Path::new(env!("CARGO_MANIFEST_DIR")).join("include/scopecreep_feed.h");
    let status = Command::new("c++").args(["-std=c++17", "-Wall", "-Wextra", "-Werror", "-fsyntax-only", "-x", "c++"]).arg(header).status().unwrap();
    assert!(status.success());
}

#[test]
fn frames_written_by_the_c_writer_are_read_by_the_rust_reader() {
    let path = temp_feed("c-to-rust");
    // 3 frames of 16x9: the C tool fills pixel i of frame f with f * 1000 + i * 0.25 - 2
    run(&["write", path.to_str().unwrap(), "4", "16", "9", "3"]);
    let mut reader = FeedReader::open(&path).unwrap();
    let sender = reader.sender();
    assert_eq!(sender.name, "scopecreep Sender");
    assert_eq!(sender.version, "c-tool");
    assert_eq!(sender.host, "feed_c_tool");
    for f in 0..3u64 {
        let frame = reader.next_frame().unwrap_or_else(|| panic!("frame {f} missing"));
        assert_eq!(frame.sequence(), f + 1);
        assert_eq!((frame.width(), frame.height(), frame.row_floats()), (16, 9, 64));
        assert!(frame.bottom_up());
        assert_eq!(frame.frame_time(), 100.0 + f as f64);
        assert_eq!(frame.frame_rate(), 25.0);
        assert_eq!(frame.sender_instance(), 7);
        assert!(frame.render_ns() > 0 && frame.publish_ns() >= frame.render_ns());
        let want: Vec<f32> = (0..16 * 9 * 4).map(|i| f as f32 * 1000.0 + i as f32 * 0.25 - 2.0).collect();
        assert_eq!(frame.pixels(), &want[..]);
    }
    assert!(reader.next_frame().is_none());
    let stats = reader.stats();
    assert_eq!((stats.slot_count, stats.frames_published, stats.dropped()), (4, 3, 0));
}

#[test]
fn the_c_writer_overwrites_the_oldest_unread_frame_and_counts_drops() {
    let path = temp_feed("c-overwrite");
    run(&["write", path.to_str().unwrap(), "4", "8", "8", "10"]);
    let mut reader = FeedReader::open(&path).unwrap();
    let times: Vec<f64> = std::iter::from_fn(|| reader.next_frame().map(|f| f.frame_time())).collect();
    assert_eq!(times, [106.0, 107.0, 108.0, 109.0]);
    assert_eq!(reader.stats().drops_overwritten, 6);
}

#[test]
fn frames_written_by_the_rust_writer_are_read_by_the_c_reader() {
    let path = temp_feed("rust-to-c");
    let mut writer = FeedWriter::create(&path, 4, 16 * 9 * 16).unwrap();
    let t = now_ns();
    for f in 0..2u32 {
        let pixels: Vec<f32> = (0..16 * 9 * 4).map(|i| f as f32 + i as f32).collect();
        let meta = FrameMeta { width: 16, height: 9, frame_time: 50.0 + f as f64, frame_rate: 23.976, render_ns: t + f as u64, sender_instance: 2 };
        writer.write_frame(&meta, &pixels).unwrap();
    }
    // per frame: seq width height row_bytes row_order frame_time frame_rate instance first last sum
    let sum = |f: f64| (0..16 * 9 * 4).map(|i| f + i as f64).sum::<f64>();
    let want = format!(
        "sender=scopecreep Sender slots=4\n1 16 9 256 0 50.000 23.976 2 0.0 575.0 {:.1}\n2 16 9 256 0 51.000 23.976 2 1.0 576.0 {:.1}\nframes=2\n",
        sum(0.0),
        sum(1.0)
    );
    assert_eq!(run(&["read", path.to_str().unwrap()]), want);
}

#[test]
fn the_c_reader_reports_a_version_mismatch_and_a_missing_feed() {
    let path = temp_feed("c-version");
    assert_eq!(run(&["read", path.to_str().unwrap()]), "no feed\n");
    let _writer = FeedWriter::create_with_version(&path, 1, 4096, 2, 3).unwrap();
    assert_eq!(run(&["read", path.to_str().unwrap()]), "version mismatch: found 2.3\n");
}

#[test]
fn the_rust_reader_reports_a_c_feed_of_another_version() {
    let path = temp_feed("c-version2");
    run(&["write-version", path.to_str().unwrap(), "3", "1"]);
    assert!(matches!(FeedReader::open(&path), Err(OpenError::VersionMismatch { found: (3, 1), .. })));
}

#[test]
fn the_c_sender_works_only_while_the_rust_reader_is_attached() {
    let path = temp_feed("c-presence");
    // `serve`: publish a Feed, then for every 5 ms tick behave as the Sender does: no reader, no copy.
    // It prints how many frames it copied and how many ticks it idled, and stops after 1.5 s.
    let child = Command::new(c_tool()).args(["serve", path.to_str().unwrap(), "8", "8", "1500"]).stdout(Stdio::piped()).spawn().unwrap();
    wait_for("the C tool to publish its Feed", || path.exists());
    std::thread::sleep(Duration::from_millis(100)); // no reader yet: these ticks must be idle
    {
        let mut reader = FeedReader::open(&path).unwrap();
        let mut seen = 0;
        wait_for("frames from the C tool", || {
            reader.heartbeat(now_ns());
            seen += std::iter::from_fn(|| reader.next_frame()).count();
            seen >= 5
        });
        let idle_before = reader.stats().renders_idle;
        assert!(idle_before >= 5, "the Sender idled while no reader was attached ({idle_before} idle renders)");
    } // reader dropped: detached
    let detached_at = FeedWriterProbe::published(&path);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(FeedWriterProbe::published(&path), detached_at, "no frames are copied once the reader has detached");
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
}

/// Read a counter straight from a Feed file without attaching (attaching would start the Sender copying).
struct FeedWriterProbe;
impl FeedWriterProbe {
    fn published(path: &Path) -> u64 {
        use std::os::unix::fs::FileExt;
        let mut b = [0u8; 8];
        std::fs::File::open(path).unwrap().read_exact_at(&mut b, 328).unwrap();
        u64::from_le_bytes(b)
    }
}

#[test]
fn a_synthetic_test_card_reaches_the_c_reader() {
    let path = temp_feed("card");
    let mut writer = FeedWriter::create(&path, 1, 32 * 18 * 16).unwrap();
    let meta = FrameMeta { width: 32, height: 18, frame_time: 0.0, frame_rate: 24.0, render_ns: now_ns(), sender_instance: 1 };
    writer.write_frame(&meta, &synthetic::test_card(32, 18, 0)).unwrap();
    assert!(run(&["read", path.to_str().unwrap()]).contains("\n1 32 18 512 0 0.000 24.000 1 "));
}
