//! Verdicts, and the text the tool prints around the table: the tolerance with its reasoning, and the sources.

use crate::compare::{Stats, TOLERANCE_FLOAT, TOLERANCE_LINEAR};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Every sample within `TOLERANCE_FLOAT` in the encoded value.
    Exact,
    /// Every sample within `TOLERANCE_FLOAT` in the encoded value or `TOLERANCE_LINEAR` in light, and some only by
    /// the second rule: float32 noise in Resolve's GPU maths, amplified by the gamma/PQ encode at display black.
    ExactToeNoise,
    /// At least one sample is outside both tolerances.
    Approximated,
    /// The case did not exercise the thing it is meant to test.
    Untested,
}

impl Verdict {
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Exact => "exact",
            Verdict::ExactToeNoise => "exact (float32 noise at black)",
            Verdict::Approximated => "approximated",
            Verdict::Untested => "UNTESTED",
        }
    }
}

pub fn classify(s: &Stats) -> Verdict {
    if s.failing > 0 {
        Verdict::Approximated
    } else if s.over_tolerance == 0 {
        Verdict::Exact
    } else {
        Verdict::ExactToeNoise
    }
}

/// What the app's label claims about a conversion (app/src/output_conversion.rs, `Plan::label`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
    Exact,
    Approximated,
    /// Shown as sent: no Output Conversion.
    None,
}

impl Claim {
    /// From the Picture View label the app showed with the frame.
    pub fn from_label(label: &str) -> Claim {
        if label.contains("Output Conversion exact") {
            Claim::Exact
        } else if label.contains("Output Conversion approximated") {
            Claim::Approximated
        } else {
            Claim::None
        }
    }
}

/// Whether a measured verdict supports what the label claims. A label may claim less than was measured, never more.
pub fn claim_holds(claim: Claim, verdict: Verdict) -> bool {
    claim != Claim::Exact || matches!(verdict, Verdict::Exact | Verdict::ExactToeNoise)
}

pub fn tolerance_text() -> String {
    format!(
        "Tolerance for \"exact\" (per colour channel value, over every pixel of the four test frames):
  float:   |ours - Resolve| <= {tf:.0e} on the encoded 0..1 scale (the scale the waveform plots), which is
           {codes:.2} of one 10-bit code ({tf:.0e} x 1023). Out-of-range values (negative, above 1.0) are held to the same
           absolute bound.
  10-bit:  so a 10-bit code can differ only where a value sits within {codes:.2} code of a rounding boundary.
  or, as a second rule, |ours - Resolve| <= {tl:.0e} in display-linear light (fraction of reference white).
Reasoning:
  * Resolve computes in float32 on the GPU. One float32 step near 1.0 is 6e-8, GPU pow/exp/log are a few ulp, and
    the longest chain here (the ACES 2.0 CAM16-style JMh maths) carries intermediates up to ~100x white, so ~6e-6 of
    white is the expected float32 noise floor there ({tl:.0e} is that floor, with a little margin). {tf:.0e} is
    ~8x the largest error seen in the simple pipelines (1.2e-5) and ~7 steps of Resolve's own 16-bit TIFF (1.5e-5 each),
    yet far below anything a real formula difference (wrong gamma, matrix or constant) would produce.
  * Bit-for-bit identity is not a testable claim: our reference maths runs in f64, Resolve's in f32.
  * The second rule exists because gamma and PQ encoding multiply float noise near display black: one 10-bit code at
    the bottom of a gamma 2.4 curve is only 6e-8 of white, below float32 resolution of the ACES maths. Samples there
    can differ by several codes with the formula still identical. These are reported separately as
    \"exact (float32 noise at black)\" and the worst code error is shown, so they are never hidden.",
        tf = TOLERANCE_FLOAT,
        tl = TOLERANCE_LINEAR,
        codes = TOLERANCE_FLOAT * 1023.0,
    )
}

pub const SOURCES: &str = "Sources (primary):
  Blackmagic Design, \"DaVinci Resolve 17 Wide Gamut Intermediate\" v1.1, Aug 2021: DaVinci Wide Gamut primaries and
    matrices (p.3), DaVinci Intermediate OETF constants and mapping table (p.4).
    documents.blackmagicdesign.com/InformationNotes/DaVinci_Resolve_17_Wide_Gamut_Intermediate.pdf
  ITU-R BT.709-6 (Rec.709 primaries, D65 white); ITU-R BT.1886-0 (display EOTF; Lb = 0 is a pure 2.4 power law);
    ITU-R BT.2020-2 and BT.2100-2 (Rec.2020/2100 primaries; PQ definition, Table 4).
  SMPTE ST 2084:2014 (PQ constants, 10000 cd/m2 peak); SMPTE ST 431-2:2011 and SMPTE EG 432-1:2010 (DCI-P3
    primaries; D65 white for P3-D65). ST 2065-1:2021 (ACES2065-1 / AP0).
  Academy S-2016-001 (ACEScct), S-2014-004 (ACEScg / AP1).
  ACES 2.0 Output Transform, Apache-2.0 reference CTL: github.com/aces-aswf/aces-core @ 069b0bc3 lib/Lib.Academy.*.ctl and
    github.com/aces-aswf/aces-output @ 6d8f9071 d65/{rec709,p3,rec2100}/Output.Academy.*.ctl (the ACESuserName of those files
    are exactly Resolve's ODT names: \"Rec.709 BT.1886\", \"P3-D65\", \"Rec.2100 ST2084 (1000 nit)\").
Observed Resolve behaviour (not published; found by comparison, recorded in src/pipeline.rs): float output unclamped;
  sign-preserving power law for gamma outputs; P3-D65 output = gamma 2.6; PQ negatives encode as 0; DWG 1.0 = 100 nit.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exact_label_needs_an_exact_measurement_and_less_may_always_be_claimed() {
        assert_eq!(Claim::from_label("Clean picture  ·  1920 × 1080  ·  … (from project)  ·  Output Conversion exact"), Claim::Exact);
        assert_eq!(Claim::from_label("…  ·  Output Conversion exact for ACES 2.0"), Claim::Exact);
        assert_eq!(Claim::from_label("…  ·  Output Conversion approximated: DaVinci tone mapping not applied (unpublished)"), Claim::Approximated);
        assert_eq!(Claim::from_label("…  ·  as sent, no Output Conversion"), Claim::None);
        assert!(claim_holds(Claim::Exact, Verdict::Exact));
        assert!(claim_holds(Claim::Exact, Verdict::ExactToeNoise));
        assert!(!claim_holds(Claim::Exact, Verdict::Approximated));
        assert!(claim_holds(Claim::Approximated, Verdict::Exact));
        assert!(claim_holds(Claim::None, Verdict::Approximated));
    }
}
