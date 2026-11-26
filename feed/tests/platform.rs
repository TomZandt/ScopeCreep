//! The Feed's platform seam: the protocol clock and the "is the Sender alive" check.
use scopecreep_feed::{now_ns, process_alive};

#[test]
fn this_process_is_alive_and_pid_zero_is_nobody() {
    assert!(process_alive(std::process::id() as u64));
    assert!(!process_alive(0));
    assert!(!process_alive(u64::MAX), "a pid no OS hands out");
}

#[test]
fn a_finished_process_is_not_alive() {
    let mut child = std::process::Command::new("true").spawn().expect("spawn");
    let pid = child.id() as u64;
    child.wait().expect("wait");
    assert!(!process_alive(pid));
}

#[test]
fn the_protocol_clock_never_goes_back() {
    let a = now_ns();
    let b = now_ns();
    assert!(a > 0 && b >= a);
}
