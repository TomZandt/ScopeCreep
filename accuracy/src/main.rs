//! `accuracy`: one command, all test projects, one table.
//!
//!   cargo run --release --manifest-path accuracy/Cargo.toml              table for every Colour Setup
//!   cargo run --release --manifest-path accuracy/Cargo.toml -- <key>     one setup, with per-clip detail
//!   cargo run --release --manifest-path accuracy/Cargo.toml -- probe <clip> <key>   pixel dump for debugging
//!
//! Inputs come from the harness (`python3 harness/harness.py all` then `clone-sender`, then
//! `dump-timeline-frames`), under $SCOPECREEP_TESTDATA (default ~/scopecreep-testdata).

use scopecreep_accuracy::compare::{self, Stats};
use scopecreep_accuracy::data::{self, CLIPS};
use scopecreep_accuracy::report::{self, Verdict};
use scopecreep_accuracy::setups::{SETUPS, Setup};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = data::testdata_root();
    match args.first().map(String::as_str) {
        Some("probe") => probe(&root, args.get(1).map(String::as_str).unwrap_or("chart"), args.get(2).map(String::as_str).unwrap_or("dwg-rec709")),
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
    Ok(Row { float, tiff, per_clip })
}

fn verdict(s: &Setup, r: &Row) -> Verdict {
    if s.untested_reason.is_some() { Verdict::Untested } else { report::classify(&r.float) }
}

fn table(root: &Path, only: Option<&str>) -> i32 {
    println!("scopecreep accuracy: published maths vs Resolve 21.2 Studio renders (data: {})\n", root.display());
    println!(
        "{:<16} {:>9} {:>9} {:>10} {:>10} {:>8} {:>9}  {}",
        "Colour Setup", "max err", "mean err", "max err", "mean err", "10-bit", "TIFF16", "verdict"
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
    println!("\nColumns: error = |ours - Resolve| per colour channel value over all pixels of 4 frames (ramp, chart, out-of-range, natural),");
    println!("float EXR reference; 10b = the same in 10-bit codes (x1023); '10-bit codes <>' = share of values whose 10-bit code after clamp and");
    println!("round differs; TIFF16 max code = largest difference to Resolve's 16-bit TIFF in 16-bit codes after our clamp and round.\n");
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

fn probe(root: &Path, clip: &str, project: &str) {
    let feed = data::read_feed(root, project, clip).unwrap();
    let rf = data::read_reference_exr(root, project, clip).unwrap();
    let setup = SETUPS.iter().find(|s| s.key == project).expect("unknown setup");
    let pl = (setup.pipeline)();
    let (w, h) = (feed.width, feed.height);
    let pts: Vec<(usize, usize)> = match clip {
        "chart" => (0..2).flat_map(|r| (0..6).map(move |c| ((c * w / 6 + (c + 1) * w / 6) / 2, (r * h / 2 + (r + 1) * h / 2) / 2))).collect(),
        "ramp" => (0..21).map(|i| (i * w / 21 + w / 42, h * 7 / 10)).chain((0..9).map(|i| (i * (w - 1) / 8, h * 9 / 10))).collect(),
        "outofrange" => vec![(w / 4, h / 4), (w / 2, h / 4), (3 * w / 4, h / 4), (w / 4, h / 2), (w / 2, h / 2), (3 * w / 4, h / 2), (w / 4, 3 * h / 4), (3 * w / 4, 3 * h / 4), (10, 10)],
        _ => vec![(100, 100), (500, 300), (1000, 800)],
    };
    for (x, y) in pts {
        let i = feed.pixel(x, y);
        let o = rf.pixel(x, y);
        let m = pl.apply([i[0] as f64, i[1] as f64, i[2] as f64]);
        println!("({x:4},{y:4}) in {:9.5} {:9.5} {:9.5} ref {:9.5} {:9.5} {:9.5} ours {:9.5} {:9.5} {:9.5}", i[0], i[1], i[2], o[0], o[1], o[2], m[0], m[1], m[2]);
    }
}
