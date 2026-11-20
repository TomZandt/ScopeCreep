//! The Resolve scripting helper as a real process (app/resolve/colour_setup_helper.py driven by
//! `scopecreep::resolve_helper::ScriptHelper`), with a fake `DaVinciResolveScript` module standing in for Resolve
//! under the system Python. Checks the wire between the app and the helper, and that a stuck helper is recovered.
use scopecreep::project::{Answer, Outcome, ProjectReader};
use scopecreep::resolve_helper::ScriptHelper;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const FAKE_MODULE: &str = r#"
import os, time

class Timeline:
    def GetName(self): return "Timeline 1"
    def GetSetting(self, key=None):
        if os.environ.get("FAKE_RESOLVE_HANG"):
            time.sleep(60)
        s = {"colorScienceMode": "acescct", "colorAcesODT": "Rec.709 BT.1886", "videoDataLevels": "Video",
             "colorSpaceTimeline": "Rec.709 (Scene)", "unrelatedKey": "x"}
        return s if key is None else s.get(key)

class Project:
    def GetName(self): return "scopecreep-test-acescct-rec709"
    def GetCurrentTimeline(self): return Timeline()
    def GetSetting(self, key=None): return {}

class ProjectManager:
    def GetCurrentProject(self): return Project()

class Resolve:
    def GetProductName(self): return "DaVinci Resolve Studio"
    def GetVersionString(self): return "21.2.0.6"
    def IsStudio(self): return True
    def GetProjectManager(self): return ProjectManager()

def scriptapp(name):
    return None if os.environ.get("FAKE_RESOLVE_DOWN") else Resolve()
"#;

fn fake_module_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fake-resolve-script");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("DaVinciResolveScript.py"), FAKE_MODULE).unwrap();
    dir
}

fn helper(env: &[(&str, &str)]) -> ScriptHelper {
    let mut h = ScriptHelper::new("python3").env("PYTHONPATH", fake_module_dir());
    for (k, v) in env {
        h = h.env(k, v);
    }
    h
}

fn wait_answer(h: &mut ScriptHelper, within: Duration) -> Option<Answer> {
    let start = Instant::now();
    while start.elapsed() < within {
        if let Some(a) = h.take_answer() {
            return Some(a);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn the_helper_reads_the_project_and_the_app_understands_it() {
    let mut h = helper(&[]);
    h.request();
    let answer = wait_answer(&mut h, Duration::from_secs(10)).expect("the helper answers");
    let Outcome::Read(reading) = Outcome::from_answer(&answer) else {
        panic!("not read: {answer:?}")
    };
    assert_eq!(reading.project, "scopecreep-test-acescct-rec709");
    assert_eq!(reading.timeline.as_deref(), Some("Timeline 1"));
    assert_eq!(
        (reading.product.as_str(), reading.studio),
        ("DaVinci Resolve Studio 21.2.0.6", true)
    );
    let setup = reading.colour_setup.expect("a Colour Setup");
    assert_eq!(
        (setup.timeline.as_str(), setup.output.as_str()),
        ("ACEScct", "Rec.709 BT.1886")
    );
    assert!(
        !answer.iter().any(|(k, _)| k == "setting.unrelatedKey"),
        "only Colour Setup keys are passed on"
    );
    // A second read goes to the same process.
    h.request();
    assert!(wait_answer(&mut h, Duration::from_secs(10)).is_some());
}

#[test]
fn the_helper_says_when_resolve_cannot_be_reached() {
    let mut h = helper(&[("FAKE_RESOLVE_DOWN", "1")]);
    h.request();
    let answer = wait_answer(&mut h, Duration::from_secs(10)).unwrap();
    match Outcome::from_answer(&answer) {
        Outcome::Gone(why) => assert!(why.contains("External scripting"), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn nothing_is_asked_of_resolve_until_a_read_is_requested() {
    // The helper only starts on the first request; before that there is no process at all.
    let h = helper(&[]);
    assert_eq!(h.running_pid(), None);
}

#[test]
fn a_stuck_helper_is_killed_when_abandoned_and_a_fresh_one_starts_on_the_next_request() {
    let mut h = helper(&[("FAKE_RESOLVE_HANG", "1")]);
    h.request();
    assert!(
        wait_answer(&mut h, Duration::from_millis(1500)).is_none(),
        "the read hangs"
    );
    let pid = h.running_pid().expect("a helper is running");
    let t = Instant::now();
    h.abandon();
    assert!(
        t.elapsed() < Duration::from_millis(500),
        "abandoning does not wait for the stuck call"
    );
    assert_eq!(h.running_pid(), None);
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists()
            || std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .unwrap_or_default()
                .contains(") Z"),
        "the stuck helper is gone"
    );
    h.request();
    assert!(h.running_pid().is_some_and(|p| p != pid), "a fresh helper");
}

#[test]
fn the_helper_inherits_no_open_files_but_its_pipes() {
    // Found in Resolve testing: run under `flock`, an earlier helper inherited the lock file's descriptor and,
    // stuck in a Resolve call, held the lock for everyone. Make an inheritable descriptor and check it is not passed on.
    use std::os::fd::AsRawFd;
    let marker = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("inherited-marker");
    let file = std::fs::File::create(&marker).unwrap();
    // SAFETY: clearing close-on-exec on a descriptor this test owns.
    unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) };
    let mut h = helper(&[("FAKE_RESOLVE_HANG", "1")]);
    h.request();
    std::thread::sleep(Duration::from_millis(500));
    let pid = h.running_pid().unwrap();
    let open: Vec<PathBuf> = std::fs::read_dir(format!("/proc/{pid}/fd"))
        .unwrap()
        .filter_map(|e| std::fs::read_link(e.ok()?.path()).ok())
        .collect();
    assert!(!open.contains(&marker), "the helper holds {open:?}");
    h.abandon();
}

#[test]
fn a_missing_resolve_python_is_reported_not_crashed_on() {
    let mut h = ScriptHelper::new("/nonexistent/ResolvePython");
    h.request();
    let answer = wait_answer(&mut h, Duration::from_secs(2)).unwrap();
    match Outcome::from_answer(&answer) {
        Outcome::Gone(why) => assert!(why.contains("/nonexistent/ResolvePython"), "{why}"),
        other => panic!("{other:?}"),
    }
}
