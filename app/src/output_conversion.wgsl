// Output Conversion, per pixel, in 32-bit float. The published constants above this line, and every matrix and
// table in `params` and `tables`, come from the accuracy crate's f64 reference (app/src/output_conversion.rs);
// this file only repeats the per-pixel steps of accuracy/src/pipeline.rs, transfer.rs and aces2.rs, with the
// same function names so it can be read side by side with them. Matrices are rows: v * M, as there.

struct Params {
    size: vec4<u32>,                    // width, height, method (0 none, 1 DWG/DI, 2 ACEScct + ACES 2.0), encoding
    encode: vec4<f32>,                  // 1 / gamma, nits per unit of linear light (PQ), display clamp (ACES), 1 (`opaque`)
    // Matrices: three rows in 32-bit float, then three rows of what 32-bit float lost of them.
    m: array<vec4<f32>, 6>,             // DWG -> display primaries; ACES: AP1 -> input CAM16 cone response
    to_aab: array<vec4<f32>, 6>,        // input cone response -> Aab
    from_aab: array<vec4<f32>, 6>,      // limiting Aab -> cone response
    to_rgb: array<vec4<f32>, 6>,        // limiting cone response -> limiting RGB
    to_display: array<vec4<f32>, 6>,    // limiting RGB -> display RGB
    tonescale: vec4<f32>,               // n_r, g, t_1, s_2
    tonescale2: vec4<f32>,              // m_2, forward_limit, f_l_n, -
    jmh: vec4<f32>,                     // cz, inv_cz, a_w_j, inv_a_w_j
    odt: vec4<f32>,                     // limit_j_max, model_gamma_inv, sat, sat_thr
    odt2: vec4<f32>,                    // compr, chroma_compress_scale, mid_j, focus_dist
    odt3: vec4<f32>,                    // lower_hull_gamma_inv, hue search range low, high, -
};
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var converted: texture_storage_2d<rgba32float, write>;
@group(0) @binding(3) var<storage, read> tables: array<f32>;

const PI: f32 = 3.14159265358979;
const N: i32 = i32(TOTAL_TABLE_SIZE);
const REACH_M: i32 = 0;
const HUES: i32 = N;
const CUSP_J: i32 = 2 * N;
const CUSP_M: i32 = 3 * N;
const UPPER_HULL_GAMMA: i32 = 4 * N;

// ---- double-float: a value carried as hi + lo, two 32-bit floats (about 46 significant bits instead of 24), for
// the few ACES 2.0 steps whose 32-bit result is not enough (the CAM16 cone responses, below).

struct Df3 {
    hi: vec3<f32>,
    lo: vec3<f32>,
};

// x, as a value the GPU's compiler cannot see through. The driver simplifies algebra: it reads (a + b) - a as b,
// which turns the rounding error two-sum recovers into 0, and it gives fma operands other than the rounded values
// the code names (measured on the RTX 3070 Ti, #44: without this every two-sum here returned 0). Multiplying by a 1
// the compiler cannot know (from the parameters) keeps each rounded step as written.
fn opaque(x: f32) -> f32 {
    return x * p.encode.w;
}

fn opaque3(x: vec3<f32>) -> vec3<f32> {
    return x * p.encode.w;
}

// a + b as hi + lo exactly (two-sum).
fn two_sum(a: f32, b: f32) -> vec2<f32> {
    let s = opaque(a + b);
    let bb = opaque(s - a);
    return vec2<f32>(s, (a - (s - bb)) + (b - bb));
}

fn two_sum3(a: vec3<f32>, b: vec3<f32>) -> Df3 {
    let s = opaque3(a + b);
    let bb = opaque3(s - a);
    return Df3(s, (a - (s - bb)) + (b - bb));
}

// a + b as hi + lo exactly, when |a| >= |b|.
fn fast_two_sum(a: f32, b: f32) -> vec2<f32> {
    let s = opaque(a + b);
    return vec2<f32>(s, b - (s - a));
}

fn df_add(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    let s = two_sum(x.x, y.x);
    return fast_two_sum(s.x, s.y + (x.y + y.y));
}

fn df_mul(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    let p = opaque(x.x * y.x);
    return fast_two_sum(p, fma(x.x, y.x, -p) + (x.x * y.y + x.y * y.x));
}

fn df_sqr(x: vec2<f32>) -> vec2<f32> {
    let p = opaque(x.x * x.x);
    return fast_two_sum(p, fma(x.x, x.x, -p) + 2.0 * x.x * x.y);
}

// x / y: the 32-bit quotient, then the quotient of what it leaves over. The quotient must be opaque: otherwise the
// compiler hands fma its unrounded x * (1 / y) and the remainder is wrong.
fn df_div(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    let q1 = opaque(x.x / y.x);
    let p = opaque(q1 * y.x);
    let r = (opaque(x.x - p) - fma(q1, y.x, -p)) + (x.y - q1 * y.y);
    return fast_two_sum(q1, r / y.x);
}

// m^21 and m^50, by squaring (the two powers the CAM16 exponent 0.42 = 21 / 50 needs).
fn df_pow21(m: f32) -> vec2<f32> {
    let m1 = vec2<f32>(m, 0.0);
    let m4 = df_sqr(df_sqr(m1));
    let m5 = df_mul(m4, m1);
    return df_mul(df_sqr(df_sqr(m5)), m1);
}

fn df_pow50(m: f32) -> vec2<f32> {
    let m1 = vec2<f32>(m, 0.0);
    let m3 = df_mul(df_sqr(m1), m1);
    let m12 = df_sqr(df_sqr(m3));
    return df_sqr(df_mul(df_sqr(m12), m1));
}

// x^(a/b) for a double-float x > 0, (a, b) being (21, 50) or (50, 21), from spow's y0 (good to about 1e-7): one
// Newton step on y^b = x^a takes it to double-float accuracy, y = y0 * (1 - r / b) with r = y0^b / x^a - 1. The
// powers are of the mantissas (so nothing overflows), 2^k carries the exponents; y0^b * 2^k and x^a agree to about
// 1e-6, so their difference is exact and r needs only 32 bits. x's low part enters r to first order.
fn df_pow_newton(x: vec2<f32>, y0: f32, a: f32, b: f32, y0_pow_b: vec2<f32>, x_pow_a: vec2<f32>, k: i32) -> vec2<f32> {
    let d = (ldexp(y0_pow_b.x, k) - x_pow_a.x) + (ldexp(y0_pow_b.y, k) - x_pow_a.y);
    let r = d / x_pow_a.x - a * x.y / x.x;
    return fast_two_sum(y0, -y0 * r / b);
}

// x^0.42, 0 at or below 0.
fn df_pow_042(x: vec2<f32>) -> vec2<f32> {
    if (x.x <= 0.0) {
        return vec2<f32>(0.0);
    }
    let y0 = spow(x.x, 0.42);
    let fy = frexp(y0);
    let fx = frexp(x.x);
    return df_pow_newton(x, y0, 21.0, 50.0, df_pow50(fy.fract), df_pow21(fx.fract), 50 * fy.exp - 21 * fx.exp);
}

// x^(1 / 0.42), 0 at or below 0.
fn df_pow_inv_042(x: vec2<f32>) -> vec2<f32> {
    if (x.x <= 0.0) {
        return vec2<f32>(0.0);
    }
    let y0 = spow(x.x, 1.0 / 0.42);
    let fy = frexp(y0);
    let fx = frexp(x.x);
    return df_pow_newton(x, y0, 50.0, 21.0, df_pow21(fy.fract), df_pow50(fx.fract), 21 * fy.exp - 50 * fx.exp);
}

// v * m, as a compensated dot product: the rounding of each product and sum is recovered (fma, two-sum) and added
// back, with what the matrix itself lost and the low part of v. The ACES 2.0 matrices sum terms in the thousands
// to results near zero, where a plain 32-bit dot product keeps only a few significant figures.
fn mul3_df(v_in: vec3<f32>, v_lo: vec3<f32>, m: array<vec4<f32>, 6>) -> Df3 {
    let v = opaque3(v_in);
    let p1 = opaque3(v.x * m[0].xyz);
    let p2 = opaque3(v.y * m[1].xyz);
    let p3 = opaque3(v.z * m[2].xyz);
    let e = fma(vec3<f32>(v.x), m[0].xyz, -p1) + fma(vec3<f32>(v.y), m[1].xyz, -p2) + fma(vec3<f32>(v.z), m[2].xyz, -p3);
    let s12 = two_sum3(p1, p2);
    let s = two_sum3(s12.hi, p3);
    let lost = v.x * m[3].xyz + v.y * m[4].xyz + v.z * m[5].xyz;
    let low = v_lo.x * m[0].xyz + v_lo.y * m[1].xyz + v_lo.z * m[2].xyz;
    return two_sum3(s.hi, e + s12.lo + s.lo + lost + low);
}

fn mul3(v: vec3<f32>, m: array<vec4<f32>, 6>) -> vec3<f32> {
    return mul3_df(v, vec3<f32>(0.0), m).hi;
}

// pow for a base that should not be negative: 0 at or below 0 (the GPU's pow is undefined there).
//
// The GPU's own pow is exp2(e * log2(x)) with both steps approximated, and the rounding of e * log2(x) alone
// costs about 4 bits when log2(x) is large. Here the exponent is kept in two parts: x = m * 2^k with k whole, so
// e * log2(x) = e * k + e * log2(m), and the rounding error of e * k is recovered exactly with fma. Only the small
// remainder goes through exp2. About twice as accurate as pow, which the ACES 2.0 maths needs (its CAM16 steps
// raise values in the hundreds to the power 0.42 and then subtract nearly equal results).
fn spow(x: f32, e: f32) -> f32 {
    if (x <= 0.0) {
        return 0.0;
    }
    let parts = frexp(x);                       // x = fract * 2^exp, fract in [0.5, 1)
    let k = f32(parts.exp);
    let hi = e * k;
    let lo = fma(e, k, -hi) + e * log2(parts.fract);
    let n = round(hi);
    return ldexp(exp2((hi - n) + lo), i32(n));
}

fn signed_pow(x: f32, e: f32) -> f32 {
    return sign(x) * spow(abs(x), e);
}

// ---- transfer.rs

fn di_decode(v: f32) -> f32 {
    if (v > DI_LOG_CUT) {
        return exp2(v / DI_C - DI_B) - DI_A;
    }
    return v / DI_M;
}

fn acescct_decode(v: f32) -> f32 {
    if (v <= CCT_Y_BRK) {
        return (v - CCT_B) / CCT_A;
    }
    // v * 17.52 - 9.72 with the constants' and the product's rounding put back: the result is an exponent of
    // about 3 to 6, where every lost bit costs 0.7 of a part per million of the linear value.
    let e = fma(v, CCT_LOG_SCALE, -CCT_LOG_OFFSET) + (v * CCT_LOG_SCALE_LO - CCT_LOG_OFFSET_LO);
    return exp2(e);
}

fn pq_encode(nits: f32) -> f32 {
    let ym = spow(max(nits / 10000.0, 0.0), PQ_M1);
    return pow((PQ_C1 + PQ_C2 * ym) / (1.0 + PQ_C3 * ym), PQ_M2);
}

// Display encoding (pipeline.rs `Encoding::encode`; aces2.rs `AcesOutputTransform::forward`).
fn encode(l: f32) -> f32 {
    switch p.size.w {
        case 0u: { return signed_pow(l, p.encode.x); }          // sign-preserving power law
        case 1u: { return pq_encode(max(l, 0.0) * p.encode.y); } // PQ: negatives encode as 0
        default: { return spow(max(l, 0.0), p.encode.x); }      // power law of the non-negative part
    }
}

// ---- aces2.rs

fn copysign(x: f32, y: f32) -> f32 {
    return sign(y) * abs(x);
}

fn wrap_to_360(hue: f32) -> f32 {
    let y = hue % 360.0;
    return select(y, y + 360.0, y < 0.0);
}

fn hue_position_in_uniform_table(hue: f32, table_size: f32) -> i32 {
    return i32(wrap_to_360(hue) / HUE_LIMIT * table_size);
}

fn post_adaptation_fwd_(rc: f32) -> f32 {
    let f_l_y = spow(rc, 0.42);
    return f_l_y / (CAM_NL_OFFSET + f_l_y);
}

fn post_adaptation_inv_(ra: f32) -> f32 {
    let ra_lim = min(ra, 0.99);
    let f_l_y = (CAM_NL_OFFSET * ra_lim) / (1.0 - ra_lim);
    return spow(f_l_y, 1.0 / 0.42);
}

// The cone responses of a colour, in and out of the model, in double-float (#44). The opponent a and b are small
// differences of cone responses thousands of times larger, so one unit in the last place of a 32-bit cone response
// moved dark saturated colours by up to half a 10-bit code in Rec.2100 PQ.
fn post_adaptation_fwd(v: vec2<f32>) -> vec2<f32> {
    let s = select(1.0, -1.0, v.x < 0.0);
    let f_l_y = df_pow_042(v * s);
    return s * df_div(f_l_y, df_add(vec2<f32>(CAM_NL_OFFSET, CAM_NL_OFFSET_LO), f_l_y));
}

fn post_adaptation_inv(v: vec2<f32>) -> vec2<f32> {
    let s = select(1.0, -1.0, v.x < 0.0);
    var ra = v * s;
    if (ra.x >= 0.99) {
        ra = vec2<f32>(0.99, 0.0);
    }
    let f_l_y = df_div(df_mul(vec2<f32>(CAM_NL_OFFSET, CAM_NL_OFFSET_LO), ra), df_add(vec2<f32>(1.0, 0.0), -ra));
    return s * df_pow_inv_042(f_l_y);
}

fn post_adaptation_fwd3(v: Df3) -> Df3 {
    let x = post_adaptation_fwd(vec2<f32>(v.hi.x, v.lo.x));
    let y = post_adaptation_fwd(vec2<f32>(v.hi.y, v.lo.y));
    let z = post_adaptation_fwd(vec2<f32>(v.hi.z, v.lo.z));
    return Df3(vec3<f32>(x.x, y.x, z.x), vec3<f32>(x.y, y.y, z.y));
}

fn post_adaptation_inv3(v: Df3) -> Df3 {
    let x = post_adaptation_inv(vec2<f32>(v.hi.x, v.lo.x));
    let y = post_adaptation_inv(vec2<f32>(v.hi.y, v.lo.y));
    let z = post_adaptation_inv(vec2<f32>(v.hi.z, v.lo.z));
    return Df3(vec3<f32>(x.x, y.x, z.x), vec3<f32>(x.y, y.y, z.y));
}

fn j_to_y(j: f32) -> f32 {
    let a = spow(abs(j) / J_SCALE, p.jmh.y);
    return post_adaptation_inv_(p.jmh.z * a) / p.tonescale2.z;
}

fn y_to_j(y: f32) -> f32 {
    let ra = post_adaptation_fwd_(abs(y) * p.tonescale2.z);
    return copysign(J_SCALE * spow(ra * p.jmh.w, p.jmh.x), y);
}

fn tonescale_fwd(x: f32) -> f32 {
    let f = p.tonescale2.x * spow(max(x, 0.0) / (x + p.tonescale.w), p.tonescale.y);
    let h = max(f * f / (f + p.tonescale.z), 0.0);
    return h * p.tonescale.x;
}

fn table(offset: i32, i: i32) -> f32 {
    return tables[offset + i];
}

fn reach_m_from_table(h: f32) -> f32 {
    let base = hue_position_in_uniform_table(h, TABLE_SIZE);
    let t = h - f32(base);
    let i_lo = base + BASE_INDEX;
    return mix(table(REACH_M, i_lo), table(REACH_M, i_lo + 1), t);
}

fn toe_fwd(x: f32, limit: f32, k1_in: f32, k2_in: f32) -> f32 {
    if (x > limit) {
        return x;
    }
    let k2 = max(k2_in, 0.001);
    let k1 = sqrt(k1_in * k1_in + k2 * k2);
    let k3 = (limit + k1) / (limit + k2);
    let minus_b = k3 * x - k1;
    let minus_c = k2 * k3 * x;
    let root = sqrt(minus_b * minus_b + 4.0 * minus_c);
    // The same root as the reference's 0.5 * (minus_b + root), written so that 32-bit float does not lose it to
    // cancellation when minus_b is negative.
    if (minus_b < 0.0) {
        return 2.0 * minus_c / (root - minus_b);
    }
    return 0.5 * (minus_b + root);
}

// `hue_dir` is (cos, sin) of the hue: see aces2_forward.
fn chroma_compress_norm(hue_dir: vec2<f32>) -> f32 {
    let a = hue_dir.x;
    let b = hue_dir.y;
    let cos_hr2 = a * a - b * b;
    let sin_hr2 = 2.0 * a * b;
    let cos_hr3 = 4.0 * a * a * a - 3.0 * a;
    let sin_hr3 = 3.0 * b - 4.0 * b * b * b;
    let m = 11.34072 * a + 16.46899 * cos_hr2 + 7.88380 * cos_hr3 + 14.66441 * b + -6.37224 * sin_hr2 + 9.19364 * sin_hr3 + 77.12896;
    return m * p.odt2.y;
}

fn chroma_compress_fwd(jmh: vec3<f32>, hue_dir: vec2<f32>, tonemapped_j: f32) -> vec3<f32> {
    let j = jmh.x;
    let m = jmh.y;
    let h = jmh.z;
    var m_compr = m;
    if (m != 0.0) {
        let n_j = tonemapped_j / p.odt.x;
        let sn_j = max(1.0 - n_j, 0.0);
        let mnorm = chroma_compress_norm(hue_dir);
        let limit = spow(n_j, p.odt.y) * reach_m_from_table(h) / mnorm;
        let toe_limit = limit - 0.001;
        let toe_sn_j_sat = sn_j * p.odt.z;
        let toe_sqrt_n_j_sat_thr = sqrt(n_j * n_j + p.odt.w);
        let toe_n_j_compr = n_j * p.odt2.x;
        m_compr = m * spow(tonemapped_j / j, p.odt.y);
        m_compr = m_compr / mnorm;
        m_compr = limit - toe_fwd(limit - m_compr, toe_limit, toe_sn_j_sat, toe_sqrt_n_j_sat_thr);
        m_compr = toe_fwd(m_compr, limit, toe_n_j_compr, sn_j);
        m_compr = m_compr * mnorm;
    }
    return vec3<f32>(tonemapped_j, m_compr, h);
}

fn tonemap_and_compress_fwd(jmh: vec3<f32>, hue_dir: vec2<f32>) -> vec3<f32> {
    let linear = j_to_y(jmh.x) / REF_LUMINANCE;
    let tonemapped_y = tonescale_fwd(linear);
    let j_ts = y_to_j(tonemapped_y);
    return chroma_compress_fwd(jmh, hue_dir, j_ts);
}

fn compute_compression_vector_slope(intersect_j: f32, focus_j: f32, limit_j_max: f32, slope_gain: f32) -> f32 {
    let direction_scalar = select(limit_j_max - intersect_j, intersect_j, intersect_j < focus_j);
    return direction_scalar * (intersect_j - focus_j) / (focus_j * slope_gain);
}

fn solve_j_intersect(j: f32, m: f32, focus_j: f32, max_j: f32, slope_gain: f32) -> f32 {
    let m_scaled = m / slope_gain;
    let a = m_scaled / focus_j;
    if (j < focus_j) {
        let b = 1.0 - m_scaled;
        let c = -j;
        let root = sqrt(b * b - 4.0 * a * c);
        return -2.0 * c / (b + root);
    }
    let b = -(1.0 + m_scaled + max_j * a);
    let c = max_j * m_scaled + j;
    let root = sqrt(b * b - 4.0 * a * c);
    return -2.0 * c / (b - root);
}

fn smin_scaled(a: f32, b: f32, scale_reference: f32) -> f32 {
    let s_scaled = SMOOTH_CUSPS * scale_reference;
    let h = max(s_scaled - abs(a - b), 0.0) / s_scaled;
    return min(a, b) - h * h * h * s_scaled * (1.0 / 6.0);
}

fn estimate_line_and_boundary_intersection_m(j_axis_intersect: f32, slope: f32, inv_gamma: f32, j_max: f32, m_max: f32, j_intersection_reference: f32) -> f32 {
    let normalised_j = j_axis_intersect / j_intersection_reference;
    let shifted_intersection = j_intersection_reference * spow(normalised_j, inv_gamma);
    return shifted_intersection * m_max / (j_max - slope * m_max);
}

fn find_gamut_boundary_intersection(jm_cusp: vec2<f32>, j_max: f32, gamma_top_inv: f32, gamma_bottom_inv: f32, j_intersect_source: f32, slope: f32, j_intersect_cusp: f32) -> f32 {
    let m_boundary_lower = estimate_line_and_boundary_intersection_m(j_intersect_source, slope, gamma_bottom_inv, jm_cusp.x, jm_cusp.y, j_intersect_cusp);
    let f_j_intersect_cusp = j_max - j_intersect_cusp;
    let f_j_intersect_source = j_max - j_intersect_source;
    let f_jm_cusp_j = j_max - jm_cusp.x;
    let m_boundary_upper = estimate_line_and_boundary_intersection_m(f_j_intersect_source, -slope, gamma_top_inv, f_jm_cusp_j, jm_cusp.y, f_j_intersect_cusp);
    return smin_scaled(m_boundary_lower, m_boundary_upper, jm_cusp.y);
}

fn log10(x: f32) -> f32 {
    return log2(x) * 0.30102999566398;
}

fn get_focus_gain(j: f32, analytical_threshold: f32, limit_j_max: f32, focus_dist: f32) -> f32 {
    var gain = limit_j_max * focus_dist;
    if (j > analytical_threshold) {
        var gain_adjustment = log10((limit_j_max - analytical_threshold) / max(0.0001, limit_j_max - j));
        gain_adjustment = gain_adjustment * gain_adjustment + 1.0;
        gain = gain * gain_adjustment;
    }
    return gain;
}

fn remap_m(m: f32, gamut_boundary_m: f32, reach_boundary_m: f32) -> f32 {
    let boundary_ratio = gamut_boundary_m / reach_boundary_m;
    let proportion = max(boundary_ratio, COMPRESSION_THRESHOLD);
    let threshold = proportion * gamut_boundary_m;
    if (m <= threshold || proportion >= 1.0) {
        return m;
    }
    let m_offset = m - threshold;
    let gamut_offset = gamut_boundary_m - threshold;
    let reach_offset = reach_boundary_m - threshold;
    let scale = reach_offset / ((reach_offset / gamut_offset) - 1.0);
    let nd = m_offset / scale;
    return threshold + scale * nd / (1.0 + nd);
}

fn cusp_from_table(h: f32) -> vec2<f32> {
    var low_i = 0;
    var high_i = BASE_INDEX + i32(TABLE_SIZE);
    var i = hue_position_in_uniform_table(h, TABLE_SIZE) + BASE_INDEX;
    while (low_i + 1 < high_i) {
        if (h > table(HUES, i)) {
            low_i = i;
        } else {
            high_i = i;
        }
        i = (low_i + high_i) / 2;
    }
    let lo = high_i - 1;
    let t = (h - table(HUES, lo)) / (table(HUES, high_i) - table(HUES, lo));
    return vec2<f32>(mix(table(CUSP_J, lo), table(CUSP_J, high_i), t), mix(table(CUSP_M, lo), table(CUSP_M, high_i), t));
}

fn lookup_hue_interval(h: f32) -> i32 {
    var i = BASE_INDEX + hue_position_in_uniform_table(h, TOTAL_TABLE_SIZE);
    var i_lo = max(BASE_INDEX, i + i32(p.odt3.y));
    var i_hi = min(BASE_INDEX + i32(TABLE_SIZE), i + i32(p.odt3.z));
    while (i_lo + 1 < i_hi) {
        if (h > table(HUES, i)) {
            i_lo = i;
        } else {
            i_hi = i;
        }
        i = (i_lo + i_hi) / 2;
    }
    return max(i_hi, 1);
}

fn compute_focus_j(cusp_j: f32, mid_j: f32, limit_j_max: f32) -> f32 {
    return mix(cusp_j, mid_j, min(1.0, CUSP_MID_BLEND - (cusp_j / limit_j_max)));
}

fn gamut_compress_fwd(jmh: vec3<f32>) -> vec3<f32> {
    let j = jmh.x;
    let m = jmh.y;
    let h = jmh.z;
    let limit_j_max = p.odt.x;
    if (j <= 0.0) {
        return vec3<f32>(0.0, 0.0, h);
    }
    if (m < 0.0 || j > limit_j_max) {
        return vec3<f32>(j, 0.0, h);
    }
    // init_hue_dependent_gamut_params
    let i_hi = lookup_hue_interval(h);
    let t = h - table(HUES, i_hi - 1);
    let jm_cusp = cusp_from_table(h);
    let gamma_bottom_inv = p.odt3.x;
    let gamma_top_inv = mix(table(UPPER_HULL_GAMMA, i_hi - 1), table(UPPER_HULL_GAMMA, i_hi), t);
    let focus_j = compute_focus_j(jm_cusp.x, p.odt2.z, limit_j_max);
    let analytical_threshold = mix(jm_cusp.x, limit_j_max, FOCUS_GAIN_BLEND);
    // compress_gamut, forward
    let slope_gain = get_focus_gain(j, analytical_threshold, limit_j_max, p.odt2.w);
    let j_intersect_source = solve_j_intersect(j, m, focus_j, limit_j_max, slope_gain);
    let gamut_slope = compute_compression_vector_slope(j_intersect_source, focus_j, limit_j_max, slope_gain);
    let j_intersect_cusp = solve_j_intersect(jm_cusp.x, jm_cusp.y, focus_j, limit_j_max, slope_gain);
    let gamut_boundary_m = find_gamut_boundary_intersection(jm_cusp, limit_j_max, gamma_top_inv, gamma_bottom_inv, j_intersect_source, gamut_slope, j_intersect_cusp);
    if (gamut_boundary_m <= 0.0) {
        return vec3<f32>(j, 0.0, h);
    }
    let reach_max_m = reach_m_from_table(h);
    let reach_boundary_m = estimate_line_and_boundary_intersection_m(j_intersect_source, gamut_slope, p.odt.y, limit_j_max, reach_max_m, limit_j_max);
    let remapped_m = remap_m(m, gamut_boundary_m, reach_boundary_m);
    return vec3<f32>(j_intersect_source + remapped_m * gamut_slope, remapped_m, h);
}

fn aces2_forward(ap1: vec3<f32>) -> vec3<f32> {
    // output_transform_fwd: clamp in AP1, then to the input JMh (AP1 -> AP0 -> CAM16 folded into p.m).
    let c = clamp(ap1, vec3<f32>(0.0), vec3<f32>(p.tonescale2.y));
    let cone_a = post_adaptation_fwd3(mul3_df(c, vec3<f32>(0.0), p.m));
    let aab = mul3_df(cone_a.hi, cone_a.lo, p.to_aab).hi;
    var jmh = vec3<f32>(0.0);
    if (aab.x > 0.0) {
        jmh = vec3<f32>(J_SCALE * spow(aab.x, p.jmh.x), sqrt(aab.y * aab.y + aab.z * aab.z), wrap_to_360(atan2(aab.z, aab.y) * 180.0 / PI));
    }
    // (cos, sin) of the hue, straight from a and b: the same numbers the reference gets from cos and sin of h, but
    // without a 32-bit round trip through degrees (h near 200 keeps only 1.5e-5 of a degree, and dark saturated
    // colours in Rec.2100 PQ move by up to a tenth of a 10-bit code per such step, #44). h itself is still used to
    // look up the tables.
    let hue_dir = select(vec2<f32>(1.0, 0.0), aab.yz / jmh.y, jmh.y > 0.0);
    let tm = tonemap_and_compress_fwd(jmh, hue_dir);
    let compressed = gamut_compress_fwd(tm);
    // jmh_to_rgb with the limiting primaries
    let a = spow(compressed.x / J_SCALE, p.jmh.y);
    let out_aab = vec3<f32>(a, compressed.y * hue_dir);
    let out_cone = post_adaptation_inv3(mul3_df(out_aab, vec3<f32>(0.0), p.from_aab));
    let rgb = clamp(mul3_df(out_cone.hi, out_cone.lo, p.to_rgb).hi, vec3<f32>(0.0), vec3<f32>(p.encode.z));
    return mul3(rgb, p.to_display);
}

@compute @workgroup_size(8, 8)
fn convert(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= p.size.x || id.y >= p.size.y) {
        return;
    }
    let texel = textureLoad(source, id.xy, 0);
    var display = texel.rgb;
    switch p.size.z {
        case 1u: {
            let lin = vec3<f32>(di_decode(texel.r), di_decode(texel.g), di_decode(texel.b));
            display = mul3(lin, p.m);
        }
        case 2u: {
            display = aces2_forward(vec3<f32>(acescct_decode(texel.r), acescct_decode(texel.g), acescct_decode(texel.b)));
        }
        default: {}
    }
    var out = texel.rgb;
    if (p.size.z != 0u) {
        out = vec3<f32>(encode(display.x), encode(display.y), encode(display.z));
    }
    textureStore(converted, id.xy, vec4<f32>(out, texel.a));
}
