//! The ColorChecker frames' expected values against the research table (docs/research/colorchecker-reference-values.md),
//! to the table's printed precision. The table was computed for the research with an independent tool chain, so
//! these tests can disagree with the code.
use scopecreep_accuracy::colorchecker::{ArriBand, ColorChecker, TimelineSpace};

struct Row {
    n: usize,
    name: &'static str,
    y: f64,
    stops: f64,
    acescct: [f64; 3],
    di: [f64; 3],
    luma_pct: f64,
    green_band: bool,
    hue: f64,
}

const TABLE: [Row; 24] = [
    Row { n: 1, name: "dark skin", y: 0.0971, stops: -0.89, acescct: [0.3904, 0.3519, 0.3206], di: [0.3027, 0.2575, 0.2365], luma_pct: 27.89, green_band: false, hue: 120.7 },
    Row { n: 2, name: "light skin", y: 0.3327, stops: 0.89, acescct: [0.4886, 0.4546, 0.4321], di: [0.4253, 0.3841, 0.3680], luma_pct: 56.17, green_band: false, hue: 117.8 },
    Row { n: 3, name: "blue sky", y: 0.1810, stops: 0.01, acescct: [0.3950, 0.4158, 0.4581], di: [0.3223, 0.3500, 0.3840], luma_pct: 40.49, green_band: true, hue: 322.3 },
    Row { n: 4, name: "foliage", y: 0.1340, stops: -0.43, acescct: [0.3791, 0.3965, 0.3270], di: [0.2902, 0.3032, 0.2474], luma_pct: 34.22, green_band: false, hue: 197.6 },
    Row { n: 5, name: "blue flower", y: 0.2310, stops: 0.36, acescct: [0.4344, 0.4287, 0.4791], di: [0.3676, 0.3698, 0.4107], luma_pct: 46.65, green_band: false, hue: 359.6 },
    Row { n: 6, name: "bluish green", y: 0.4171, stops: 1.21, acescct: [0.4446, 0.4941, 0.4825], di: [0.3797, 0.4327, 0.4208], luma_pct: 62.18, green_band: false, hue: 272.5 },
    Row { n: 7, name: "orange", y: 0.3053, stops: 0.76, acescct: [0.5021, 0.4366, 0.3278], di: [0.4389, 0.3540, 0.2859], luma_pct: 50.49, green_band: false, hue: 131.7 },
    Row { n: 8, name: "purplish blue", y: 0.1128, stops: -0.67, acescct: [0.3554, 0.3669, 0.4682], di: [0.2865, 0.3158, 0.3922], luma_pct: 29.84, green_band: false, hue: 341.6 },
    Row { n: 9, name: "moderate red", y: 0.1847, stops: 0.04, acescct: [0.4743, 0.3762, 0.3802], di: [0.4055, 0.2980, 0.3083], luma_pct: 36.33, green_band: false, hue: 96.0 },
    Row { n: 10, name: "purple", y: 0.0632, stops: -1.51, acescct: [0.3544, 0.3047, 0.3849], di: [0.2677, 0.2338, 0.2934], luma_pct: 20.85, green_band: false, hue: 38.9 },
    Row { n: 11, name: "yellow green", y: 0.4295, stops: 1.25, acescct: [0.4740, 0.4943, 0.3639], di: [0.4052, 0.4197, 0.3167], luma_pct: 63.66, green_band: false, hue: 189.4 },
    Row { n: 12, name: "orange yellow", y: 0.4214, stops: 1.23, acescct: [0.5121, 0.4756, 0.3391], di: [0.4517, 0.3987, 0.3061], luma_pct: 61.76, green_band: false, hue: 149.7 },
    Row { n: 13, name: "blue", y: 0.0590, stops: -1.61, acescct: [0.2946, 0.3062, 0.4415], di: [0.2272, 0.2628, 0.3573], luma_pct: 18.66, green_band: false, hue: 342.1 },
    Row { n: 14, name: "green", y: 0.2231, stops: 0.31, acescct: [0.3870, 0.4475, 0.3532], di: [0.3012, 0.3624, 0.2818], luma_pct: 43.38, green_band: false, hue: 234.1 },
    Row { n: 15, name: "red", y: 0.1238, stops: -0.54, acescct: [0.4522, 0.3299, 0.3055], di: [0.3765, 0.2412, 0.2370], luma_pct: 25.91, green_band: false, hue: 101.6 },
    Row { n: 16, name: "yellow", y: 0.5901, stops: 1.71, acescct: [0.5267, 0.5107, 0.3511], di: [0.4703, 0.4406, 0.3291], luma_pct: 73.83, green_band: false, hue: 163.6 },
    Row { n: 17, name: "magenta", y: 0.1876, stops: 0.06, acescct: [0.4708, 0.3744, 0.4473], di: [0.4049, 0.3168, 0.3751], luma_pct: 36.99, green_band: false, hue: 68.9 },
    Row { n: 18, name: "cyan", y: 0.1850, stops: 0.04, acescct: [0.3362, 0.4286, 0.4690], di: [0.2709, 0.3641, 0.3958], luma_pct: 34.56, green_band: false, hue: 293.5 },
    Row { n: 19, name: "white 9.5 (.05 D)", y: 0.8802, stops: 2.29, acescct: [0.5442, 0.5446, 0.5405], di: [0.5000, 0.4999, 0.4964], luma_pct: 93.86, green_band: false, hue: 180.6 },
    Row { n: 20, name: "neutral 8 (.23 D)", y: 0.5900, stops: 1.71, acescct: [0.5109, 0.5116, 0.5107], di: [0.4581, 0.4586, 0.4579], luma_pct: 76.77, green_band: false, hue: 228.9 },
    Row { n: 21, name: "neutral 6.5 (.44 D)", y: 0.3649, stops: 1.02, acescct: [0.4710, 0.4721, 0.4719], di: [0.4077, 0.4089, 0.4087], luma_pct: 59.92, green_band: false, hue: 272.1 },
    Row { n: 22, name: "neutral 5 (.70 D)", y: 0.1906, stops: 0.08, acescct: [0.4182, 0.4184, 0.4180], di: [0.3417, 0.3419, 0.3415], luma_pct: 42.23, green_band: false, hue: 215.4 },
    Row { n: 23, name: "neutral 3.5 (1.05 D)", y: 0.0882, stops: -1.03, acescct: [0.3538, 0.3552, 0.3561], di: [0.2639, 0.2655, 0.2662], luma_pct: 26.95, green_band: false, hue: 300.5 },
    Row { n: 24, name: "black 2 (1.5 D)", y: 0.0315, stops: -2.51, acescct: [0.2699, 0.2700, 0.2719], di: [0.1700, 0.1703, 0.1716], luma_pct: 13.29, green_band: false, hue: 346.2 },
];

fn near(what: &str, row: &Row, got: f64, want: f64, half_unit: f64) {
    assert!((got - want).abs() <= half_unit + 1e-9, "patch {} {}: {what} {got} vs table {want}", row.n, row.name);
}

#[test]
fn every_patch_matches_the_research_table_to_its_printed_precision() {
    let acescct = ColorChecker::new(TimelineSpace::AcesCct);
    let di = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    for row in &TABLE {
        let (a, d) = (&acescct.patches()[row.n - 1], &di.patches()[row.n - 1]);
        assert_eq!((a.number, a.name), (row.n, row.name));
        near("Y", row, a.y, row.y, 0.00005);
        near("stops", row, a.stops, row.stops, 0.005);
        for c in 0..3 {
            near("ACEScct", row, a.encoded[c], row.acescct[c], 0.00005);
            near("DaVinci Intermediate", row, d.encoded[c], row.di[c], 0.00005);
        }
        near("Rec.709 luma %", row, a.rec709_luma * 100.0, row.luma_pct, 0.005);
        near("hue", row, a.hue_degrees, row.hue, 0.05);
        let expected_band = if row.green_band { Some(ArriBand::Green) } else { None };
        assert_eq!(a.arri_band, expected_band, "patch {} {}: ARRI-style band", row.n, row.name);
    }
}

#[test]
fn the_ticket_examples_hold() {
    let cc = ColorChecker::new(TimelineSpace::AcesCct);
    let n5 = &cc.patches()[21];
    assert!((n5.y - 0.1906).abs() < 5e-5 && (n5.stops - 0.08).abs() < 0.005 && (n5.rec709_luma * 100.0 - 42.23).abs() < 0.005);
    assert!((cc.patches()[20].stops - 1.02).abs() < 0.005, "neutral 6.5");
    assert!((cc.grey_18().encoded[1] - 0.4136).abs() < 5e-5, "18 % grey in ACEScct");
    assert!((ColorChecker::new(TimelineSpace::DavinciIntermediate).grey_18().encoded[1] - 0.3360).abs() < 5e-5);
    // Only cyan goes out of the Rec.709 gamut: its red is negative (linear R -0.044).
    for p in cc.patches() {
        assert_eq!(p.out_of_gamut_rec709.is_empty(), p.number != 18, "patch {} {}", p.number, p.name);
    }
    let cyan = &cc.patches()[17];
    assert_eq!(cyan.out_of_gamut_rec709, vec![0]);
    assert!((cyan.rec709_linear[0] + 0.044).abs() < 0.0005);
}

#[test]
fn the_same_chart_in_every_space_is_the_same_light() {
    let cg = ColorChecker::new(TimelineSpace::AcesCg);
    let rec709 = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    for (a, b) in cg.patches().iter().zip(rec709.patches()) {
        assert_eq!(a.encoded, a.linear, "ACEScg is linear");
        assert_eq!(b.linear, b.rec709_linear, "Rec.709 light is its own linear");
        for c in 0..3 {
            assert!((b.encoded[c] - b.linear[c].max(0.0).powf(1.0 / 2.4)).abs() < 1e-12, "Rec.709 Gamma 2.4 encoding");
        }
        assert_eq!(a.y, b.y);
        assert_eq!(a.rec709_luma, b.rec709_luma);
    }
}

#[test]
fn a_frame_has_a_six_by_four_layout_and_each_patch_is_painted_with_its_encoded_value() {
    let cc = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    let frame = cc.frame(600, 400);
    assert_eq!(frame.pixels.len(), 600 * 400 * 4);
    // Patch 1 is top left, 6 is top right, 19 is bottom left: a chart reads left to right, top to bottom.
    let (r1, r6, r19, r24) = (frame.patch_rect(1), frame.patch_rect(6), frame.patch_rect(19), frame.patch_rect(24));
    assert!(r1.x1 <= r6.x0 && r1.y1 <= r19.y0 && r24.x1 <= 600 && r24.y1 <= 400);
    for n in 1..=24 {
        let r = frame.patch_rect(n);
        assert!(r.x1 - r.x0 >= 50 && r.y1 - r.y0 >= 50, "patch {n} has an area to measure");
        for (x, y) in [(r.x0, r.y0), (r.x1 - 1, r.y1 - 1), ((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2)] {
            // Rows are stored bottom first, as the Sender writes them.
            let i = (((399 - y) * 600 + x) * 4) as usize;
            let want = cc.patches()[n - 1].encoded;
            for c in 0..3 {
                assert_eq!(frame.pixels[i + c], want[c] as f32, "patch {n} pixel ({x}, {y}) channel {c}");
            }
            assert_eq!(frame.pixels[i + 3], 1.0);
        }
    }
}
