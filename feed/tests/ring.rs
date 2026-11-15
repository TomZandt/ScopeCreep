//! The Feed as the app and a test harness use it: frames written with the synthetic writer, read with the reader.
mod common;
use common::{forge_header_u64, temp_feed};
use scopecreep_feed::{FeedReader, FeedWriter, FrameMeta, OpenError, Provenance, READER_TIMEOUT_NS, SentColourSetup, WriteError, now_ns, synthetic};

fn meta(width: u32, height: u32, frame_time: f64) -> FrameMeta {
    FrameMeta { width, height, frame_time, frame_rate: 24.0, render_ns: now_ns(), sender_instance: 1 }
}

#[test]
fn a_frame_written_to_the_feed_is_read_back_unchanged() {
    let path = temp_feed("roundtrip");
    let mut writer = FeedWriter::create(&path, 4, 64 * 36 * 16).unwrap();
    let mut reader = FeedReader::open(&path).unwrap();
    let pixels = synthetic::awkward_values(64, 36);
    writer.write_frame(&meta(64, 36, 86400.0), &pixels).unwrap();

    let frame = reader.next_frame().expect("one frame is waiting");
    assert_eq!(frame.sequence(), 1);
    assert_eq!((frame.width(), frame.height()), (64, 36));
    assert_eq!(frame.frame_time(), 86400.0);
    assert_eq!(frame.frame_rate(), 24.0);
    assert_eq!(frame.sender_instance(), 1);
    // bit for bit, including negative, over-range and denormal values
    let got: Vec<u32> = frame.pixels().iter().map(|f| f.to_bits()).collect();
    let want: Vec<u32> = pixels.iter().map(|f| f.to_bits()).collect();
    assert_eq!(got, want);
    drop(frame);
    assert!(reader.next_frame().is_none(), "the frame is delivered once");
}

#[test]
fn frames_arrive_in_the_order_they_were_written() {
    let path = temp_feed("order");
    let mut writer = FeedWriter::create(&path, 4, 8 * 8 * 16).unwrap();
    let mut reader = FeedReader::open(&path).unwrap();
    for t in 0..3 {
        writer.write_frame(&meta(8, 8, t as f64), &synthetic::solid(8, 8, [t as f32; 4])).unwrap();
    }
    let times: Vec<f64> = std::iter::from_fn(|| reader.next_frame().map(|f| f.frame_time())).collect();
    assert_eq!(times, [0.0, 1.0, 2.0]);
}

#[test]
fn a_full_ring_overwrites_the_oldest_unread_frame_and_counts_a_drop() {
    let path = temp_feed("overwrite");
    let mut writer = FeedWriter::create(&path, 4, 8 * 8 * 16).unwrap();
    let mut reader = FeedReader::open(&path).unwrap();
    for t in 0..6 {
        // the writer never waits and never fails because the reader is slow
        writer.write_frame(&meta(8, 8, t as f64), &synthetic::solid(8, 8, [t as f32; 4])).unwrap();
    }
    let got: Vec<(u64, f64, f32)> =
        std::iter::from_fn(|| reader.next_frame().map(|f| (f.sequence(), f.frame_time(), f.pixels()[0]))).collect();
    assert_eq!(got, [(3, 2.0, 2.0), (4, 3.0, 3.0), (5, 4.0, 4.0), (6, 5.0, 5.0)], "the two oldest frames were lost");
    let stats = reader.stats();
    assert_eq!(stats.dropped(), 2);
    assert_eq!(stats.drops_overwritten, 2);
    assert_eq!(stats.frames_published, 6);
}

#[test]
fn a_frame_being_read_is_never_overwritten() {
    let path = temp_feed("held");
    let mut writer = FeedWriter::create(&path, 2, 8 * 8 * 16).unwrap();
    let mut reader = FeedReader::open(&path).unwrap();
    writer.write_frame(&meta(8, 8, 0.0), &synthetic::solid(8, 8, [0.25; 4])).unwrap();
    let held = reader.next_frame().unwrap();
    for t in 1..5 {
        writer.write_frame(&meta(8, 8, t as f64), &synthetic::solid(8, 8, [9.0; 4])).unwrap();
    }
    assert!(held.pixels().iter().all(|&v| v == 0.25), "the held frame is untouched");
    assert_eq!(held.frame_time(), 0.0);
    drop(held);
    assert_eq!(reader.next_frame().unwrap().frame_time(), 4.0, "the one other slot holds the newest frame");
    assert_eq!(reader.stats().dropped(), 3);
}

#[test]
fn when_every_slot_is_in_use_the_frame_is_skipped_and_counted() {
    let path = temp_feed("busy");
    let mut writer = FeedWriter::create(&path, 1, 8 * 8 * 16).unwrap();
    let mut reader = FeedReader::open(&path).unwrap();
    writer.write_frame(&meta(8, 8, 0.0), &synthetic::solid(8, 8, [0.0; 4])).unwrap();
    let held = reader.next_frame().unwrap();
    assert_eq!(writer.write_frame(&meta(8, 8, 1.0), &synthetic::solid(8, 8, [0.0; 4])), Err(WriteError::Dropped));
    assert_eq!(reader.stats().drops_busy, 1);
    drop(held);
}

#[test]
fn the_sender_sees_a_reader_only_while_one_is_attached() {
    let path = temp_feed("present");
    let writer = FeedWriter::create(&path, 2, 4096).unwrap();
    assert!(!writer.reader_present(now_ns()), "no app yet: the Sender must do no work");
    let reader = FeedReader::open(&path).unwrap();
    assert!(writer.reader_present(now_ns()));
    drop(reader);
    assert!(!writer.reader_present(now_ns()), "a clean exit detaches at once");
}

#[test]
fn a_reader_that_was_killed_stops_counting_as_present_within_a_second() {
    let path = temp_feed("killed");
    let writer = FeedWriter::create(&path, 2, 4096).unwrap();
    let reader = FeedReader::open(&path).unwrap();
    let t = now_ns();
    reader.heartbeat(t);
    std::mem::forget(reader); // killed: no clean detach, no more heartbeats
    assert!(writer.reader_present(t + READER_TIMEOUT_NS - 1));
    assert!(!writer.reader_present(t + READER_TIMEOUT_NS));
}

#[test]
fn a_new_reader_reclaims_the_slot_a_killed_reader_was_holding() {
    let path = temp_feed("reclaim");
    let mut writer = FeedWriter::create(&path, 1, 8 * 8 * 16).unwrap();
    let mut first = FeedReader::open(&path).unwrap();
    writer.write_frame(&meta(8, 8, 0.0), &synthetic::solid(8, 8, [0.0; 4])).unwrap();
    std::mem::forget(first.next_frame().unwrap()); // killed while reading
    first.heartbeat(now_ns() - 2 * READER_TIMEOUT_NS);
    std::mem::forget(first);
    let mut second = FeedReader::open(&path).unwrap();
    writer.write_frame(&meta(8, 8, 7.0), &synthetic::solid(8, 8, [0.0; 4])).expect("the slot is usable again");
    assert_eq!(second.next_frame().unwrap().frame_time(), 7.0);
}

#[test]
fn a_second_app_cannot_attach_while_another_is_using_the_feed() {
    let path = temp_feed("second");
    let _writer = FeedWriter::create(&path, 1, 4096).unwrap();
    let first = FeedReader::open(&path).unwrap();
    forge_header_u64(&path, 264, 1); // pretend the attached reader is another process
    match FeedReader::open(&path) {
        Err(OpenError::Busy { pid }) => assert_eq!(pid, 1),
        other => panic!("expected Busy, got {:?}", other.err()),
    }
    drop(first);
}

#[test]
fn a_missing_feed_is_reported_as_no_feed() {
    let path = temp_feed("missing");
    assert!(matches!(FeedReader::open(&path), Err(OpenError::NoFeed)));
}

#[test]
fn a_file_that_is_not_a_feed_is_refused() {
    let path = temp_feed("garbage");
    std::fs::write(&path, vec![0x5au8; 8192]).unwrap();
    assert!(matches!(FeedReader::open(&path), Err(OpenError::NotAFeed)));
    std::fs::write(&path, b"short").unwrap();
    assert!(matches!(FeedReader::open(&path), Err(OpenError::NotAFeed)));
}

#[test]
fn another_major_version_is_reported_with_both_versions() {
    let path = temp_feed("version");
    let _writer = FeedWriter::create_with_version(&path, 2, 4096, 2, 3).unwrap();
    match FeedReader::open(&path) {
        Err(OpenError::VersionMismatch { found, supported }) => {
            assert_eq!(found, (2, 3));
            assert_eq!(supported, (scopecreep_feed::VERSION_MAJOR, scopecreep_feed::VERSION_MINOR));
        }
        other => panic!("expected VersionMismatch, got {:?}", other.err()),
    }
}

#[test]
fn a_newer_minor_version_is_compatible() {
    let path = temp_feed("minor");
    let mut writer = FeedWriter::create_with_version(&path, 2, 8 * 8 * 16, scopecreep_feed::VERSION_MAJOR, 9).unwrap();
    let mut reader = FeedReader::open(&path).expect("minor versions only add fields");
    writer.write_frame(&meta(8, 8, 1.0), &synthetic::solid(8, 8, [0.5; 4])).unwrap();
    assert!(reader.next_frame().is_some());
    assert_eq!(reader.sender().protocol, (scopecreep_feed::VERSION_MAJOR, 9));
}

#[test]
fn a_reader_learns_that_the_sender_replaced_the_feed() {
    let path = temp_feed("retired");
    let _small = FeedWriter::create(&path, 0, 0).unwrap();
    let reader = FeedReader::open(&path).unwrap();
    assert!(!reader.retired());
    assert_eq!(reader.stats().slot_count, 0, "a Feed with no ring yet is valid");
    let mut big = FeedWriter::create(&path, 4, 16 * 16 * 16).unwrap();
    assert!(reader.retired(), "the old file is marked retired");
    assert!(!big.reader_present(now_ns()), "the reader has not attached to the new Feed yet");
    let mut reader = FeedReader::open(&path).unwrap();
    assert!(big.reader_present(now_ns()));
    big.write_frame(&meta(16, 16, 3.0), &synthetic::solid(16, 16, [0.5; 4])).unwrap();
    assert_eq!(reader.next_frame().unwrap().width(), 16);
}

#[test]
fn a_frame_larger_than_a_slot_is_refused_not_truncated() {
    let path = temp_feed("toolarge");
    let mut writer = FeedWriter::create(&path, 2, 8 * 8 * 16).unwrap();
    assert_eq!(writer.write_frame(&meta(64, 64, 0.0), &synthetic::solid(64, 64, [0.0; 4])), Err(WriteError::TooLarge));
}

#[test]
fn the_reader_can_tell_who_is_sending() {
    let path = temp_feed("identity");
    let writer = FeedWriter::create(&path, 1, 4096).unwrap();
    let reader = FeedReader::open(&path).unwrap();
    let sender = reader.sender();
    assert_eq!(sender.name, "scopecreep Sender");
    assert_eq!(sender.pid, std::process::id() as u64);
    assert!(reader.sender_alive());
    assert_eq!(reader.sender_error(), None);
    writer.set_sender_error("cudaHostRegister: out of memory");
    assert_eq!(reader.sender_error().as_deref(), Some("cudaHostRegister: out of memory"));
}

// ---- the Colour Setup block (protocol 1.1)

fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn a_frame_carries_the_colour_setup_the_sender_set_and_its_provenance() {
    let path = temp_feed("colour-manual");
    let mut writer = FeedWriter::create(&path, 4, 8 * 8 * 16).unwrap();
    let mut reader = FeedReader::open(&path).unwrap();
    writer.write_frame(&meta(8, 8, 1.0), &synthetic::solid(8, 8, [0.0; 4])).unwrap();
    writer.set_colour_setup(Some(SentColourSetup::new(Provenance::Manual, "timeline=ACEScct\noutput=Rec.709 BT.1886\nlevels=video\naces_version=1.3\n")));
    writer.write_frame(&meta(8, 8, 2.0), &synthetic::solid(8, 8, [0.0; 4])).unwrap();
    writer.set_colour_setup(None);
    writer.write_frame(&meta(8, 8, 3.0), &synthetic::solid(8, 8, [0.0; 4])).unwrap();

    assert_eq!(reader.next_frame().unwrap().colour_setup(), None, "a Sender with no Colour Setup of its own sends none");
    let sent = reader.next_frame().unwrap().colour_setup().expect("the manual Colour Setup travels with the frame");
    assert_eq!(sent.provenance, Provenance::Manual);
    assert_eq!(sent.entries(), kv(&[("timeline", "ACEScct"), ("output", "Rec.709 BT.1886"), ("levels", "video"), ("aces_version", "1.3")]));
    assert_eq!(sent.get("output"), Some("Rec.709 BT.1886"));
    assert_eq!(reader.next_frame().unwrap().colour_setup(), None, "and stops when the Sender clears it");
}

#[test]
fn the_colour_setup_block_is_read_leniently() {
    // Blank lines, lines without '=', and spaces around keys and values are tolerated; inner spaces are kept.
    let sent = SentColourSetup::new(Provenance::Project, "\n timeline = DaVinci WG/Intermediate\nnonsense\nfuture_key=x\n\noutput=P3-D65");
    assert_eq!(sent.get("timeline"), Some("DaVinci WG/Intermediate"));
    assert_eq!(sent.get("output"), Some("P3-D65"));
    assert_eq!(sent.get("future_key"), Some("x"));
    assert_eq!(sent.get("nonsense"), None);
}

#[test]
fn this_crate_speaks_feed_protocol_1_2() {
    assert_eq!((scopecreep_feed::VERSION_MAJOR, scopecreep_feed::VERSION_MINOR), (1, 2));
}

#[test]
fn a_reader_accepts_a_1_0_feed_and_finds_no_colour_setup_in_it() {
    let path = temp_feed("colour-v10");
    let mut writer = FeedWriter::create_with_version(&path, 4, 8 * 8 * 16, 1, 0).unwrap();
    let mut reader = FeedReader::open(&path).unwrap();
    writer.write_frame(&meta(8, 8, 1.0), &synthetic::solid(8, 8, [0.0; 4])).unwrap();
    assert_eq!(reader.sender().protocol, (1, 0));
    assert_eq!(reader.next_frame().unwrap().colour_setup(), None);
}

#[test]
fn a_colour_setup_block_with_an_impossible_size_or_unknown_provenance_is_ignored() {
    let path = temp_feed("colour-forged");
    let mut writer = FeedWriter::create(&path, 1, 8 * 8 * 16).unwrap();
    let mut reader = FeedReader::open(&path).unwrap();
    // slot 0's header starts at 4096: provenance at +1024, byte count at +1028
    writer.write_frame(&meta(8, 8, 1.0), &synthetic::solid(8, 8, [0.0; 4])).unwrap();
    forge_header_u64(&path, 4096 + 1024, 2 | (5000u64 << 32));
    assert_eq!(reader.next_frame().unwrap().colour_setup(), None, "more bytes than the block holds");
    writer.write_frame(&meta(8, 8, 2.0), &synthetic::solid(8, 8, [0.0; 4])).unwrap();
    forge_header_u64(&path, 4096 + 1024, 9 | (4u64 << 32));
    assert_eq!(reader.next_frame().unwrap().colour_setup(), None, "a provenance this reader does not know");
}
