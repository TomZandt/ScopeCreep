//! ACES 2.0 Output Transform (forward), ported line by line from the Academy's reference CTL.
//!
//! Sources (all Apache-2.0, ACES Software Foundation):
//!  * aces-aswf/aces-core @ 069b0bc3 `lib/Lib.Academy.{OutputTransform,Tonescale,DisplayEncoding,Utilities}.ctl`
//!    (library functions; function and constant names are kept so the port can be diffed against them)
//!  * aces-aswf/aces-output @ 6d8f9071 `d65/rec709/Output.Academy.Rec709-D65_100nit_in_Rec709-D65_BT1886.ctl`
//!    (ACESuserName "Rec.709 BT.1886"), `d65/p3/Output.Academy.P3-D65_48nit_in_P3-D65_Gamma2pt6.ctl`
//!    ("P3-D65") and `d65/rec2100/Output.Academy.Rec2100-D65_1000nit_in_Rec2100-D65_ST2084.ctl`
//!    ("Rec.2100 ST2084 (1000 nit)"). Those user names are exactly the entries in Resolve's ACES ODT list.
//!
//! Differences from the CTL: f64 instead of f32 (so this is the "ideal" result, Resolve runs f32 on a GPU);
//! otherwise the algorithm, tables and search tolerances are the same.

use crate::colour::{self, AP0, Chromaticities, M3, P3_D65, REC709, REC2020, V3, invert, mmul, rgb_to_xyz, vmul, xyz_to_rgb};
use crate::transfer;

const TABLE_SIZE: usize = 360;
const ADDITIONAL_TABLE_ENTRIES: usize = 2;
const TOTAL_TABLE_SIZE: usize = TABLE_SIZE + ADDITIONAL_TABLE_ENTRIES;
const BASE_INDEX: usize = 1;
const HUE_LIMIT: f64 = 360.0;
const CUSP_CORNER_COUNT: usize = 6;
const TOTAL_CORNER_COUNT: usize = CUSP_CORNER_COUNT + 2;
const MAX_SORTED_CORNERS: usize = 2 * CUSP_CORNER_COUNT;
const REACH_CUSP_TOLERANCE: f64 = 1e-3;
const DISPLAY_CUSP_TOLERANCE: f64 = 1e-7;
const GAMMA_MINIMUM: f64 = 0.0;
const GAMMA_MAXIMUM: f64 = 5.0;
const GAMMA_SEARCH_STEP: f64 = 0.4;
const GAMMA_ACCURACY: f64 = 1e-5;

const REF_LUMINANCE: f64 = 100.0;
const L_A: f64 = 100.0;
const Y_B: f64 = 20.0;
const SURROUND: [f64; 3] = [0.9, 0.59, 0.9];
const J_SCALE: f64 = 100.0;
const CAM_NL_Y_REFERENCE: f64 = 100.0;
const CAM_NL_OFFSET: f64 = 0.2713 * CAM_NL_Y_REFERENCE;
const CAM_NL_SCALE: f64 = 4.0 * CAM_NL_Y_REFERENCE;

const CHROMA_COMPRESS: f64 = 2.4;
const CHROMA_COMPRESS_FACT: f64 = 3.3;
const CHROMA_EXPAND: f64 = 1.3;
const CHROMA_EXPAND_FACT: f64 = 0.69;
const CHROMA_EXPAND_THR: f64 = 0.5;

const SMOOTH_CUSPS: f64 = 0.12;
const SMOOTH_M: f64 = 0.27;
const CUSP_MID_BLEND: f64 = 1.3;
const FOCUS_GAIN_BLEND: f64 = 0.3;
const FOCUS_DISTANCE: f64 = 1.35;
const FOCUS_DISTANCE_SCALING: f64 = 1.75;
const COMPRESSION_THRESHOLD: f64 = 0.75;

fn model_gamma() -> f64 {
    SURROUND[1] * (1.48 + (Y_B / REF_LUMINANCE).sqrt())
}

const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn scale_m(s: f64, m: &M3) -> M3 {
    let mut o = *m;
    for r in o.iter_mut() {
        for v in r.iter_mut() {
            *v *= s;
        }
    }
    o
}

/// CTL `scale_matrix_diagonal_f33_f3`: scales only the diagonal.
fn scale_diag(a: &M3, v: V3) -> M3 {
    let mut b = *a;
    b[0][0] *= v[0];
    b[1][1] *= v[1];
    b[2][2] *= v[2];
    b
}

fn sign(x: f64) -> f64 {
    if x < 0.0 { -1.0 } else if x > 0.0 { 1.0 } else { 0.0 }
}

/// CTL `copysign`: sign(y) * |x| (so 0 for y == 0).
fn copysign(x: f64, y: f64) -> f64 {
    sign(y) * x.abs()
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

fn lerp3(a: V3, b: V3, t: f64) -> V3 {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}

fn midpoint(a: f64, b: f64) -> f64 {
    (a + b) / 2.0
}

fn wrap_to_360(hue: f64) -> f64 {
    let y = hue % 360.0;
    if y < 0.0 { y + 360.0 } else { y }
}

fn hue_position_in_uniform_table(hue: f64, table_size: usize) -> i64 {
    (wrap_to_360(hue) / HUE_LIMIT * table_size as f64) as i64
}

fn base_hue_for_position(i_lo: usize, table_size: usize) -> f64 {
    i_lo as f64 * HUE_LIMIT / table_size as f64
}

// ---- Tonescale (Lib.Academy.Tonescale.ctl) --------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct TsParams {
    pub n: f64,
    pub n_r: f64,
    pub g: f64,
    pub t_1: f64,
    pub c_t: f64,
    pub s_2: f64,
    pub u_2: f64,
    pub m_2: f64,
    pub forward_limit: f64,
    pub inverse_limit: f64,
    pub log_peak: f64,
}

pub fn init_ts_params(peak_luminance: f64) -> TsParams {
    let n = peak_luminance;
    let n_r = 100.0;
    let g = 1.15;
    let c = 0.18;
    let c_d = 10.013;
    let w_g = 0.14;
    let t_1 = 0.04;
    let r_hit_min = 128.0;
    let r_hit_max = 896.0;

    let r_hit = r_hit_min + (r_hit_max - r_hit_min) * ((n / n_r).ln() / (10000.0f64 / 100.0).ln());
    let m_0 = n / n_r;
    let m_1 = 0.5 * (m_0 + (m_0 * (m_0 + 4.0 * t_1)).sqrt());
    let u = ((r_hit / m_1) / ((r_hit / m_1) + 1.0)).powf(g);
    let m = m_1 / u;
    let w_i = (n / 100.0).ln() / 2f64.ln();
    let c_t = c_d / n_r * (1.0 + w_i * w_g);
    let g_ip = 0.5 * (c_t + (c_t * (c_t + 4.0 * t_1)).sqrt());
    let g_ipp2 = -(m_1 * (g_ip / m).powf(1.0 / g)) / ((g_ip / m).powf(1.0 / g) - 1.0);
    let w_2 = c / g_ipp2;
    let s_2 = w_2 * m_1;
    let u_2 = ((r_hit / m_1) / ((r_hit / m_1) + w_2)).powf(g);
    let m_2 = m_1 / u_2;

    TsParams {
        n,
        n_r,
        g,
        t_1,
        c_t,
        s_2,
        u_2,
        m_2,
        forward_limit: 8.0 * r_hit,
        inverse_limit: n / (u_2 * n_r),
        log_peak: (n / n_r).log10(),
    }
}

/// Scene-referred linear (ACES) to display luminance in cd/m^2.
pub fn tonescale_fwd(x: f64, p: &TsParams) -> f64 {
    let f = p.m_2 * (x.max(0.0) / (x + p.s_2)).powf(p.g);
    let h = (f * f / (f + p.t_1)).max(0.0);
    h * p.n_r
}

// ---- JMh ------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct JmhParams {
    m_rgb_to_cam16_c: M3,
    m_cam16_c_to_rgb: M3,
    m_cone_response_to_aab: M3,
    m_aab_to_cone_response: M3,
    f_l_n: f64,
    cz: f64,
    inv_cz: f64,
    a_w_j: f64,
    inv_a_w_j: f64,
}

fn post_adaptation_fwd_(rc: f64) -> f64 {
    let f_l_y = rc.powf(0.42);
    f_l_y / (CAM_NL_OFFSET + f_l_y)
}

fn post_adaptation_inv_(ra: f64) -> f64 {
    let ra_lim = ra.min(0.99);
    let f_l_y = (CAM_NL_OFFSET * ra_lim) / (1.0 - ra_lim);
    f_l_y.powf(1.0 / 0.42)
}

fn post_adaptation_fwd(v: f64) -> f64 {
    copysign(post_adaptation_fwd_(v.abs()), v)
}

fn post_adaptation_inv(v: f64) -> f64 {
    copysign(post_adaptation_inv_(v.abs()), v)
}

fn achromatic_n_to_j(a: f64, cz: f64) -> f64 {
    J_SCALE * a.powf(cz)
}

fn j_to_achromatic_n(j: f64, inv_cz: f64) -> f64 {
    (j * (1.0 / J_SCALE)).powf(inv_cz)
}

fn a_to_y(a: f64, p: &JmhParams) -> f64 {
    let ra = p.a_w_j * a;
    post_adaptation_inv_(ra) / p.f_l_n
}

fn j_to_y(j: f64, p: &JmhParams) -> f64 {
    a_to_y(j_to_achromatic_n(j.abs(), p.inv_cz), p)
}

fn y_to_j(y: f64, p: &JmhParams) -> f64 {
    let ra = post_adaptation_fwd_(y.abs() * p.f_l_n);
    let j = achromatic_n_to_j(ra * p.inv_a_w_j, p.cz);
    copysign(j, y)
}

fn rgb_to_aab(rgb: V3, p: &JmhParams) -> V3 {
    let rgb_m = vmul(rgb, &p.m_rgb_to_cam16_c);
    let rgb_a = [post_adaptation_fwd(rgb_m[0]), post_adaptation_fwd(rgb_m[1]), post_adaptation_fwd(rgb_m[2])];
    vmul(rgb_a, &p.m_cone_response_to_aab)
}

fn aab_to_jmh(aab: V3, p: &JmhParams) -> V3 {
    if aab[0] <= 0.0 {
        return [0.0, 0.0, 0.0];
    }
    let j = achromatic_n_to_j(aab[0], p.cz);
    let m = (aab[1] * aab[1] + aab[2] * aab[2]).sqrt();
    let h = wrap_to_360(aab[2].atan2(aab[1]) * 180.0 / std::f64::consts::PI);
    [j, m, h]
}

fn rgb_to_jmh(rgb: V3, p: &JmhParams) -> V3 {
    aab_to_jmh(rgb_to_aab(rgb, p), p)
}

fn jmh_to_aab(jmh: V3, p: &JmhParams) -> V3 {
    let h_rad = jmh[2] / 180.0 * std::f64::consts::PI;
    let a = j_to_achromatic_n(jmh[0], p.inv_cz);
    [a, jmh[1] * h_rad.cos(), jmh[1] * h_rad.sin()]
}

fn aab_to_rgb(aab: V3, p: &JmhParams) -> V3 {
    let rgb_a = vmul(aab, &p.m_aab_to_cone_response);
    let rgb_m = [post_adaptation_inv(rgb_a[0]), post_adaptation_inv(rgb_a[1]), post_adaptation_inv(rgb_a[2])];
    vmul(rgb_m, &p.m_cam16_c_to_rgb)
}

fn jmh_to_rgb(jmh: V3, p: &JmhParams) -> V3 {
    aab_to_rgb(jmh_to_aab(jmh, p), p)
}

pub fn init_jmh_params(prims: &Chromaticities) -> JmhParams {
    let cam16_pri = Chromaticities {
        red: [0.8336, 0.1735],
        green: [2.3854, -1.4659],
        blue: [0.087, -0.125],
        white: [0.333, 0.333],
    };
    let matrix_16 = xyz_to_rgb(&cam16_pri);
    let base_cone_response_to_aab: M3 = [
        [2.0, 1.0, 1.0 / 9.0],
        [1.0, -12.0 / 11.0, 1.0 / 9.0],
        [1.0 / 20.0, 1.0 / 11.0, -2.0 / 9.0],
    ];
    let rgb_to_xyz_m = rgb_to_xyz(prims);
    let xyz_w = vmul([REF_LUMINANCE; 3], &rgb_to_xyz_m);
    let y_w = xyz_w[1];
    let rgb_w = vmul(xyz_w, &matrix_16);

    let k = 1.0 / (5.0 * L_A + 1.0);
    let k4 = k * k * k * k;
    let f_l = 0.2 * k4 * (5.0 * L_A) + 0.1 * (1.0 - k4).powi(2) * (5.0 * L_A).powf(1.0 / 3.0);
    let f_l_n = f_l / REF_LUMINANCE;
    let cz = model_gamma();

    let d_rgb = [f_l_n * y_w / rgb_w[0], f_l_n * y_w / rgb_w[1], f_l_n * y_w / rgb_w[2]];
    let rgb_wc = [d_rgb[0] * rgb_w[0], d_rgb[1] * rgb_w[1], d_rgb[2] * rgb_w[2]];
    let rgb_aw = [post_adaptation_fwd(rgb_wc[0]), post_adaptation_fwd(rgb_wc[1]), post_adaptation_fwd(rgb_wc[2])];

    let cone_response_to_aab = mmul(&scale_m(CAM_NL_SCALE, &IDENTITY), &base_cone_response_to_aab);
    let a_w = cone_response_to_aab[0][0] * rgb_aw[0] + cone_response_to_aab[1][0] * rgb_aw[1] + cone_response_to_aab[2][0] * rgb_aw[2];
    let a_w_j = post_adaptation_fwd_(f_l);

    let m1 = mmul(&rgb_to_xyz_m, &matrix_16);
    let m2 = scale_m(REF_LUMINANCE, &IDENTITY);
    let matrix_rgb_to_cam16 = mmul(&m1, &m2);
    let matrix_rgb_to_cam16_c = mmul(&matrix_rgb_to_cam16, &scale_diag(&IDENTITY, d_rgb));

    let mut m_cone: M3 = [[0.0; 3]; 3];
    for i in 0..3 {
        m_cone[i][0] = cone_response_to_aab[i][0] / a_w;
        m_cone[i][1] = cone_response_to_aab[i][1] * 43.0 * SURROUND[2];
        m_cone[i][2] = cone_response_to_aab[i][2] * 43.0 * SURROUND[2];
    }

    JmhParams {
        m_rgb_to_cam16_c: matrix_rgb_to_cam16_c,
        m_cam16_c_to_rgb: invert(&matrix_rgb_to_cam16_c),
        m_cone_response_to_aab: m_cone,
        m_aab_to_cone_response: invert(&m_cone),
        f_l_n,
        cz,
        inv_cz: 1.0 / cz,
        a_w_j,
        inv_a_w_j: 1.0 / a_w_j,
    }
}

// ---- ODT parameters -------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct OdtParams {
    pub peak_luminance: f64,
    input_params: JmhParams,
    reach_params: JmhParams,
    limit_params: JmhParams,
    pub ts: TsParams,
    limit_j_max: f64,
    model_gamma_inv: f64,
    table_reach_m: [f64; TOTAL_TABLE_SIZE],
    sat: f64,
    sat_thr: f64,
    compr: f64,
    chroma_compress_scale: f64,
    mid_j: f64,
    focus_dist: f64,
    lower_hull_gamma_inv: f64,
    table_hues: [f64; TOTAL_TABLE_SIZE],
    table_gamut_cusps: [[f64; 3]; TOTAL_TABLE_SIZE],
    table_upper_hull_gamma: [f64; TOTAL_TABLE_SIZE],
    hue_linearity_search_range: [i64; 2],
}

struct HueDependentGamutParams {
    jm_cusp: [f64; 2],
    gamma_bottom_inv: f64,
    gamma_top_inv: f64,
    focus_j: f64,
    analytical_threshold: f64,
}

fn reach_m_from_table(h: f64, table: &[f64]) -> f64 {
    let base = hue_position_in_uniform_table(h, TABLE_SIZE);
    let t = h - base as f64;
    let i_lo = base as usize + BASE_INDEX;
    let i_hi = i_lo + 1;
    lerp(table[i_lo], table[i_hi], t)
}

fn reinhard_remap(scale: f64, nd: f64, invert: bool) -> f64 {
    if invert {
        if nd >= 1.0 { scale } else { scale * -(nd / (nd - 1.0)) }
    } else {
        scale * nd / (1.0 + nd)
    }
}

fn toe(x: f64, limit: f64, k1_in: f64, k2_in: f64, invert: bool) -> f64 {
    if x > limit {
        return x;
    }
    let k2 = k2_in.max(0.001);
    let k1 = (k1_in * k1_in + k2 * k2).sqrt();
    let k3 = (limit + k1) / (limit + k2);
    if invert {
        (x * x + k1 * x) / (k3 * (x + k2))
    } else {
        let minus_b = k3 * x - k1;
        let minus_c = k2 * k3 * x;
        0.5 * (minus_b + (minus_b * minus_b + 4.0 * minus_c).sqrt())
    }
}

fn chroma_compress_norm(h: f64, chroma_compress_scale: f64) -> f64 {
    let hr = h / 180.0 * std::f64::consts::PI;
    let a = hr.cos();
    let b = hr.sin();
    let cos_hr2 = a * a - b * b;
    let sin_hr2 = 2.0 * a * b;
    let cos_hr3 = 4.0 * a * a * a - 3.0 * a;
    let sin_hr3 = 3.0 * b - 4.0 * b * b * b;
    let m = 11.34072 * a + 16.46899 * cos_hr2 + 7.88380 * cos_hr3 + 14.66441 * b + -6.37224 * sin_hr2 + 9.19364 * sin_hr3 + 77.12896;
    m * chroma_compress_scale
}

fn chroma_compress_fwd(jmh: V3, tonemapped_j: f64, p: &OdtParams) -> V3 {
    let (j, m, h) = (jmh[0], jmh[1], jmh[2]);
    let mut m_compr = m;
    if m != 0.0 {
        let n_j = tonemapped_j / p.limit_j_max;
        let sn_j = (1.0 - n_j).max(0.0);
        let mnorm = chroma_compress_norm(h, p.chroma_compress_scale);
        let limit = n_j.powf(p.model_gamma_inv) * reach_m_from_table(h, &p.table_reach_m) / mnorm;

        let toe_limit = limit - 0.001;
        let toe_sn_j_sat = sn_j * p.sat;
        let toe_sqrt_n_j_sat_thr = (n_j * n_j + p.sat_thr).sqrt();
        let toe_n_j_compr = n_j * p.compr;

        m_compr = m * (tonemapped_j / j).powf(p.model_gamma_inv);
        m_compr /= mnorm;
        m_compr = limit - toe(limit - m_compr, toe_limit, toe_sn_j_sat, toe_sqrt_n_j_sat_thr, false);
        m_compr = toe(m_compr, limit, toe_n_j_compr, sn_j, false);
        m_compr *= mnorm;
    }
    [tonemapped_j, m_compr, h]
}

fn tonemap_and_compress_fwd(jmh: V3, p: &OdtParams) -> V3 {
    let linear = j_to_y(jmh[0], &p.input_params) / REF_LUMINANCE;
    let tonemapped_y = tonescale_fwd(linear, &p.ts);
    let j_ts = y_to_j(tonemapped_y, &p.input_params);
    chroma_compress_fwd(jmh, j_ts, p)
}

fn compute_compression_vector_slope(intersect_j: f64, focus_j: f64, limit_j_max: f64, slope_gain: f64) -> f64 {
    let direction_scalar = if intersect_j < focus_j { intersect_j } else { limit_j_max - intersect_j };
    direction_scalar * (intersect_j - focus_j) / (focus_j * slope_gain)
}

fn solve_j_intersect(j: f64, m: f64, focus_j: f64, max_j: f64, slope_gain: f64) -> f64 {
    let m_scaled = m / slope_gain;
    let a = m_scaled / focus_j;
    if j < focus_j {
        let b = 1.0 - m_scaled;
        let c = -j;
        let det = b * b - 4.0 * a * c;
        let root = det.sqrt();
        -2.0 * c / (b + root)
    } else {
        let b = -(1.0 + m_scaled + max_j * a);
        let c = max_j * m_scaled + j;
        let det = b * b - 4.0 * a * c;
        let root = det.sqrt();
        -2.0 * c / (b - root)
    }
}

fn smin_scaled(a: f64, b: f64, scale_reference: f64) -> f64 {
    let s_scaled = SMOOTH_CUSPS * scale_reference;
    let h = (s_scaled - (a - b).abs()).max(0.0) / s_scaled;
    a.min(b) - h * h * h * s_scaled * (1.0 / 6.0)
}

fn estimate_line_and_boundary_intersection_m(j_axis_intersect: f64, slope: f64, inv_gamma: f64, j_max: f64, m_max: f64, j_intersection_reference: f64) -> f64 {
    let normalised_j = j_axis_intersect / j_intersection_reference;
    let shifted_intersection = j_intersection_reference * normalised_j.powf(inv_gamma);
    shifted_intersection * m_max / (j_max - slope * m_max)
}

fn find_gamut_boundary_intersection(jm_cusp: [f64; 2], j_max: f64, gamma_top_inv: f64, gamma_bottom_inv: f64, j_intersect_source: f64, slope: f64, j_intersect_cusp: f64) -> f64 {
    let m_boundary_lower = estimate_line_and_boundary_intersection_m(j_intersect_source, slope, gamma_bottom_inv, jm_cusp[0], jm_cusp[1], j_intersect_cusp);
    let f_j_intersect_cusp = j_max - j_intersect_cusp;
    let f_j_intersect_source = j_max - j_intersect_source;
    let f_jm_cusp_j = j_max - jm_cusp[0];
    let m_boundary_upper = estimate_line_and_boundary_intersection_m(f_j_intersect_source, -slope, gamma_top_inv, f_jm_cusp_j, jm_cusp[1], f_j_intersect_cusp);
    smin_scaled(m_boundary_lower, m_boundary_upper, jm_cusp[1])
}

fn get_focus_gain(j: f64, analytical_threshold: f64, limit_j_max: f64, focus_dist: f64) -> f64 {
    let mut gain = limit_j_max * focus_dist;
    if j > analytical_threshold {
        let mut gain_adjustment = ((limit_j_max - analytical_threshold) / (0.0001f64).max(limit_j_max - j)).log10();
        gain_adjustment = gain_adjustment * gain_adjustment + 1.0;
        gain *= gain_adjustment;
    }
    gain
}

fn remap_m(m: f64, gamut_boundary_m: f64, reach_boundary_m: f64, invert: bool) -> f64 {
    let boundary_ratio = gamut_boundary_m / reach_boundary_m;
    let proportion = boundary_ratio.max(COMPRESSION_THRESHOLD);
    let threshold = proportion * gamut_boundary_m;
    if m <= threshold || proportion >= 1.0 {
        return m;
    }
    let m_offset = m - threshold;
    let gamut_offset = gamut_boundary_m - threshold;
    let reach_offset = reach_boundary_m - threshold;
    let scale = reach_offset / ((reach_offset / gamut_offset) - 1.0);
    let nd = m_offset / scale;
    threshold + reinhard_remap(scale, nd, invert)
}

fn compress_gamut(jmh: V3, jx: f64, p: &OdtParams, hdp: &HueDependentGamutParams, invert: bool) -> V3 {
    let (j, m, h) = (jmh[0], jmh[1], jmh[2]);
    let slope_gain = get_focus_gain(jx, hdp.analytical_threshold, p.limit_j_max, p.focus_dist);
    let j_intersect_source = solve_j_intersect(j, m, hdp.focus_j, p.limit_j_max, slope_gain);
    let gamut_slope = compute_compression_vector_slope(j_intersect_source, hdp.focus_j, p.limit_j_max, slope_gain);
    let j_intersect_cusp = solve_j_intersect(hdp.jm_cusp[0], hdp.jm_cusp[1], hdp.focus_j, p.limit_j_max, slope_gain);
    let gamut_boundary_m = find_gamut_boundary_intersection(hdp.jm_cusp, p.limit_j_max, hdp.gamma_top_inv, hdp.gamma_bottom_inv, j_intersect_source, gamut_slope, j_intersect_cusp);
    if gamut_boundary_m <= 0.0 {
        return [j, 0.0, h];
    }
    let reach_max_m = reach_m_from_table(h, &p.table_reach_m);
    let reach_boundary_m = estimate_line_and_boundary_intersection_m(j_intersect_source, gamut_slope, p.model_gamma_inv, p.limit_j_max, reach_max_m, p.limit_j_max);
    let remapped_m = remap_m(m, gamut_boundary_m, reach_boundary_m, invert);
    [j_intersect_source + remapped_m * gamut_slope, remapped_m, h]
}

fn cusp_from_table(h: f64, table: &[[f64; 3]]) -> [f64; 2] {
    let mut low_i = 0usize;
    let mut high_i = BASE_INDEX + TABLE_SIZE;
    let mut i = hue_position_in_uniform_table(h, TABLE_SIZE) as usize + BASE_INDEX;
    while low_i + 1 < high_i {
        if h > table[i][2] {
            low_i = i;
        } else {
            high_i = i;
        }
        i = (low_i + high_i) / 2;
    }
    let lo = table[high_i - 1];
    let hi = table[high_i];
    let t = (h - lo[2]) / (hi[2] - lo[2]);
    [lerp(lo[0], hi[0], t), lerp(lo[1], hi[1], t)]
}

fn lookup_hue_interval(h: f64, hue_table: &[f64], range: [i64; 2]) -> usize {
    let i = BASE_INDEX as i64 + hue_position_in_uniform_table(h, TOTAL_TABLE_SIZE);
    let mut i_lo = (BASE_INDEX as i64).max(i + range[0]);
    let mut i_hi = ((BASE_INDEX + TABLE_SIZE) as i64).min(i + range[1]);
    let mut i = i;
    while i_lo + 1 < i_hi {
        if h > hue_table[i as usize] {
            i_lo = i;
        } else {
            i_hi = i;
        }
        i = (i_lo + i_hi) / 2;
    }
    i_hi.max(1) as usize
}

fn compute_focus_j(cusp_j: f64, mid_j: f64, limit_j_max: f64) -> f64 {
    lerp(cusp_j, mid_j, (1.0f64).min(CUSP_MID_BLEND - (cusp_j / limit_j_max)))
}

fn init_hue_dependent_gamut_params(hue: f64, p: &OdtParams) -> HueDependentGamutParams {
    let i_hi = lookup_hue_interval(hue, &p.table_hues, p.hue_linearity_search_range);
    let t = hue - p.table_hues[i_hi - 1]; // CTL `interpolation_weight`: h - h_lo
    let jm_cusp = cusp_from_table(hue, &p.table_gamut_cusps);
    HueDependentGamutParams {
        jm_cusp,
        gamma_bottom_inv: p.lower_hull_gamma_inv,
        gamma_top_inv: lerp(p.table_upper_hull_gamma[i_hi - 1], p.table_upper_hull_gamma[i_hi], t),
        focus_j: compute_focus_j(jm_cusp[0], p.mid_j, p.limit_j_max),
        analytical_threshold: lerp(jm_cusp[0], p.limit_j_max, FOCUS_GAIN_BLEND),
    }
}

fn gamut_compress_fwd(jmh: V3, p: &OdtParams) -> V3 {
    let (j, m, h) = (jmh[0], jmh[1], jmh[2]);
    if j <= 0.0 {
        return [0.0, 0.0, h];
    }
    if m < 0.0 || j > p.limit_j_max {
        return [j, 0.0, h];
    }
    let hdp = init_hue_dependent_gamut_params(h, p);
    compress_gamut(jmh, j, p, &hdp, false)
}

// ---- Table building -------------------------------------------------------------------------------------

fn any_below_zero(v: V3) -> bool {
    v[0] < 0.0 || v[1] < 0.0 || v[2] < 0.0
}

fn generate_unit_cube_cusp_corners(corner: usize) -> V3 {
    let f = |k: usize| if (corner + k) % CUSP_CORNER_COUNT < 3 { 1.0 } else { 0.0 };
    [f(1), f(5), f(3)]
}

fn build_limiting_cusp_corners_tables(params: &JmhParams, peak_luminance: f64) -> ([V3; TOTAL_CORNER_COUNT], [V3; TOTAL_CORNER_COUNT]) {
    let mut temp_rgb = [[0.0; 3]; CUSP_CORNER_COUNT];
    let mut temp_jmh = [[0.0; 3]; CUSP_CORNER_COUNT];
    let mut min_index = 0;
    for i in 0..CUSP_CORNER_COUNT {
        let c = generate_unit_cube_cusp_corners(i);
        let s = peak_luminance / REF_LUMINANCE;
        temp_rgb[i] = [c[0] * s, c[1] * s, c[2] * s];
        temp_jmh[i] = rgb_to_jmh(temp_rgb[i], params);
        if temp_jmh[i][2] < temp_jmh[min_index][2] {
            min_index = i;
        }
    }
    let mut rgb_corners = [[0.0; 3]; TOTAL_CORNER_COUNT];
    let mut jmh_corners = [[0.0; 3]; TOTAL_CORNER_COUNT];
    for i in 0..CUSP_CORNER_COUNT {
        rgb_corners[i + 1] = temp_rgb[(i + min_index) % CUSP_CORNER_COUNT];
        jmh_corners[i + 1] = temp_jmh[(i + min_index) % CUSP_CORNER_COUNT];
    }
    rgb_corners[0] = rgb_corners[CUSP_CORNER_COUNT];
    rgb_corners[CUSP_CORNER_COUNT + 1] = rgb_corners[1];
    jmh_corners[0] = jmh_corners[CUSP_CORNER_COUNT];
    jmh_corners[CUSP_CORNER_COUNT + 1] = jmh_corners[1];
    jmh_corners[0][2] -= HUE_LIMIT;
    jmh_corners[CUSP_CORNER_COUNT + 1][2] += HUE_LIMIT;
    (rgb_corners, jmh_corners)
}

fn find_reach_corners_table(params_reach: &JmhParams, limit_j_max: f64, forward_limit: f64) -> [V3; TOTAL_CORNER_COUNT] {
    let mut temp_jmh = [[0.0; 3]; CUSP_CORNER_COUNT];
    let limit_a = j_to_achromatic_n(limit_j_max, params_reach.inv_cz);
    let mut min_index = 0;
    for i in 0..CUSP_CORNER_COUNT {
        let v = generate_unit_cube_cusp_corners(i);
        let mut lower = 0.0;
        let mut upper = forward_limit;
        while (upper - lower) > REACH_CUSP_TOLERANCE {
            let test = (lower + upper) / 2.0;
            let tc = [test * v[0], test * v[1], test * v[2]];
            let a = rgb_to_aab(tc, params_reach)[0];
            if a < limit_a {
                lower = test;
            } else {
                upper = test;
            }
        }
        temp_jmh[i] = rgb_to_jmh([upper * v[0], upper * v[1], upper * v[2]], params_reach);
        if temp_jmh[i][2] < temp_jmh[min_index][2] {
            min_index = i;
        }
    }
    let mut jmh_corners = [[0.0; 3]; TOTAL_CORNER_COUNT];
    for i in 0..CUSP_CORNER_COUNT {
        jmh_corners[i + 1] = temp_jmh[(i + min_index) % CUSP_CORNER_COUNT];
    }
    jmh_corners[0] = jmh_corners[CUSP_CORNER_COUNT];
    jmh_corners[CUSP_CORNER_COUNT + 1] = jmh_corners[1];
    jmh_corners[0][2] -= HUE_LIMIT;
    jmh_corners[CUSP_CORNER_COUNT + 1][2] += HUE_LIMIT;
    jmh_corners
}

fn extract_sorted_cube_hues(reach: &[V3; TOTAL_CORNER_COUNT], limit: &[V3; TOTAL_CORNER_COUNT]) -> [f64; MAX_SORTED_CORNERS] {
    let mut sorted = [0.0; MAX_SORTED_CORNERS];
    let (mut idx, mut reach_idx, mut limit_idx) = (0, 1, 1);
    while reach_idx < CUSP_CORNER_COUNT + 1 || limit_idx < CUSP_CORNER_COUNT + 1 {
        let reach_hue = reach[reach_idx][2];
        let limit_hue = limit[limit_idx][2];
        if reach_hue == limit_hue {
            sorted[idx] = reach_hue;
            reach_idx += 1;
            limit_idx += 1;
        } else if reach_hue < limit_hue {
            sorted[idx] = reach_hue;
            reach_idx += 1;
        } else {
            sorted[idx] = limit_hue;
            limit_idx += 1;
        }
        idx += 1;
    }
    sorted
}

fn build_hue_sample_interval(samples: i64, lower: f64, upper: f64, table: &mut [f64; TOTAL_TABLE_SIZE], base: i64) {
    if samples <= 0 {
        return;
    }
    let delta = (upper - lower) / samples as f64;
    for i in 0..samples {
        table[(base + i) as usize] = lower + i as f64 * delta;
    }
}

fn round_ctl(x: f64) -> i64 {
    if x < 0.0 { (x - 0.5) as i64 } else { (x + 0.5) as i64 }
}

fn build_hue_table(sorted_hues: &[f64; MAX_SORTED_CORNERS]) -> [f64; TOTAL_TABLE_SIZE] {
    let mut hue_table = [0.0; TOTAL_TABLE_SIZE];
    let ideal_spacing = TABLE_SIZE as f64 / HUE_LIMIT;
    let mut samples_count = [0i64; 2 * CUSP_CORNER_COUNT + 2];
    let mut last_idx: i64 = 0;
    let mut min_index: i64 = if sorted_hues[0] == 0.0 { 0 } else { 1 };

    for hue_idx in 0..MAX_SORTED_CORNERS {
        let mut nominal_idx = round_ctl(sorted_hues[hue_idx] * ideal_spacing).max(min_index).min(TABLE_SIZE as i64 - 1);
        if last_idx == nominal_idx {
            if hue_idx > 1 && samples_count[hue_idx - 2] != (samples_count[hue_idx - 1] - 1) {
                samples_count[hue_idx - 1] -= 1;
            } else {
                nominal_idx += 1;
            }
        }
        samples_count[hue_idx] = nominal_idx.min(TABLE_SIZE as i64 - 1);
        min_index = nominal_idx;
        last_idx = min_index;
    }

    let mut total_samples = 0i64;
    build_hue_sample_interval(samples_count[0], 0.0, sorted_hues[0], &mut hue_table, total_samples + 1);
    total_samples += samples_count[0];
    for i in 1..MAX_SORTED_CORNERS {
        let samples = samples_count[i] - samples_count[i - 1];
        build_hue_sample_interval(samples, sorted_hues[i - 1], sorted_hues[i], &mut hue_table, total_samples + 1);
        total_samples += samples;
    }
    build_hue_sample_interval(TABLE_SIZE as i64 - total_samples, sorted_hues[MAX_SORTED_CORNERS - 1], HUE_LIMIT, &mut hue_table, total_samples + 1);

    hue_table[0] = hue_table[BASE_INDEX + TABLE_SIZE - 1] - HUE_LIMIT;
    hue_table[BASE_INDEX + TABLE_SIZE] = hue_table[BASE_INDEX] + HUE_LIMIT;
    hue_table
}

fn find_display_cusp_for_hue(hue: f64, rgb_corners: &[V3; TOTAL_CORNER_COUNT], jmh_corners: &[V3; TOTAL_CORNER_COUNT], params: &JmhParams) -> [f64; 2] {
    let mut upper_corner = 1;
    let mut found = false;
    let mut i = upper_corner;
    while i != TOTAL_CORNER_COUNT && !found {
        if jmh_corners[i][2] > hue {
            upper_corner = i;
            found = true;
        }
        i += 1;
    }
    let lower_corner = upper_corner - 1;
    if jmh_corners[lower_corner][2] == hue {
        return [jmh_corners[lower_corner][0], jmh_corners[lower_corner][1]];
    }
    let cusp_lower = rgb_corners[lower_corner];
    let cusp_upper = rgb_corners[upper_corner];
    let mut lower_t = 0.0;
    let mut upper_t = 1.0;
    while (upper_t - lower_t) > DISPLAY_CUSP_TOLERANCE {
        let sample_t = midpoint(lower_t, upper_t);
        let sample = lerp3(cusp_lower, cusp_upper, sample_t);
        let jmh = rgb_to_jmh(sample, params);
        if jmh[2] < jmh_corners[lower_corner][2] {
            upper_t = sample_t;
        } else if jmh[2] >= jmh_corners[upper_corner][2] {
            lower_t = sample_t;
        } else if jmh[2] > hue {
            upper_t = sample_t;
        } else {
            lower_t = sample_t;
        }
    }
    let sample_t = midpoint(lower_t, upper_t);
    let sample = lerp3(cusp_lower, cusp_upper, sample_t);
    let jmh = rgb_to_jmh(sample, params);
    [jmh[0], jmh[1]]
}

fn build_cusp_table(hue_table: &[f64; TOTAL_TABLE_SIZE], rgb_corners: &[V3; TOTAL_CORNER_COUNT], jmh_corners: &[V3; TOTAL_CORNER_COUNT], params: &JmhParams) -> [[f64; 3]; TOTAL_TABLE_SIZE] {
    let mut out = [[0.0; 3]; TOTAL_TABLE_SIZE];
    for i in BASE_INDEX..TOTAL_TABLE_SIZE {
        let hue = hue_table[i];
        let jm = find_display_cusp_for_hue(hue, rgb_corners, jmh_corners, params);
        out[i] = [jm[0], jm[1] * (1.0 + SMOOTH_M * SMOOTH_CUSPS), hue];
    }
    out[0] = [out[TABLE_SIZE][0], out[TABLE_SIZE][1], hue_table[0]];
    out[BASE_INDEX + TABLE_SIZE] = [out[BASE_INDEX][0], out[BASE_INDEX][1], hue_table[BASE_INDEX + TABLE_SIZE]];
    out
}

fn make_reach_m_table(params: &JmhParams, limit_j_max: f64) -> [f64; TOTAL_TABLE_SIZE] {
    let mut table = [0.0; TOTAL_TABLE_SIZE];
    for i in 0..TABLE_SIZE {
        let hue = base_hue_for_position(i, TABLE_SIZE);
        let search_range = 50.0;
        let search_maximum = 1300.0;
        let mut low = 0.0;
        let mut high = low + search_range;
        let mut outside = false;
        while !outside && high < search_maximum {
            let rgb = jmh_to_rgb([limit_j_max, high, hue], params);
            outside = any_below_zero(rgb);
            if !outside {
                low = high;
                high += search_range;
            }
        }
        while high - low > 1e-2 {
            let sample_m = (high + low) / 2.0;
            let rgb = jmh_to_rgb([limit_j_max, sample_m, hue], params);
            if any_below_zero(rgb) {
                high = sample_m;
            } else {
                low = sample_m;
            }
        }
        table[i + BASE_INDEX] = high;
    }
    table[0] = table[TABLE_SIZE];
    table[BASE_INDEX + TABLE_SIZE] = table[BASE_INDEX];
    table
}

const TEST_COUNT: usize = 5;
const TEST_POSITIONS: [f64; TEST_COUNT] = [0.01, 0.1, 0.5, 0.8, 0.99];

#[allow(clippy::too_many_arguments)]
fn evaluate_gamma_fit(
    jm_cusp: [f64; 2],
    test_jmh: &[V3; TEST_COUNT],
    j_intersect_source: &[f64; TEST_COUNT],
    slopes: &[f64; TEST_COUNT],
    j_intersect_cusp: &[f64; TEST_COUNT],
    top_gamma_inv: f64,
    peak_luminance: f64,
    limit_j_max: f64,
    lower_hull_gamma_inv: f64,
    limit_params: &JmhParams,
) -> bool {
    let luminance_limit = peak_luminance / REF_LUMINANCE;
    for k in 0..TEST_COUNT {
        let approx_limit_m = find_gamut_boundary_intersection(jm_cusp, limit_j_max, top_gamma_inv, lower_hull_gamma_inv, j_intersect_source[k], slopes[k], j_intersect_cusp[k]);
        let approx_limit_j = j_intersect_source[k] + slopes[k] * approx_limit_m;
        let rgb = jmh_to_rgb([approx_limit_j, approx_limit_m, test_jmh[k][2]], limit_params);
        let outside = rgb[0] > luminance_limit || rgb[1] > luminance_limit || rgb[2] > luminance_limit;
        if !outside {
            return false;
        }
    }
    true
}

fn make_upper_hull_gamma_table(cusps: &[[f64; 3]; TOTAL_TABLE_SIZE], p: &OdtParams) -> [f64; TOTAL_TABLE_SIZE] {
    let mut upper = [0.0; TOTAL_TABLE_SIZE];
    for i in BASE_INDEX..BASE_INDEX + TABLE_SIZE {
        let hue = cusps[i][2];
        let jm_cusp = [cusps[i][0], cusps[i][1]];

        // generate_gamma_test_data
        let analytical_threshold = lerp(jm_cusp[0], p.limit_j_max, FOCUS_GAIN_BLEND);
        let focus_j = compute_focus_j(jm_cusp[0], p.mid_j, p.limit_j_max);
        let mut test_jmh = [[0.0; 3]; TEST_COUNT];
        let mut j_intersect_source = [0.0; TEST_COUNT];
        let mut slopes = [0.0; TEST_COUNT];
        let mut j_intersect_cusp = [0.0; TEST_COUNT];
        for k in 0..TEST_COUNT {
            let test_j = lerp(jm_cusp[0], p.limit_j_max, TEST_POSITIONS[k]);
            let slope_gain = get_focus_gain(test_j, analytical_threshold, p.limit_j_max, p.focus_dist);
            let j_intersect = solve_j_intersect(test_j, jm_cusp[1], focus_j, p.limit_j_max, slope_gain);
            let slope = compute_compression_vector_slope(j_intersect, focus_j, p.limit_j_max, slope_gain);
            let j_cusp = solve_j_intersect(jm_cusp[0], jm_cusp[1], focus_j, p.limit_j_max, slope_gain);
            test_jmh[k] = [test_j, jm_cusp[1], hue];
            j_intersect_source[k] = j_intersect;
            slopes[k] = slope;
            j_intersect_cusp[k] = j_cusp;
        }

        let fit = |gamma_inv: f64| {
            evaluate_gamma_fit(jm_cusp, &test_jmh, &j_intersect_source, &slopes, &j_intersect_cusp, gamma_inv, p.peak_luminance, p.limit_j_max, p.lower_hull_gamma_inv, &p.limit_params)
        };

        let search_range = GAMMA_SEARCH_STEP;
        let mut low = GAMMA_MINIMUM;
        let mut high = low + search_range;
        let mut outside = false;
        while !outside && high < GAMMA_MAXIMUM {
            if !fit(1.0 / high) {
                low = high;
                high += search_range;
            } else {
                outside = true;
            }
        }
        while (high - low) > GAMMA_ACCURACY {
            let test_gamma = midpoint(high, low);
            if fit(1.0 / test_gamma) {
                high = test_gamma;
            } else {
                low = test_gamma;
            }
        }
        upper[i] = 1.0 / high;
    }
    upper[0] = upper[TABLE_SIZE];
    upper[TABLE_SIZE + BASE_INDEX] = upper[BASE_INDEX];
    upper
}

fn determine_hue_linearity_search_range(hue_table: &[f64]) -> [i64; 2] {
    let (lower_padding, upper_padding) = (0i64, 1i64);
    let mut range = [lower_padding, upper_padding];
    for i in BASE_INDEX..BASE_INDEX + TABLE_SIZE {
        let pos = hue_position_in_uniform_table(hue_table[i], TOTAL_TABLE_SIZE);
        let delta = i as i64 - pos;
        range[0] = range[0].min(delta + lower_padding);
        range[1] = range[1].max(delta + upper_padding);
    }
    range
}

pub fn init_odt_params(peak_luminance: f64, limiting: &Chromaticities) -> OdtParams {
    let input_params = init_jmh_params(&AP0);
    let reach_params = init_jmh_params(&colour::AP1); // REACH_PRI = AP1
    let limit_params = init_jmh_params(limiting);
    let ts = init_ts_params(peak_luminance);
    let limit_j_max = y_to_j(peak_luminance, &input_params);
    let table_reach_m = make_reach_m_table(&reach_params, limit_j_max);

    let sat = (0.2f64).max(CHROMA_EXPAND - (CHROMA_EXPAND * CHROMA_EXPAND_FACT) * ts.log_peak);
    let sat_thr = CHROMA_EXPAND_THR / peak_luminance;
    let compr = CHROMA_COMPRESS + (CHROMA_COMPRESS * CHROMA_COMPRESS_FACT) * ts.log_peak;
    let chroma_compress_scale = (0.03379 * peak_luminance).powf(0.30596) - 0.45135;
    let mid_j = y_to_j(ts.c_t * REF_LUMINANCE, &input_params);
    let focus_dist = FOCUS_DISTANCE + FOCUS_DISTANCE * FOCUS_DISTANCE_SCALING * ts.log_peak;
    let lower_hull_gamma = 1.14 + 0.07 * ts.log_peak;

    let mut p = OdtParams {
        peak_luminance,
        input_params,
        reach_params,
        limit_params,
        ts,
        limit_j_max,
        model_gamma_inv: 1.0 / model_gamma(),
        table_reach_m,
        sat,
        sat_thr,
        compr,
        chroma_compress_scale,
        mid_j,
        focus_dist,
        lower_hull_gamma_inv: 1.0 / lower_hull_gamma,
        table_hues: [0.0; TOTAL_TABLE_SIZE],
        table_gamut_cusps: [[0.0; 3]; TOTAL_TABLE_SIZE],
        table_upper_hull_gamma: [0.0; TOTAL_TABLE_SIZE],
        hue_linearity_search_range: [0, 1],
    };

    // make_uniform_hue_gamut_table
    let reach_jmh_corners = find_reach_corners_table(&p.reach_params, p.limit_j_max, p.ts.forward_limit);
    let (limiting_rgb_corners, limiting_jmh_corners) = build_limiting_cusp_corners_tables(&p.limit_params, p.peak_luminance);
    let sorted_hues = extract_sorted_cube_hues(&reach_jmh_corners, &limiting_jmh_corners);
    let hue_table = build_hue_table(&sorted_hues);
    p.table_gamut_cusps = build_cusp_table(&hue_table, &limiting_rgb_corners, &limiting_jmh_corners, &p.limit_params);
    for i in 0..TOTAL_TABLE_SIZE {
        p.table_hues[i] = p.table_gamut_cusps[i][2];
    }
    p.table_upper_hull_gamma = make_upper_hull_gamma_table(&p.table_gamut_cusps.clone(), &p);
    p.hue_linearity_search_range = determine_hue_linearity_search_range(&p.table_hues);
    p
}

pub fn output_transform_fwd(aces: V3, p: &OdtParams, ap0_to_ap1: &M3, ap1_to_ap0: &M3) -> V3 {
    let ap1 = vmul(aces, ap0_to_ap1);
    let c = [ap1[0].clamp(0.0, p.ts.forward_limit), ap1[1].clamp(0.0, p.ts.forward_limit), ap1[2].clamp(0.0, p.ts.forward_limit)];
    let ap0_clamped = vmul(c, ap1_to_ap0);
    let jmh = rgb_to_jmh(ap0_clamped, &p.input_params);
    let tonemapped = tonemap_and_compress_fwd(jmh, p);
    let compressed = gamut_compress_fwd(tonemapped, p);
    jmh_to_rgb(compressed, &p.limit_params)
}

// ---- Display presets ------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Eotf {
    St2084,
    Gamma26,
    Bt1886Gamma24,
}

#[derive(Clone, Debug)]
pub struct AcesOutputTransform {
    pub name: &'static str,
    pub odt: OdtParams,
    pub peak_luminance: f64,
    matrix_limit_to_display: M3,
    ap0_to_ap1: M3,
    ap1_to_ap0: M3,
    eotf: Eotf,
}

impl AcesOutputTransform {
    fn new(name: &'static str, limiting: &Chromaticities, encoding: &Chromaticities, peak: f64, eotf: Eotf) -> Self {
        AcesOutputTransform {
            name,
            odt: init_odt_params(peak, limiting),
            peak_luminance: peak,
            matrix_limit_to_display: colour::mmul(&rgb_to_xyz(limiting), &xyz_to_rgb(encoding)),
            ap0_to_ap1: colour::rgb_to_rgb(&AP0, &colour::AP1),
            ap1_to_ap0: colour::rgb_to_rgb(&colour::AP1, &AP0),
            eotf,
        }
    }

    /// "Rec.709 BT.1886": 100 nit, Rec.709 limiting and encoding primaries, BT.1886 gamma 2.4.
    pub fn rec709_bt1886() -> Self {
        Self::new("ACES 2.0 Rec.709 BT.1886", &REC709, &REC709, 100.0, Eotf::Bt1886Gamma24)
    }
    /// "P3-D65": peakLuminance 100 (= 48 nit * 100/48), P3 primaries, gamma 2.6.
    pub fn p3_d65() -> Self {
        Self::new("ACES 2.0 P3-D65", &P3_D65, &P3_D65, 100.0, Eotf::Gamma26)
    }
    /// "Rec.2100 ST2084 (1000 nit)": 1000 nit, Rec.2100 primaries, PQ.
    pub fn rec2100_pq_1000() -> Self {
        Self::new("ACES 2.0 Rec.2100 ST2084 (1000 nit)", &REC2020, &REC2020, 1000.0, Eotf::St2084)
    }

    /// Display code value to display-linear light relative to 100 nit (1.0 = 100 nit).
    pub fn decode(&self, cv: f64) -> f64 {
        match self.eotf {
            Eotf::St2084 => transfer::pq_decode(cv) / REF_LUMINANCE,
            Eotf::Gamma26 => cv.max(0.0).powf(2.6),
            Eotf::Bt1886Gamma24 => cv.max(0.0).powf(2.4),
        }
    }

    /// ACES2065-1 (AP0 linear) to display code value, full range, unclamped except as the transform clamps.
    pub fn forward(&self, aces: V3) -> V3 {
        let mut rgb = output_transform_fwd(aces, &self.odt, &self.ap0_to_ap1, &self.ap1_to_ap0);
        let hi = self.peak_luminance / REF_LUMINANCE;
        for c in rgb.iter_mut() {
            *c = c.clamp(0.0, hi);
        }
        let disp = vmul(rgb, &self.matrix_limit_to_display);
        let enc = |l: f64| {
            let l = l.max(0.0);
            match self.eotf {
                Eotf::St2084 => transfer::pq_encode(REF_LUMINANCE * l),
                Eotf::Gamma26 => l.powf(1.0 / 2.6),
                Eotf::Bt1886Gamma24 => l.powf(1.0 / 2.4),
            }
        };
        [enc(disp[0]), enc(disp[1]), enc(disp[2])]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn tonescale_constants_for_100_nit() {
        // CTL: 18% grey lands on c_d = 10.013 nit for a 100 nit display, white hits peak at r_hit = 128
        let p = init_ts_params(100.0);
        assert!(close(tonescale_fwd(0.18, &p), 10.013, 0.02), "{}", tonescale_fwd(0.18, &p));
        assert!(close(tonescale_fwd(128.0, &p), 100.0, 0.5), "{}", tonescale_fwd(128.0, &p));
        assert!(close(p.forward_limit, 8.0 * 128.0, 1e-9));
        assert_eq!(tonescale_fwd(0.0, &p), 0.0);
        assert!(tonescale_fwd(0.5, &p) < tonescale_fwd(1.0, &p));
    }

    #[test]
    fn tonescale_1000_nit_uses_larger_r_hit() {
        let p = init_ts_params(1000.0);
        let r_hit = 128.0 + (896.0 - 128.0) * (10f64.ln() / 100f64.ln());
        assert!(close(p.forward_limit, 8.0 * r_hit, 1e-9));
        assert!(close(tonescale_fwd(r_hit, &p), 1000.0, 5.0), "{}", tonescale_fwd(r_hit, &p));
    }

    #[test]
    fn tables_are_well_formed_for_rec709() {
        let t = AcesOutputTransform::rec709_bt1886();
        let p = &t.odt;
        // hue tables strictly increase and wrap at +-360
        for i in 1..TOTAL_TABLE_SIZE {
            assert!(p.table_hues[i] > p.table_hues[i - 1], "hue table not monotonic at {i}");
        }
        assert!(close(p.table_hues[BASE_INDEX + TABLE_SIZE] - p.table_hues[BASE_INDEX], 360.0, 1e-9));
        // cusps are inside the limit J range and have positive colourfulness
        for i in BASE_INDEX..BASE_INDEX + TABLE_SIZE {
            assert!(p.table_gamut_cusps[i][0] > 0.0 && p.table_gamut_cusps[i][0] <= p.limit_j_max + 1e-6);
            assert!(p.table_gamut_cusps[i][1] > 0.0);
        }
        assert!(p.table_upper_hull_gamma.iter().all(|g| *g > 0.0 && g.is_finite()));
    }

    #[test]
    fn neutrals_stay_neutral_and_hit_the_published_grey_and_white() {
        let t = AcesOutputTransform::rec709_bt1886();
        let grey = t.forward([0.18, 0.18, 0.18]);
        // 18% grey -> 10.013 nit of 100 -> BT.1886 code 0.1001^(1/2.4)
        let want = (10.013f64 / 100.0).powf(1.0 / 2.4);
        assert!(close(grey[0], want, 2e-3), "{grey:?} vs {want}");
        assert!(close(grey[0], grey[1], 1e-6) && close(grey[1], grey[2], 1e-6), "{grey:?}");
        let white = t.forward([128.0, 128.0, 128.0]);
        assert!(close(white[0], 1.0, 1e-3) && close(white[2], 1.0, 1e-3), "{white:?}");
        assert_eq!(t.forward([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn output_is_monotonic_in_exposure_and_within_display_range() {
        for t in [AcesOutputTransform::rec709_bt1886(), AcesOutputTransform::p3_d65(), AcesOutputTransform::rec2100_pq_1000()] {
            let mut prev = -1.0;
            for stop in -8..=9 {
                let v = 0.18 * 2f64.powi(stop);
                let o = t.forward([v, v, v]);
                assert!(o.iter().all(|c| (0.0..=1.0 + 1e-9).contains(c)), "{} {o:?}", t.name);
                assert!(o[1] >= prev, "{} not monotonic at stop {stop}", t.name);
                prev = o[1];
            }
        }
    }

    #[test]
    fn saturated_primaries_land_inside_the_display_gamut() {
        let t = AcesOutputTransform::rec709_bt1886();
        for rgb in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.5, 0.0, 0.5]] {
            // ACEScg (AP1) primaries are outside Rec.709; gamut compression must pull them inside.
            let aces = vmul(rgb, &colour::rgb_to_rgb(&colour::AP1, &AP0));
            let o = t.forward(aces);
            assert!(o.iter().all(|c| c.is_finite() && *c >= 0.0 && *c <= 1.0), "{rgb:?} -> {o:?}");
        }
    }
}
