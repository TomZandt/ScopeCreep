//! Runs the real comparison when the harness data is present (skips quietly otherwise, so `cargo test` works on a
//! machine without Resolve). A setup may be exact, exact with float32 noise at black, or untested; "approximated"
//! with no recorded reason is a failure.

use scopecreep_accuracy::compare;
use scopecreep_accuracy::data::{self, CLIPS};
use scopecreep_accuracy::report::{Verdict, classify};
use scopecreep_accuracy::setups::SETUPS;

#[test]
fn every_setup_matches_resolve_or_has_a_recorded_reason() {
    let root = data::testdata_root();
    if !data::feed_path(&root, SETUPS[0].key, CLIPS[0]).exists() {
        eprintln!("skipped: no harness data under {}", root.display());
        return;
    }
    for s in SETUPS {
        let pipeline = (s.pipeline)();
        let mut stats = compare::Stats::default();
        for clip in CLIPS {
            let feed = data::read_feed(&root, s.key, clip).unwrap();
            let reference = data::read_reference_exr(&root, s.key, clip).unwrap();
            stats.merge(&compare::compare_float(&pipeline, &feed, &reference));
        }
        let v = classify(&stats);
        assert!(
            v != Verdict::Approximated || s.approximation_reason.is_some(),
            "{}: approximated with no reason (max err {:.3e}, {} failing samples)",
            s.key,
            stats.max_abs,
            stats.failing
        );
    }
}
