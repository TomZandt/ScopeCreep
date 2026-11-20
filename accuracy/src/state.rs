//! The test project state recorded next to the test data (`project-state.txt`, written by the harness:
//! harness/scopecreep_harness/state.py). The references must have been rendered from an ungraded project, as the
//! Stage 0 Feed dumps were, and the Feed frames captured later from the same state as the references (ticket #43).

use std::collections::BTreeMap;

pub const FILE: &str = "project-state.txt";

fn parse(text: &str) -> BTreeMap<&str, &str> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .collect()
}

/// Why the test data of one project can't be compared: no state recorded with the references, a clip graded in
/// the references, or Feed frames captured from a different state. Empty when the data is consistent.
pub fn problems(references: Option<&str>, feed: Option<&str>) -> Vec<String> {
    let Some(references) = references else {
        return vec![format!(
            "no {FILE} next to the references: re-render them (harness.py render)"
        )];
    };
    let refs = parse(references);
    let mut out: Vec<String> = refs
        .iter()
        .filter(|(k, v)| k.starts_with("clip ") && k.ends_with(" grade") && **v != "none")
        .map(|(k, v)| {
            format!(
                "the references were rendered with a grade on {}: {v} (test projects carry none)",
                &k[..k.len() - 6]
            )
        })
        .collect();
    if let Some(feed) = feed {
        let feed = parse(feed);
        for k in refs
            .keys()
            .chain(feed.keys().filter(|k| !refs.contains_key(*k)))
        {
            let (a, b) = (refs.get(k), feed.get(k));
            if a != b {
                out.push(format!(
                    "the Feed frames were captured from a different state: {k} is {} in the references, {} in the Feed capture",
                    a.unwrap_or(&"(absent)"),
                    b.unwrap_or(&"(absent)")
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::problems;

    const UNGRADED: &str = "# header\nproject = p\ncolour colorSpaceOutput = Rec.709\ntimeline grade = node 1: Sender\nclip ramp grade = none\n";
    const GRADED: &str = "# header\nproject = p\ncolour colorSpaceOutput = Rec.709\ntimeline grade = node 1: Sender\nclip ramp grade = node 1: Primary Balance\n";

    #[test]
    fn ungraded_references_and_a_matching_feed_capture_are_consistent() {
        assert!(problems(Some(UNGRADED), Some(UNGRADED)).is_empty());
        assert!(problems(Some(UNGRADED), None).is_empty());
    }

    #[test]
    fn references_without_a_recorded_state_are_a_problem() {
        assert_eq!(problems(None, Some(UNGRADED)).len(), 1);
    }

    #[test]
    fn references_rendered_from_a_graded_clip_are_a_problem() {
        let got = problems(Some(GRADED), Some(GRADED));
        assert_eq!(
            got,
            vec![
                "the references were rendered with a grade on clip ramp: node 1: Primary Balance (test projects carry none)"
            ]
        );
    }

    #[test]
    fn a_feed_capture_from_another_state_is_a_problem() {
        let feed = UNGRADED.replace("Rec.709", "P3-D65");
        let got = problems(Some(UNGRADED), Some(&feed));
        assert_eq!(
            got,
            vec![
                "the Feed frames were captured from a different state: colour colorSpaceOutput is Rec.709 in the references, P3-D65 in the Feed capture"
            ]
        );
    }
}
