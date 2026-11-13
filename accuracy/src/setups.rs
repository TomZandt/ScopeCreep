//! The Colour Setups under test: one per `scopecreep-test-<key>` Resolve project built by the harness
//! (`harness/scopecreep_harness/setups.py`).

use crate::aces2::AcesOutputTransform;
use crate::pipeline::Pipeline;

pub struct Setup {
    pub key: &'static str,
    pub title: &'static str,
    pub pipeline: fn() -> Pipeline,
    /// Why this setup may legitimately differ from the published maths. Printed when the measured error
    /// exceeds the tolerance. `None` means "we expect exact": a miss is then reported as UNEXPLAINED.
    pub approximation_reason: Option<&'static str>,
    /// Set when the case does not test what its name says; the verdict is then UNTESTED regardless of the numbers.
    pub untested_reason: Option<&'static str>,
}

pub const SETUPS: &[Setup] = &[
    Setup {
        key: "yrgb-rec709g24",
        title: "DaVinci YRGB, Rec.709 Gamma 2.4 timeline and output",
        pipeline: || Pipeline::Identity,
        approximation_reason: None,
        untested_reason: None,
    },
    Setup {
        key: "dwg-rec709",
        title: "Colour Managed, DWG/DI -> Rec.709 Gamma 2.4",
        pipeline: Pipeline::dwg_rec709,
        approximation_reason: None,
        untested_reason: None,
    },
    Setup {
        key: "dwg-p3d65",
        title: "Colour Managed, DWG/DI -> P3-D65",
        pipeline: Pipeline::dwg_p3d65,
        approximation_reason: None,
        untested_reason: None,
    },
    Setup {
        key: "dwg-pq-tmoff",
        title: "Colour Managed, DWG/DI -> Rec.2100 PQ, tone mapping off",
        pipeline: Pipeline::dwg_rec2100_pq,
        approximation_reason: None,
        untested_reason: None,
    },
    Setup {
        key: "dwg-pq-tmon",
        title: "Colour Managed, DWG/DI -> Rec.2100 PQ, DaVinci tone mapping on",
        pipeline: Pipeline::dwg_rec2100_pq,
        approximation_reason: Some("DaVinci tone mapping is unpublished, so there is no published maths to apply"),
        untested_reason: Some(
            "the setting reads back as DaVinci, but Resolve's render is identical to tone mapping off (tried DaVinci and \
             Luminance Mapping, working luminance 1000/4000, PQ and SDR output); so this proves nothing about tone mapping",
        ),
    },
    Setup {
        key: "acescct-rec709",
        title: "ACEScct (ACES 2.0), Rec.709 BT.1886 output transform",
        pipeline: || Pipeline::Aces2(Box::new(AcesOutputTransform::rec709_bt1886())),
        approximation_reason: None,
        untested_reason: None,
    },
    Setup {
        key: "acescct-p3d65",
        title: "ACEScct (ACES 2.0), P3-D65 output transform",
        pipeline: || Pipeline::Aces2(Box::new(AcesOutputTransform::p3_d65())),
        approximation_reason: None,
        untested_reason: None,
    },
    Setup {
        key: "acescct-pq",
        title: "ACEScct (ACES 2.0), Rec.2100 PQ 1000 nit output transform",
        pipeline: || Pipeline::Aces2(Box::new(AcesOutputTransform::rec2100_pq_1000())),
        approximation_reason: None,
        untested_reason: None,
    },
];
