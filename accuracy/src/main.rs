//! `accuracy`: one command, all test projects, one table.
//!
//! The accuracy suite (ticket #17): frames taken from the real Feed after the app's Output Conversion, against
//! Resolve's reference renders, with the label the app showed checked against the measurement:
//!
//!   cargo run --release -p scopecreep-accuracy -- feed           table for every Colour Setup
//!   cargo run --release -p scopecreep-accuracy -- feed <key>     one setup, with per-clip detail
//!
//! Its input comes from `python3 harness/harness.py feed-frames` (Resolve running, the Sender on every test project;
//! see that command). The published maths alone (Stage 0, ticket #10), the f64 reference the app's GPU conversion
//! is built from, against the same renders:
//!
//!   cargo run --release -p scopecreep-accuracy                   table for every Colour Setup
//!   cargo run --release -p scopecreep-accuracy -- <key>          one setup, with per-clip detail
//!   cargo run --release -p scopecreep-accuracy -- probe <clip> <key>   pixel dump for debugging
//!
//! Its inputs come from the harness (`python3 harness/harness.py all` then `clone-sender`, then
//! `dump-timeline-frames`). All data lives under $SCOPECREEP_TESTDATA (default ~/scopecreep-testdata).

use scopecreep_accuracy::compare::{self, Stats};
use scopecreep_accuracy::data::{self, CLIPS};
use scopecreep_accuracy::report::{self, Claim, Verdict};
use scopecreep_accuracy::setups::{SETUPS, Setup};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = data::testdata_root();
    match args.first().map(String::as_str) {
        Some("feed") => std::process::exit(feed_table(&root, args.get(1).map(String::as_str))),
        Some("probe") => probe(
            &root,
            args.get(1).map(String::as_str).unwrap_or("chart"),
            args.get(2).map(String::as_str).unwrap_or("dwg-rec709"),
        ),
        None => std::process::exit(table(&root, None)),
        Some(only) => std::process::exit(table(&root, Some(only))),
    }
}

struct Row {
    float: Stats,
    tiff: Stats,
    per_clip: Vec<(&'static str, Stats)>,
}

fn run(root: &Path, s: &Setup) -> Result<Row, String> {
    let pipeline = (s.pipeline)();
    let (mut float, mut tiff) = (Stats::default(), Stats::default());
    let mut per_clip = vec![];
    for clip in CLIPS {
        let feed = data::read_feed(root, s.key, clip)?;
        let rf = data::read_reference_exr(root, s.key, clip)?;
        let rt = data::read_reference_tiff(root, s.key, clip)?;
        let f = compare::compare_float(&pipeline, &feed, &rf);
        float.merge(&f);
        per_clip.push((clip, f));
        tiff.merge(&compare::compare_tiff16(&pipeline, &feed, &rt));
    }
    Ok(Row {
        float,
        tiff,
        per_clip,
    })
}

fn verdict(s: &Setup, r: &Row) -> Verdict {
    if s.untested_reason.is_some() {
        Verdict::Untested
    } else {
        report::classify(&r.float)
    }
}

fn table(root: &Path, only: Option<&str>) -> i32 {
    println!(
        "scopecreep accuracy: published maths vs Resolve 21.2 Studio renders (data: {})\n",
        root.display()
    );
    println!(
        "{:<16} {:>9} {:>9} {:>10} {:>10} {:>8} {:>9}  verdict",
        "Colour Setup", "max err", "mean err", "max err", "mean err", "10-bit", "TIFF16"
    );
    println!(
        "{:<16} {:>9} {:>9} {:>10} {:>10} {:>8} {:>9}",
        "", "(float)", "(float)", "(10b code)", "(10b code)", "codes <>", "max code"
    );
    let mut notes = vec![];
    let mut status = 0;
    let mut detail = vec![];
    for s in SETUPS.iter().filter(|s| only.is_none_or(|o| o == s.key)) {
        match run(root, s) {
            Err(e) => {
                println!("{:<16} ERROR: {e}", s.key);
                status = 2;
            }
            Ok(r) => {
                let v = verdict(s, &r);
                println!(
                    "{:<16} {:>9.2e} {:>9.2e} {:>10.4} {:>10.5} {:>7.3}% {:>9.1}  {}",
                    s.key,
                    r.float.max_abs,
                    r.float.mean_abs(),
                    r.float.max_codes_10bit(),
                    r.float.mean_codes_10bit(),
                    100.0 * r.float.mismatched_codes_10bit as f64 / r.float.samples as f64,
                    r.tiff.max_abs * compare::CODES_16BIT,
                    v.label()
                );
                match v {
                    Verdict::Exact => {}
                    Verdict::ExactToeNoise => notes.push(format!(
                        "{}: {} of {} samples ({:.3}%) differ by more than {:.0e} in the encoded value. All are in the darkest part of the range (reference at most {:.4} = {:.0} 10-bit codes); there the error is bounded by {:.1e} of white in light, which is float32 noise, not a formula difference. Worst {:.2} codes (10-bit). A systematic difference smaller than that bound inside this dark band cannot be ruled out.",
                        s.key, r.float.over_tolerance, r.float.samples, 100.0 * r.float.over_tolerance as f64 / r.float.samples as f64, compare::TOLERANCE_FLOAT, r.float.max_ref_over_tolerance, r.float.max_ref_over_tolerance * compare::CODES_10BIT, r.float.max_linear_over_tolerance, r.float.max_codes_10bit()
                    )),
                    Verdict::Approximated => {
                        status = status.max(1);
                        notes.push(format!(
                            "{}: {} samples outside both tolerances (max {:.2e}). {}",
                            s.key,
                            r.float.failing,
                            r.float.max_abs,
                            s.approximation_reason.unwrap_or("UNEXPLAINED: no known reason; investigate before trusting this conversion")
                        ));
                    }
                    Verdict::Untested => notes.push(format!("{}: {}", s.key, s.untested_reason.unwrap_or(""))),
                }
                if only.is_some() {
                    for (clip, c) in &r.per_clip {
                        let w = &c.worst;
                        detail.push(format!(
                            "  {clip:<10} max {:.3e} mean {:.3e} over-tol {:>8} worst ({},{}) ch{} in {:?} ours {:?} ref {:?}",
                            c.max_abs, c.mean_abs(), c.over_tolerance, w.x, w.y, w.channel, w.input, w.ours, w.reference
                        ));
                    }
                }
            }
        }
    }
    println!(
        "\nColumns: error = |ours - Resolve| per colour channel value over all pixels of 4 frames (ramp, chart, out-of-range, natural),"
    );
    println!(
        "float EXR reference; 10b = the same in 10-bit codes (x1023); '10-bit codes <>' = share of values whose 10-bit code after clamp and"
    );
    println!(
        "round differs; TIFF16 max code = largest difference to Resolve's 16-bit TIFF in 16-bit codes after our clamp and round.\n"
    );
    println!("{}\n", report::tolerance_text());
    if !notes.is_empty() {
        println!("Notes:");
        for n in &notes {
            println!("  - {n}");
        }
        println!();
    }
    if !detail.is_empty() {
        println!("Per clip:");
        for d in &detail {
            println!("{d}");
        }
        println!();
    }
    println!("{}", report::SOURCES);
    status
}

/// One Colour Setup in the accuracy suite: the app's converted frames of the four clips against Resolve's renders.
fn feed_row(root: &Path, s: &Setup) -> Result<(Stats, Vec<(&'static str, Stats)>, String), String> {
    let pipeline = (s.pipeline)();
    let mut all = Stats::default();
    let mut per_clip = vec![];
    let mut label = String::new();
    for clip in CLIPS {
        let ours = data::read_converted(root, s.key, clip)?;
        let reference = data::read_reference_exr(root, s.key, clip)?;
        let clip_label = data::read_converted_label(root, s.key, clip)?;
        if !label.is_empty() && clip_label != label {
            return Err(format!(
                "the app's label changed between clips: {label:?} then {clip_label:?}"
            ));
        }
        label = clip_label;
        let c = compare::compare_converted(&pipeline, &ours, &reference);
        all.merge(&c);
        per_clip.push((clip, c));
    }
    Ok((all, per_clip, label))
}

/// The conversion part of the app's label (after the last "  ·  ").
fn conversion_part(label: &str) -> &str {
    label.rsplit("  ·  ").next().unwrap_or(label)
}

fn feed_table(root: &Path, only: Option<&str>) -> i32 {
    println!(
        "scopecreep accuracy suite: the real Feed after the app's Output Conversion vs Resolve 21.2 Studio renders (data: {})\n",
        root.display()
    );
    println!(
        "{:<16} {:>9} {:>10} {:>9}  {:<30} {:<40} label holds",
        "Colour Setup", "max err", "max err", "over tol", "measured", "the app's label"
    );
    println!(
        "{:<16} {:>9} {:>10} {:>9}",
        "", "(float)", "(10b code)", "(share)"
    );
    let mut status = 0;
    let mut notes = vec![];
    let mut detail = vec![];
    for s in SETUPS.iter().filter(|s| only.is_none_or(|o| o == s.key)) {
        match feed_row(root, s) {
            Err(e) => {
                println!("{:<16} ERROR: {e}", s.key);
                status = 2;
            }
            Ok((st, per_clip, label)) => {
                let verdict = report::classify(&st);
                let claim = Claim::from_label(&label);
                let holds = report::claim_holds(claim, verdict);
                if !holds {
                    status = status.max(1);
                }
                println!(
                    "{:<16} {:>9.2e} {:>10.4} {:>8.4}%  {:<30} {:<40} {}",
                    s.key,
                    st.max_abs,
                    st.max_codes_10bit(),
                    100.0 * st.over_tolerance as f64 / st.samples as f64,
                    verdict.label(),
                    conversion_part(&label),
                    if holds {
                        "yes"
                    } else {
                        "NO: the label claims more than was measured"
                    }
                );
                if verdict == Verdict::ExactToeNoise {
                    notes.push(format!(
                        "{}: {} of {} samples differ by more than {:.0e} encoded, all at or below reference {:.4} ({:.0} 10-bit codes), within {:.1e} of white in light (float32 noise at black). Worst {:.2} codes.",
                        s.key, st.over_tolerance, st.samples, compare::TOLERANCE_FLOAT, st.max_ref_over_tolerance, st.max_ref_over_tolerance * compare::CODES_10BIT, st.max_linear_over_tolerance, st.max_codes_10bit()
                    ));
                }
                if verdict == Verdict::Approximated {
                    notes.push(format!(
                        "{}: {} samples outside both tolerances (max {:.2e}); the app says: {}",
                        s.key,
                        st.failing,
                        st.max_abs,
                        conversion_part(&label)
                    ));
                }
                if only.is_some() {
                    for (clip, c) in &per_clip {
                        let w = &c.worst;
                        detail.push(format!(
                            "  {clip:<10} max {:.3e} mean {:.3e} over-tol {:>8} failing {:>6} worst ({},{}) ch{} ours {:?} ref {:?}",
                            c.max_abs, c.mean_abs(), c.over_tolerance, c.failing, w.x, w.y, w.channel, w.input, w.reference
                        ));
                    }
                }
            }
        }
    }
    println!(
        "\nColumns: error = |app - Resolve| per colour channel value over all pixels of 4 frames (ramp, chart, out-of-range, natural),"
    );
    println!(
        "float EXR reference; 10b = the same in 10-bit codes (x1023); over tol = share of values more than the encoded tolerance away."
    );
    println!(
        "A label may claim less than was measured (approximated, or no Output Conversion), never more.\n"
    );
    println!("{}\n", report::tolerance_text());
    if !notes.is_empty() {
        println!("Notes:");
        for n in &notes {
            println!("  - {n}");
        }
        println!();
    }
    if !detail.is_empty() {
        println!("Per clip:");
        for d in &detail {
            println!("{d}");
        }
    }
    status
}

fn probe(root: &Path, clip: &str, project: &str) {
    let feed = data::read_feed(root, project, clip).unwrap();
    let rf = data::read_reference_exr(root, project, clip).unwrap();
    let setup = SETUPS
        .iter()
        .find(|s| s.key == project)
        .expect("unknown setup");
    let pl = (setup.pipeline)();
    let (w, h) = (feed.width, feed.height);
    let pts: Vec<(usize, usize)> = match clip {
        "chart" => (0..2)
            .flat_map(|r| {
                (0..6).map(move |c| {
                    (
                        (c * w / 6 + (c + 1) * w / 6) / 2,
                        (r * h / 2 + (r + 1) * h / 2) / 2,
                    )
                })
            })
            .collect(),
        "ramp" => (0..21)
            .map(|i| (i * w / 21 + w / 42, h * 7 / 10))
            .chain((0..9).map(|i| (i * (w - 1) / 8, h * 9 / 10)))
            .collect(),
        "outofrange" => vec![
            (w / 4, h / 4),
            (w / 2, h / 4),
            (3 * w / 4, h / 4),
            (w / 4, h / 2),
            (w / 2, h / 2),
            (3 * w / 4, h / 2),
            (w / 4, 3 * h / 4),
            (3 * w / 4, 3 * h / 4),
            (10, 10),
        ],
        _ => vec![(100, 100), (500, 300), (1000, 800)],
    };
    for (x, y) in pts {
        let i = feed.pixel(x, y);
        let o = rf.pixel(x, y);
        let m = pl.apply([i[0] as f64, i[1] as f64, i[2] as f64]);
        println!(
            "({x:4},{y:4}) in {:9.5} {:9.5} {:9.5} ref {:9.5} {:9.5} {:9.5} ours {:9.5} {:9.5} {:9.5}",
            i[0], i[1], i[2], o[0], o[1], o[2], m[0], m[1], m[2]
        );
    }
}
