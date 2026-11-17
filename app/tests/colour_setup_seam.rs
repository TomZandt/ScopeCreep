//! The Colour Setup the app shows (ticket #16), driven at the Feed seam: a synthetic frame writer stands in for
//! Resolve and the Sender (whose manual override travels in the Feed), and a fake project reader stands in for the
//! Resolve scripting helper, answering in the helper's own wire format (`key=value` lines, see
//! app/resolve/colour_setup_helper.py). No Resolve is needed.
use scopecreep::monitor::Monitor;
use scopecreep::project::{Answer, ProjectReader};
use scopecreep_feed::{FeedWriter, FrameMeta, Provenance, SentColourSetup, now_ns, synthetic};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const MS: u64 = 1_000_000;
const SECOND: u64 = 1_000_000_000;
const FRAME_24: u64 = SECOND / 24;

/// What the fake helper has been asked, and what it will answer.
#[derive(Default)]
struct Script {
    requests: u64,
    abandoned: u64,
    /// The answer the next request gets; `None` = never answers (a stuck scripting call).
    answer: Option<Answer>,
    pending: bool,
    /// Requests made while frames were arriving (playback), as judged by the bench.
    requests_during_playback: u64,
    playing: bool,
}

struct FakeHelper(Arc<Mutex<Script>>);

impl ProjectReader for FakeHelper {
    fn request(&mut self) {
        let mut s = self.0.lock().unwrap();
        s.requests += 1;
        if s.playing {
            s.requests_during_playback += 1;
        }
        s.pending = true;
    }
    fn take_answer(&mut self) -> Option<Answer> {
        let mut s = self.0.lock().unwrap();
        if !s.pending {
            return None;
        }
        let a = s.answer.clone()?;
        s.pending = false;
        Some(a)
    }
    fn abandon(&mut self) {
        let mut s = self.0.lock().unwrap();
        s.abandoned += 1;
        s.pending = false;
    }
}

fn answer(lines: &[(&str, &str)]) -> Answer {
    lines.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// The helper's answer for a project, with the settings Resolve 21.2 reported for it (#6's closing comment and
/// ~/scopecreep-testdata/dumps): only the keys that differ from YRGB defaults are given per setup.
fn project_answer(project: &str, settings: &[(&str, &str)]) -> Answer {
    let mut a = answer(&[
        ("result", "ok"),
        ("product", "DaVinci Resolve Studio"),
        ("version", "21.2.0.6"),
        ("studio", "1"),
        ("project", project),
        ("timeline", "Timeline 1"),
    ]);
    let defaults = [
        ("colorScienceMode", "davinciYRGB"),
        ("separateColorSpaceAndGamma", "0"),
        ("colorSpaceTimeline", "Rec.709 Gamma 2.4"),
        ("colorSpaceTimelineGamma", ""),
        ("colorSpaceOutput", "Rec.709 Gamma 2.4"),
        ("colorSpaceOutputGamma", ""),
        ("colorSpaceOutputToneMapping", "None"),
        ("colorSpaceOutputGamutMapping", "None"),
        ("inputDRT", "None"),
        ("outputDRT", "None"),
        ("colorAcesIDT", "No Input Transform"),
        ("colorAcesODT", "No Output Transform"),
        ("colorAcesGamutCompressType", "None"),
        ("videoDataLevels", "Video"),
    ];
    for (k, v) in defaults {
        let v = settings.iter().find(|(sk, _)| *sk == k).map_or(v, |(_, sv)| *sv);
        a.push((format!("setting.{k}"), v.to_string()));
    }
    a
}

fn dwg(output: &str, tone_mapping: &str) -> Vec<(&'static str, String)> {
    vec![
        ("colorScienceMode", "davinciYRGBColorManagedv2".into()),
        ("colorSpaceTimeline", "DaVinci WG/Intermediate".into()),
        ("colorSpaceOutput", output.into()),
        ("colorSpaceOutputToneMapping", tone_mapping.into()),
    ]
}

fn aces(odt: &str) -> Vec<(&'static str, String)> {
    // In ACES mode Resolve still reports the YRGB tags (Rec.709 (Scene)); they mean nothing.
    vec![
        ("colorScienceMode", "acescct".into()),
        ("colorSpaceTimeline", "Rec.709 (Scene)".into()),
        ("colorSpaceOutput", "Rec.709 (Scene)".into()),
        ("colorAcesODT", odt.into()),
    ]
}

struct Bench {
    path: PathBuf,
    writer: Option<FeedWriter>,
    monitor: Monitor,
    script: Arc<Mutex<Script>>,
    now: u64,
    frame: f64,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-colour-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let script = Arc::new(Mutex::new(Script::default()));
        let monitor = Monitor::with_project_reader(&path, Box::new(FakeHelper(script.clone())));
        Bench { path, writer: None, monitor, script, now: now_ns(), frame: 0.0 }
    }
    /// A Sender is loaded and Resolve shows one frame.
    fn with_sender(name: &str) -> Bench {
        let mut b = Bench::new(name);
        b.writer = Some(FeedWriter::create(&b.path, 4, 16 * 9 * 16).unwrap());
        b.poll();
        b.render();
        b
    }
    fn helper_answers(&self, a: Answer) {
        self.script.lock().unwrap().answer = Some(a);
    }
    fn helper_hangs(&self) {
        self.script.lock().unwrap().answer = None;
    }
    fn requests(&self) -> u64 {
        self.script.lock().unwrap().requests
    }
    fn poll(&mut self) {
        self.monitor.poll(self.now, &mut |_| {});
    }
    fn wait(&mut self, ns: u64) {
        let end = self.now + ns;
        while self.now < end {
            self.now = (self.now + MS).min(end);
            self.poll();
        }
    }
    /// Resolve renders the next frame now.
    fn render(&mut self) {
        self.frame += 1.0;
        let meta = FrameMeta { width: 16, height: 9, frame_time: self.frame, frame_rate: 24.0, render_ns: self.now, sender_instance: 1 };
        let _ = self.writer.as_mut().unwrap().write_frame(&meta, &synthetic::solid(16, 9, [0.5; 4]));
        self.poll();
    }
    /// Resolve plays for `seconds` at 24 fps.
    fn play(&mut self, seconds: f64) {
        self.script.lock().unwrap().playing = true;
        for _ in 0..(seconds * 24.0) as u32 {
            self.render();
            self.wait(FRAME_24);
        }
        self.script.lock().unwrap().playing = false;
    }
    /// The Sender's manual override, as the Sender node would set it.
    fn set_manually(&mut self, text: Option<&str>) {
        self.writer.as_mut().unwrap().set_colour_setup(text.map(|t| SentColourSetup::new(Provenance::Manual, t)));
    }
    fn label(&self) -> String {
        let status = self.monitor.status();
        scopecreep::picture_view::PictureView::label_for(status.picture.as_ref().map(|p| [p.width, p.height]), &status.display_context())
    }
    fn colour_label(&self) -> Option<String> {
        self.monitor.status().colour_setup().map(|c| c.label())
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

#[test]
fn the_project_colour_setup_is_read_once_playback_stops_and_labelled_from_project() {
    let mut b = Bench::with_sender("from-project");
    let settings = dwg("Rec.709 Gamma 2.4", "None");
    b.helper_answers(project_answer("scopecreep-test-dwg-rec709", &settings.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>()));
    b.play(5.0);
    assert_eq!(b.requests(), 0, "Resolve is never asked by script while it plays");
    assert!(b.label().contains("Colour Setup read when playback stops"), "{}", b.label());
    b.wait(SECOND + 100 * MS);
    assert_eq!(b.requests(), 1, "asked once playback has been stopped for a second");
    assert_eq!(b.colour_label().as_deref(), Some("DaVinci WG/Intermediate → Rec.709 Gamma 2.4 (from project)"));
    assert_eq!(b.label(), "Clean picture  ·  16 × 9  ·  DaVinci WG/Intermediate → Rec.709 Gamma 2.4 (from project)  ·  Output Conversion exact");
}

#[test]
fn every_harness_test_project_is_labelled_correctly() {
    type Case = (&'static str, Vec<(&'static str, String)>, &'static str);
    let cases: Vec<Case> = vec![
        ("yrgb-rec709g24", vec![], "Rec.709 Gamma 2.4 → Rec.709 Gamma 2.4 (from project)"),
        ("dwg-rec709", dwg("Rec.709 Gamma 2.4", "None"), "DaVinci WG/Intermediate → Rec.709 Gamma 2.4 (from project)"),
        ("dwg-p3d65", dwg("P3-D65", "None"), "DaVinci WG/Intermediate → P3-D65 (gamma 2.6) (from project)"),
        ("dwg-pq-tmoff", dwg("Rec.2100 ST2084", "None"), "DaVinci WG/Intermediate → Rec.2100 ST2084 (from project)"),
        ("dwg-pq-tmon", dwg("Rec.2100 ST2084", "DaVinci"), "DaVinci WG/Intermediate → Rec.2100 ST2084, DaVinci tone mapping (from project)"),
        ("acescct-rec709", aces("Rec.709 BT.1886"), "ACEScct → Rec.709 BT.1886 (from project, assumes ACES 2.0)"),
        ("acescct-p3d65", aces("P3-D65"), "ACEScct → P3-D65 (gamma 2.6) (from project, assumes ACES 2.0)"),
        ("acescct-pq", aces("Rec.2100 ST2084 (1000 nit)"), "ACEScct → Rec.2100 ST2084 (1000 nit) (from project, assumes ACES 2.0)"),
    ];
    for (key, settings, want) in cases {
        let mut b = Bench::with_sender(key);
        let project = format!("scopecreep-test-{key}");
        let settings: Vec<(&str, &str)> = settings.iter().map(|(k, v)| (*k, v.as_str())).collect();
        b.helper_answers(project_answer(&project, &settings));
        b.wait(2 * SECOND);
        assert_eq!(b.colour_label().as_deref(), Some(want), "{key}");
        let s = b.monitor.status();
        let reading = s.project.as_ref().expect("the project is known");
        assert_eq!((reading.project.as_str(), reading.timeline.as_deref()), (project.as_str(), Some("Timeline 1")), "{key}: Sender identity");
    }
}

#[test]
fn the_manual_override_wins_over_the_project_and_is_labelled_set_manually() {
    let mut b = Bench::with_sender("manual");
    b.helper_answers(project_answer("p", &[]));
    b.wait(2 * SECOND);
    assert_eq!(b.colour_label().as_deref(), Some("Rec.709 Gamma 2.4 → Rec.709 Gamma 2.4 (from project)"));
    b.set_manually(Some("timeline=DaVinci WG/Intermediate\noutput=Rec.2100 ST2084\nlevels=full\n"));
    b.render(); // changing a parameter on the Sender node makes Resolve render the frame again
    b.wait(100 * MS);
    assert_eq!(b.colour_label().as_deref(), Some("DaVinci WG/Intermediate → Rec.2100 ST2084, full levels (set manually)"));
    b.wait(5 * SECOND);
    assert_eq!(b.colour_label().as_deref(), Some("DaVinci WG/Intermediate → Rec.2100 ST2084, full levels (set manually)"), "a fresh project reading does not override it");
    b.set_manually(None);
    b.render();
    b.wait(100 * MS);
    assert_eq!(b.colour_label().as_deref(), Some("Rec.709 Gamma 2.4 → Rec.709 Gamma 2.4 (from project)"), "turning the override off goes back to the project");
}

#[test]
fn the_manual_override_works_with_no_scripting_at_all() {
    // Resolve's free version: no external scripting. Monitor::new has no project reader.
    let path = PathBuf::from(format!("/dev/shm/scopecreep-colour-test-{}-free", std::process::id()));
    let mut writer = FeedWriter::create(&path, 4, 16 * 9 * 16).unwrap();
    let mut monitor = Monitor::new(&path);
    let now = now_ns();
    monitor.poll(now, &mut |_| {});
    writer.set_colour_setup(Some(SentColourSetup::new(Provenance::Manual, "timeline=ACEScct\noutput=Rec.709 BT.1886\nlevels=video\naces_version=1.3\n")));
    let meta = FrameMeta { width: 16, height: 9, frame_time: 1.0, frame_rate: 24.0, render_ns: now, sender_instance: 1 };
    writer.write_frame(&meta, &synthetic::solid(16, 9, [0.5; 4])).unwrap();
    monitor.poll(now + 100 * MS, &mut |_| {});
    monitor.poll(now + 200 * MS, &mut |_| {}); // a lone frame is held for half a frame period first
    assert_eq!(monitor.status().colour_setup().map(|c| c.label()).as_deref(), Some("ACEScct → Rec.709 BT.1886 (set manually, ACES 1.3)"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_manual_aces_setup_with_no_version_assumes_aces_2_0() {
    let mut b = Bench::with_sender("manual-aces");
    b.set_manually(Some("timeline=ACEScct\noutput=P3-D65\nlevels=video\n"));
    b.render();
    b.wait(100 * MS);
    assert_eq!(b.colour_label().as_deref(), Some("ACEScct → P3-D65 (gamma 2.6) (set manually, assumes ACES 2.0)"));
}

#[test]
fn a_changed_project_setting_reaches_the_label_without_restarting_anything() {
    let mut b = Bench::with_sender("changed");
    let out = |o: &str| project_answer("p", &dwg(o, "None").iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>());
    b.helper_answers(out("Rec.709 Gamma 2.4"));
    b.wait(2 * SECOND);
    assert_eq!(b.colour_label().as_deref(), Some("DaVinci WG/Intermediate → Rec.709 Gamma 2.4 (from project)"));
    // The colourist changes the Output Colour Space in Project Settings while stopped.
    b.helper_answers(out("P3-D65"));
    b.wait(3 * SECOND);
    assert_eq!(b.colour_label().as_deref(), Some("DaVinci WG/Intermediate → P3-D65 (gamma 2.6) (from project)"), "no new frame, no restart");
}

#[test]
fn while_stopped_the_project_is_read_every_two_seconds_and_never_while_playing() {
    let mut b = Bench::with_sender("cadence");
    b.helper_answers(project_answer("p", &[]));
    b.wait(SECOND + 10 * MS);
    let first = b.requests();
    assert_eq!(first, 1);
    b.wait(6 * SECOND);
    let stopped = b.requests() - first;
    assert!((2..=3).contains(&stopped), "{stopped} reads in 6 s stopped");
    b.play(10.0);
    assert_eq!(b.script.lock().unwrap().requests_during_playback, 0);
}

#[test]
fn when_playback_starts_during_a_read_a_stall_in_playback_is_not_taken_for_a_stop() {
    // Found in Resolve testing: a read in flight as playback started was followed by a render gap of over a second
    // mid-playback, which looked like a stop and drew a second read during playback.
    let mut b = Bench::with_sender("collision");
    b.helper_answers(project_answer("p", &[]));
    b.wait(2 * SECOND);
    assert_eq!(b.requests(), 1);
    b.helper_hangs();
    b.wait(1500 * MS); // the next read is now in flight...
    assert_eq!(b.requests(), 2);
    b.play(1.0); // ...when playback starts
    b.helper_answers(project_answer("p", &[]));
    b.wait(10 * MS);
    b.wait(2500 * MS); // Resolve stalls mid-playback, longer than the reread interval
    b.play(2.0);
    assert_eq!(b.script.lock().unwrap().requests_during_playback, 0, "no read is started during playback");
    assert_eq!(b.requests(), 2, "no read in the stall");
    b.wait(11 * SECOND);
    assert!(b.requests() >= 3, "reads resume after a long quiet");
}

#[test]
fn a_read_that_never_answers_is_abandoned_and_the_last_reading_kept() {
    let mut b = Bench::with_sender("stuck");
    b.helper_answers(project_answer("p", &[]));
    b.wait(2 * SECOND);
    assert!(b.colour_label().is_some());
    b.helper_hangs();
    b.wait(10 * SECOND);
    assert!(b.script.lock().unwrap().abandoned >= 1, "a read over 5 s is given up");
    assert_eq!(b.colour_label().as_deref(), Some("Rec.709 Gamma 2.4 → Rec.709 Gamma 2.4 (from project)"), "the last reading still stands");
}

#[test]
fn when_resolve_cannot_be_reached_by_script_the_colour_setup_is_unknown_and_the_notice_says_how_to_fix_it() {
    let mut b = Bench::with_sender("unreachable");
    b.helper_answers(answer(&[("result", "unreachable"), ("message", "Resolve is not running, or Preferences > System > General > External scripting using is not set to Local (Resolve Studio only)")]));
    b.wait(2 * SECOND);
    assert_eq!(b.colour_label(), None);
    assert!(b.label().contains("Colour Setup unknown"), "{}", b.label());
    assert!(b.notice().contains("External scripting using"), "{}", b.notice());
    assert!(b.notice().contains("Sender node"), "the manual override is offered: {}", b.notice());
}

#[test]
fn without_a_sender_resolve_is_not_asked() {
    let mut b = Bench::new("no-sender");
    b.helper_answers(project_answer("p", &[]));
    b.wait(5 * SECOND);
    assert_eq!(b.requests(), 0, "no Sender, so nothing to label and no way to tell whether Resolve is playing");
}

#[test]
fn a_project_reading_lists_studio_and_the_product() {
    let mut b = Bench::with_sender("product");
    b.helper_answers(project_answer("p", &[]));
    b.wait(2 * SECOND);
    let s = b.monitor.status();
    let reading = s.project.as_ref().unwrap();
    assert_eq!((reading.product.as_str(), reading.studio), ("DaVinci Resolve Studio 21.2.0.6", true));
}

#[test]
fn the_waveform_measures_in_the_output_colour_space_of_the_colour_setup() {
    use scopecreep::colour_setup::ColourSetup;
    use scopecreep::waveform::MeasuredSpace;
    let space = |output: &str| {
        let setup = ColourSetup::new(None, "DaVinci WG/Intermediate".into(), output.into(), None, None, None, None);
        setup.measured_space()
    };
    assert_eq!(space("Rec.709 Gamma 2.4"), Some(MeasuredSpace::Rec709));
    assert_eq!(space("Rec.709 BT.1886"), Some(MeasuredSpace::Rec709));
    assert_eq!(space("sRGB"), Some(MeasuredSpace::Rec709));
    assert_eq!(space("P3-D65"), Some(MeasuredSpace::P3D65));
    assert_eq!(space("P3-DCI"), Some(MeasuredSpace::P3Dci));
    assert_eq!(space("Rec.2020 Gamma 2.4"), Some(MeasuredSpace::Rec2020));
    assert_eq!(space("Rec.2100 ST2084"), Some(MeasuredSpace::Rec2100Pq));
    assert_eq!(space("Rec.2100 ST2084 (1000 nit)"), Some(MeasuredSpace::Rec2100Pq));
    assert_eq!(space("Rec.2100 HLG"), Some(MeasuredSpace::Rec2100Hlg));
    assert_eq!(space("No Output Transform"), None, "unknown: the waveform keeps its labelled assumption");
}

#[test]
fn the_waveform_follows_the_project_levels_and_says_when_they_are_set_manually() {
    use scopecreep::waveform::{LumaWeights, Measurement};
    let measuring = |b: &Bench| Measurement::for_colour_setup(b.monitor.status().colour_setup(), LumaWeights::FromPrimaries);
    let scale_of = |m: Measurement| m.label().split("  ·  ").nth(1).unwrap().to_string();

    let mut b = Bench::with_sender("levels");
    assert_eq!(scale_of(measuring(&b)), "10-bit Legal 64–940 assumed", "nothing known yet: Resolve's default, video levels");
    b.helper_answers(project_answer("scopecreep-test-levels-full", &[("videoDataLevels", "Full")]));
    b.wait(2 * SECOND);
    assert_eq!(scale_of(measuring(&b)), "10-bit Full 0–1023");
    b.helper_answers(project_answer("scopecreep-test-levels-video", &[("videoDataLevels", "Video")]));
    b.wait(3 * SECOND);
    assert_eq!(scale_of(measuring(&b)), "10-bit Legal 64–940");
    b.set_manually(Some("timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\nlevels=full\n"));
    b.render();
    b.wait(100 * MS);
    assert_eq!(scale_of(measuring(&b)), "10-bit Full 0–1023 set manually");
    b.set_manually(Some("timeline=Rec.709 Gamma 2.4\noutput=Rec.709 Gamma 2.4\nlevels=video\n"));
    b.render();
    b.wait(100 * MS);
    assert_eq!(scale_of(measuring(&b)), "10-bit Legal 64–940 set manually");
}
