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

/// The system Python the fake module runs under (`python3` is not a command on a stock Windows install).
const PYTHON: &str = if cfg!(windows) { "python" } else { "python3" };

fn helper(env: &[(&str, &str)]) -> ScriptHelper {
    let mut h = ScriptHelper::new(PYTHON).env("PYTHONPATH", fake_module_dir());
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
fn the_helper_finds_the_scripting_module_where_resolve_api_says() {
    // A Python other than Resolve's (on Windows, the one the user installed) cannot import the module by itself;
    // Resolve's documented RESOLVE_SCRIPT_API names the folder holding Modules/.
    let api = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fake-resolve-api");
    std::fs::create_dir_all(api.join("Modules")).unwrap();
    std::fs::write(
        api.join("Modules").join("DaVinciResolveScript.py"),
        FAKE_MODULE,
    )
    .unwrap();
    let mut h = ScriptHelper::new(PYTHON).env("RESOLVE_SCRIPT_API", &api);
    h.request();
    let answer = wait_answer(&mut h, Duration::from_secs(10)).expect("the helper answers");
    assert!(
        matches!(Outcome::from_answer(&answer), Outcome::Read(_)),
        "{answer:?}"
    );
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

#[cfg(target_os = "linux")] // reads /proc
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

/// Set in the copy of this test binary that plays the app in the test below.
const FAKE_APP_ENV: &str = "SCOPECREEP_TEST_FAKE_APP";

#[test]
#[ignore = "run only as the app process of the_helper_dies_with_the_app_even_when_the_app_is_killed"]
fn fake_app_holding_a_stuck_helper() {
    if std::env::var_os(FAKE_APP_ENV).is_none() {
        return;
    }
    let mut h = helper(&[("FAKE_RESOLVE_HANG", "1")]);
    h.request();
    // Well into the stuck read, so it has nothing left to write: a helper killed by a broken pipe proves nothing.
    std::thread::sleep(Duration::from_millis(1000));
    println!("helper={}", h.running_pid().expect("a helper is running"));
    std::thread::sleep(Duration::from_secs(60));
}

#[test]
fn the_helper_dies_with_the_app_even_when_the_app_is_killed() {
    // The app is killed outright (a crash, or Task Manager): no Drop runs, so only the OS can take the helper,
    // stuck in a Resolve call, down with it (Linux: parent-death signal; Windows: a kill-on-close job object).
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let mut app = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "fake_app_holding_a_stuck_helper",
            "--ignored",
            "--nocapture",
        ])
        .env(FAKE_APP_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let helper_pid: u64 = std::io::BufReader::new(app.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
        .find_map(|l| l.strip_prefix("helper=").map(|p| p.trim().parse().unwrap()))
        .expect("the fake app reports its helper");
    assert!(
        scopecreep_feed::process_alive(helper_pid),
        "the helper is running"
    );
    app.kill().unwrap();
    app.wait().unwrap();
    let t = Instant::now();
    while scopecreep_feed::process_alive(helper_pid) && t.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !scopecreep_feed::process_alive(helper_pid),
        "the helper outlived the app"
    );
}
