//! Error statistics between our conversion of the probe's frame and Resolve's reference render.

use crate::imageio::Image;
use crate::pipeline::Pipeline;
use rayon::prelude::*;

/// One 10-bit code is 1/1023 of full range.
pub const CODES_10BIT: f64 = 1023.0;
pub const CODES_16BIT: f64 = 65535.0;
/// "Exact" tolerance on the encoded 0..1 scale: see `TOLERANCE_REASONING` in the report.
pub const TOLERANCE_FLOAT: f64 = 1.0e-4;
pub const HALF_CODE_10BIT: f64 = 0.5 / CODES_10BIT;
/// ...or, where a gamma/PQ encode turns float32 noise into big code steps (the darkest few codes), agreement to
/// this much in display-linear light, as a fraction of reference white.
pub const TOLERANCE_LINEAR: f64 = 5.0e-6;

#[derive(Clone, Debug, Default)]
pub struct Worst {
    pub x: usize,
    pub y: usize,
    pub channel: usize,
    pub input: [f32; 3],
    pub ours: [f64; 3],
    pub reference: [f32; 3],
    pub err: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub samples: u64,
    pub max_abs: f64,
    pub sum_abs: f64,
    /// Samples (channel values) whose 10-bit code, after clamping to 0..1 and rounding, differs.
    pub mismatched_codes_10bit: u64,
    /// Same for 16-bit codes (used against the TIFF reference).
    pub mismatched_codes_16bit: u64,
    /// Samples whose float error exceeds `TOLERANCE_FLOAT` / half a 10-bit code.
    pub over_tolerance: u64,
    pub over_half_code_10bit: u64,
    /// Largest error in display-linear light (fraction of reference white) over all samples.
    pub max_linear: f64,
    /// Samples failing BOTH the encoded and the linear tolerance (the ones that decide exact vs approximated).
    pub failing: u64,
    /// Largest reference value (encoded) among the samples whose error exceeds `TOLERANCE_FLOAT`: shows where they sit.
    pub max_ref_over_tolerance: f64,
    /// Largest light-domain error among those same samples.
    pub max_linear_over_tolerance: f64,
    pub worst: Worst,
}

impl Stats {
    pub fn mean_abs(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.sum_abs / self.samples as f64
        }
    }
    pub fn max_codes_10bit(&self) -> f64 {
        self.max_abs * CODES_10BIT
    }
    pub fn mean_codes_10bit(&self) -> f64 {
        self.mean_abs() * CODES_10BIT
    }
    pub fn merge(&mut self, o: &Stats) {
        self.samples += o.samples;
        self.sum_abs += o.sum_abs;
        self.mismatched_codes_10bit += o.mismatched_codes_10bit;
        self.mismatched_codes_16bit += o.mismatched_codes_16bit;
        self.over_tolerance += o.over_tolerance;
        self.over_half_code_10bit += o.over_half_code_10bit;
        self.failing += o.failing;
        self.max_ref_over_tolerance = self.max_ref_over_tolerance.max(o.max_ref_over_tolerance);
        self.max_linear_over_tolerance = self
            .max_linear_over_tolerance
            .max(o.max_linear_over_tolerance);
        self.max_linear = self.max_linear.max(o.max_linear);
        if o.max_abs > self.max_abs || o.max_abs.is_nan() {
            self.max_abs = o.max_abs;
            self.worst = o.worst.clone();
        }
    }
}

pub fn code(v: f64, scale: f64) -> i64 {
    (v.clamp(0.0, 1.0) * scale).round() as i64
}

/// Our conversion of `input` against Resolve's float reference. All three images must have the same size.
pub fn compare_float(pipeline: &Pipeline, input: &Image, reference: &Image) -> Stats {
    compare_float_with(pipeline, |v| pipeline.apply(v), input, reference)
}

/// A frame the app already converted (the real Feed after Output Conversion) against Resolve's float reference.
/// `pipeline` is the Colour Setup's published maths, used only to judge errors in display light.
pub fn compare_converted(pipeline: &Pipeline, converted: &Image, reference: &Image) -> Stats {
    compare_float_with(pipeline, |v| v, converted, reference)
}

fn compare_float_with(
    pipeline: &Pipeline,
    apply: impl Fn([f64; 3]) -> [f64; 3] + Sync,
    input: &Image,
    reference: &Image,
) -> Stats {
    assert_eq!(
        (input.width, input.height),
        (reference.width, reference.height),
        "size mismatch"
    );
    let w = input.width;
    (0..input.height)
        .into_par_iter()
        .map(|y| {
            let mut s = Stats::default();
            for x in 0..w {
                let i = input.rgb[y * w + x];
                let r = reference.rgb[y * w + x];
                let o = apply([i[0] as f64, i[1] as f64, i[2] as f64]);
                for c in 0..3 {
                    let err = if o[c].is_nan() || r[c].is_nan() {
                        f64::INFINITY
                    } else {
                        (o[c] - r[c] as f64).abs()
                    };
                    s.samples += 1;
                    s.sum_abs += if err.is_finite() { err } else { 0.0 };
                    if code(o[c], CODES_10BIT) != code(r[c] as f64, CODES_10BIT) {
                        s.mismatched_codes_10bit += 1;
                    }
                    if !(err <= TOLERANCE_FLOAT) {
                        s.over_tolerance += 1;
                        s.max_ref_over_tolerance = s.max_ref_over_tolerance.max(r[c] as f64);
                        s.max_linear_over_tolerance = s
                            .max_linear_over_tolerance
                            .max((pipeline.decode(o[c]) - pipeline.decode(r[c] as f64)).abs());
                    }
                    if !(err <= HALF_CODE_10BIT) {
                        s.over_half_code_10bit += 1;
                    }
                    let lerr = (pipeline.decode(o[c]) - pipeline.decode(r[c] as f64)).abs();
                    if lerr > s.max_linear {
                        s.max_linear = lerr;
                    }
                    if !(err <= TOLERANCE_FLOAT || lerr <= TOLERANCE_LINEAR) {
                        s.failing += 1;
                    }
                    if err > s.max_abs {
                        s.max_abs = err;
                        s.worst = Worst {
                            x,
                            y,
                            channel: c,
                            input: i,
                            ours: o,
                            reference: r,
                            err,
                        };
                    }
                }
            }
            s
        })
        .reduce(Stats::default, |mut a, b| {
            a.merge(&b);
            a
        })
}

/// Our conversion, clamped and quantised to 16 bits, against Resolve's 16-bit TIFF.
/// `max_abs` here is in 16-bit codes / 65535, so it is directly comparable with the float errors.
pub fn compare_tiff16(pipeline: &Pipeline, input: &Image, reference: &Image) -> Stats {
    assert_eq!(
        (input.width, input.height),
        (reference.width, reference.height),
        "size mismatch"
    );
    let w = input.width;
    (0..input.height)
        .into_par_iter()
        .map(|y| {
            let mut s = Stats::default();
            for x in 0..w {
                let i = input.rgb[y * w + x];
                let r = reference.rgb[y * w + x];
                let o = pipeline.apply([i[0] as f64, i[1] as f64, i[2] as f64]);
                for c in 0..3 {
                    let ours16 = code(o[c], CODES_16BIT);
                    let ref16 = (r[c] as f64 * CODES_16BIT).round() as i64;
                    let err = (ours16 - ref16).abs() as f64 / CODES_16BIT;
                    s.samples += 1;
                    s.sum_abs += err;
                    if ours16 != ref16 {
                        s.mismatched_codes_16bit += 1;
                    }
                    if code(o[c], CODES_10BIT) != code(r[c] as f64, CODES_10BIT) {
                        s.mismatched_codes_10bit += 1;
                    }
                    if err > s.max_abs {
                        s.max_abs = err;
                        s.worst = Worst {
                            x,
                            y,
                            channel: c,
                            input: i,
                            ours: o,
                            reference: r,
                            err,
                        };
                    }
                }
            }
            s
        })
        .reduce(Stats::default, |mut a, b| {
            a.merge(&b);
            a
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(px: Vec<[f32; 3]>) -> Image {
        Image {
            width: px.len(),
            height: 1,
            rgb: px,
        }
    }

    #[test]
    fn identical_images_have_zero_error() {
        let a = img(vec![[0.0, 0.5, 1.0], [-0.2, 1.7, 0.3]]);
        let s = compare_float(&Pipeline::Identity, &a, &a);
        assert_eq!(s.max_abs, 0.0);
        assert_eq!(s.mean_abs(), 0.0);
        assert_eq!(s.mismatched_codes_10bit, 0);
        assert_eq!(s.samples, 6);
    }

    #[test]
    fn reports_max_mean_and_worst_pixel() {
        let a = img(vec![[0.0, 0.0, 0.0], [0.5, 0.5, 0.5]]);
        let b = img(vec![[0.0, 0.0, 0.0], [0.5, 0.5, 0.6]]);
        let s = compare_float(&Pipeline::Identity, &a, &b);
        assert!((s.max_abs - 0.1).abs() < 1e-6);
        assert!((s.mean_abs() - 0.1 / 6.0).abs() < 1e-6);
        assert_eq!((s.worst.x, s.worst.channel), (1, 2));
        assert!((s.max_codes_10bit() - 102.3).abs() < 1e-3);
        assert_eq!(s.mismatched_codes_10bit, 1);
    }

    #[test]
    fn a_converted_frame_is_compared_as_it_is_and_judged_in_the_pipelines_light() {
        let ours = img(vec![[0.5, 0.5, 0.5]]);
        let reference = img(vec![[0.5, 0.5, 0.5002]]);
        let s = compare_converted(&Pipeline::dwg_rec709(), &ours, &reference);
        assert!((s.max_abs - 2e-4).abs() < 1e-6, "{}", s.max_abs);
        assert_eq!(s.over_tolerance, 1);
        // 2e-4 at code 0.5 under gamma 2.4 is about 1.8e-4 of white in light: outside both tolerances.
        assert_eq!(s.failing, 1);
    }

    #[test]
    fn ten_bit_codes_clamp_and_round() {
        assert_eq!(code(-1.0, CODES_10BIT), 0);
        assert_eq!(code(2.0, CODES_10BIT), 1023);
        assert_eq!(code(0.5, CODES_10BIT), 512); // 511.5 rounds away from zero
        assert_eq!(code(64.0 / 1023.0, CODES_10BIT), 64);
    }

    #[test]
    fn tiff_compare_counts_one_code_off_as_one_16bit_code() {
        let a = img(vec![[0.5, 0.5, 0.5]]);
        let r = img(vec![[(32768.0 + 1.0) / 65535.0, 0.5f32, 0.5]]);
        // Identity gives 0.5 -> 32768 (rounded from 32767.5); reference 32769 -> one code of 1/65535 away...
        let s = compare_tiff16(&Pipeline::Identity, &a, &r);
        assert!((s.max_abs - 1.0 / 65535.0).abs() < 1e-9, "{}", s.max_abs);
        assert_eq!(s.mismatched_codes_16bit, 1);
        assert_eq!(s.mismatched_codes_10bit, 0);
    }
}
