//! More than one Sender: the registry in the Feed header (protocol 1.2, feed/PROTOCOL.md, "More than one Sender").
mod common;
use common::temp_feed;
use scopecreep_feed::{FeedReader, FeedWriter, FrameMeta, SenderIdentity, WriteError, synthetic};
use std::process::{Child, Command};

fn meta(instance: u32, frame_time: f64, render_ns: u64) -> FrameMeta {
    FrameMeta {
        width: 8,
        height: 8,
        frame_time,
        frame_rate: 24.0,
        render_ns,
        sender_instance: instance,
    }
}

fn me() -> u64 {
    std::process::id() as u64
}

/// Another "Resolve": a real process that is alive until killed.
struct OtherProcess(Child);
impl OtherProcess {
    fn start() -> OtherProcess {
        OtherProcess(Command::new("sleep").arg("30").spawn().unwrap())
    }
    fn pid(&self) -> u64 {
        self.0.id() as u64
    }
    fn quit(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Drop for OtherProcess {
    fn drop(&mut self) {
        self.quit();
    }
}

fn identity(pid: u64, host: &str) -> SenderIdentity {
    SenderIdentity {
        pid,
        host: host.into(),
    }
}

#[test]
fn every_sender_instance_is_listed_in_the_registry() {
    let path = temp_feed("listed");
    let mut w = FeedWriter::create(&path, 4, 8 * 8 * 16).unwrap();
    w.register(1, 24.0);
    w.register(2, 25.0);
    let r = FeedReader::open(&path).unwrap();
    let reg = r.registry();
    assert_eq!(
        reg.iter()
            .map(|e| (e.pid, e.instance, e.frame_rate))
            .collect::<Vec<_>>(),
        [(me(), 1, 24.0), (me(), 2, 25.0)]
    );
    assert!(
        reg.iter()
            .all(|e| e.host == "synthetic frame writer" && e.renders == 0)
    );
}

#[test]
fn a_render_is_recorded_against_its_instance() {
    let path = temp_feed("noted");
    let mut w = FeedWriter::create(&path, 4, 8 * 8 * 16).unwrap();
    w.register(1, 24.0);
    w.write_frame(&meta(1, 86_410.0, 5_000), &synthetic::solid(8, 8, [0.0; 4]))
        .unwrap();
    w.write_frame(&meta(1, 86_411.0, 6_000), &synthetic::solid(8, 8, [0.0; 4]))
        .unwrap();
    let r = FeedReader::open(&path).unwrap();
    let e = &r.registry()[0];
    assert_eq!(
        (e.renders, e.last_render_ns, e.last_frame_time),
        (2, 6_000, 86_411.0)
    );
}

#[test]
fn a_destroyed_instance_leaves_the_registry() {
    let path = temp_feed("destroyed");
    let mut w = FeedWriter::create(&path, 4, 8 * 8 * 16).unwrap();
    w.register(1, 24.0);
    w.register(2, 24.0);
    w.unregister(1);
    let r = FeedReader::open(&path).unwrap();
    assert_eq!(
        r.registry().iter().map(|e| e.instance).collect::<Vec<_>>(),
        [2]
    );
}

#[test]
fn the_registry_and_counters_carry_over_when_the_sender_replaces_its_feed() {
    let path = temp_feed("carried");
    let mut w = FeedWriter::create(&path, 0, 0).unwrap();
    w.register(3, 50.0);
    w.skip_frame();
    w.replace(4, 8 * 8 * 16).unwrap();
    let r = FeedReader::open(&path).unwrap();
    assert_eq!(
        r.registry()
            .iter()
            .map(|e| (e.instance, e.frame_rate))
            .collect::<Vec<_>>(),
        [(3, 50.0)]
    );
    assert_eq!((r.stats().slot_count, r.stats().drops_busy), (4, 1));
}

#[test]
fn a_second_resolve_joins_the_feed_as_a_guest_and_sends_no_frames() {
    let path = temp_feed("guest");
    let resolve_a = OtherProcess::start();
    let mut owner = FeedWriter::start(&path, identity(resolve_a.pid(), "Resolve A")).unwrap();
    owner.replace(4, 8 * 8 * 16).unwrap();
    owner.register(1, 24.0);
    let mut guest = FeedWriter::start(&path, identity(me(), "Resolve B")).unwrap();
    assert!(!owner.is_guest());
    assert!(
        guest.is_guest(),
        "the Feed already belongs to a running Resolve"
    );
    guest.register(1, 25.0);
    assert_eq!(
        guest.write_frame(&meta(1, 7.0, 9_000), &synthetic::solid(8, 8, [0.0; 4])),
        Err(WriteError::Guest)
    );

    let mut r = FeedReader::open(&path).unwrap();
    assert_eq!(
        r.sender().pid,
        resolve_a.pid(),
        "the first Resolve keeps the Feed"
    );
    assert!(r.next_frame().is_none(), "a guest copies nothing");
    let reg = r.registry();
    assert_eq!(
        reg.iter()
            .map(|e| (
                e.pid,
                e.instance,
                e.host.as_str(),
                e.renders,
                e.last_render_ns
            ))
            .collect::<Vec<_>>(),
        [
            (resolve_a.pid(), 1, "Resolve A", 0, 0),
            (me(), 1, "Resolve B", 1, 9_000)
        ],
        "but its renders are recorded, so the app can flag it"
    );
    assert_eq!(
        r.stats().renders_total,
        0,
        "and they are not counted as the owner's"
    );
}

#[test]
fn a_feed_left_by_a_resolve_that_has_quit_is_replaced() {
    let path = temp_feed("stale");
    let mut gone = OtherProcess::start();
    FeedWriter::start(&path, identity(gone.pid(), "Resolve A")).unwrap();
    gone.quit();
    let w = FeedWriter::start(&path, identity(me(), "Resolve B")).unwrap();
    assert!(!w.is_guest());
    assert_eq!(FeedReader::open(&path).unwrap().sender().pid, me());
}

#[test]
fn a_guest_takes_the_feed_over_when_the_first_resolve_quits() {
    let path = temp_feed("takeover");
    let mut resolve_a = OtherProcess::start();
    let mut owner = FeedWriter::start(&path, identity(resolve_a.pid(), "Resolve A")).unwrap();
    owner.register(1, 24.0);
    let mut guest = FeedWriter::start(&path, identity(me(), "Resolve B")).unwrap();
    guest.register(2, 25.0);
    let mut r = FeedReader::open(&path).unwrap();

    guest.refresh();
    assert!(
        guest.is_guest(),
        "nothing changes while the first Resolve runs"
    );
    resolve_a.quit();
    guest.refresh();
    assert!(!guest.is_guest());
    assert!(r.retired(), "the app is told to re-open the Feed");
    r = FeedReader::open(&path).unwrap();
    assert_eq!(r.sender().pid, me());
    assert_eq!(r.sender().host, "Resolve B");
    assert_eq!(
        r.registry()
            .iter()
            .map(|e| (e.pid, e.instance))
            .collect::<Vec<_>>(),
        [(me(), 2)],
        "the dead Resolve's instances are gone"
    );
}

#[test]
fn a_guest_follows_the_owner_to_its_new_feed_file() {
    let path = temp_feed("follow");
    let resolve_a = OtherProcess::start();
    let mut owner = FeedWriter::start(&path, identity(resolve_a.pid(), "Resolve A")).unwrap();
    let mut guest = FeedWriter::start(&path, identity(me(), "Resolve B")).unwrap();
    guest.register(1, 25.0);
    owner.replace(4, 8 * 8 * 16).unwrap(); // the owner builds its ring: a new Feed file
    guest
        .write_frame(&meta(1, 3.0, 1_000), &synthetic::solid(8, 8, [0.0; 4]))
        .unwrap_err();
    guest.refresh();
    guest
        .write_frame(&meta(1, 4.0, 2_000), &synthetic::solid(8, 8, [0.0; 4]))
        .unwrap_err();
    let r = FeedReader::open(&path).unwrap();
    let e = r
        .registry()
        .into_iter()
        .find(|e| e.pid == me())
        .expect("the guest is still registered in the new file");
    assert_eq!(e.last_render_ns, 2_000);
}

#[test]
fn a_full_registry_is_not_an_error() {
    let path = temp_feed("full");
    let mut w = FeedWriter::create(&path, 4, 8 * 8 * 16).unwrap();
    for i in 1..=40 {
        w.register(i, 24.0);
    }
    let r = FeedReader::open(&path).unwrap();
    assert_eq!(r.registry().len(), scopecreep_feed::REGISTRY_ENTRIES);
}
