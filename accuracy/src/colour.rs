//! Chromaticities and RGB/XYZ matrices.
//!
//! Matrix convention: the CTL one used by the ACES reference code, so ports stay line-for-line comparable.
//! A colour is a ROW vector and `vmul(v, m)` is `v * m`, so `m[i]` is the XYZ of the i-th primary.
//! (The Blackmagic whitepaper prints its matrices in the column convention, i.e. transposed.)

pub type V3 = [f64; 3];
pub type M3 = [[f64; 3]; 3];

#[derive(Clone, Copy, Debug)]
pub struct Chromaticities {
    pub red: [f64; 2],
    pub green: [f64; 2],
    pub blue: [f64; 2],
    pub white: [f64; 2],
}

const D65: [f64; 2] = [0.3127, 0.3290];

/// ITU-R BT.709-6, Table 1 (primaries) with the D65 white of Table 3 colorimetry.
pub const REC709: Chromaticities = Chromaticities {
    red: [0.640, 0.330],
    green: [0.300, 0.600],
    blue: [0.150, 0.060],
    white: D65,
};

/// ITU-R BT.2020-2 Table 4 / BT.2100-2 Table 2. Same primaries for Rec.2100.
pub const REC2020: Chromaticities = Chromaticities {
    red: [0.708, 0.292],
    green: [0.170, 0.797],
    blue: [0.131, 0.046],
    white: D65,
};

/// DCI-P3 primaries (SMPTE ST 431-2 Table 1 / SMPTE EG 432-1) with the D65 white of EG 432-1 (P3-D65).
pub const P3_D65: Chromaticities = Chromaticities {
    red: [0.680, 0.320],
    green: [0.265, 0.690],
    blue: [0.150, 0.060],
    white: D65,
};

/// DaVinci Wide Gamut: Blackmagic Design, "DaVinci Resolve 17 Wide Gamut Intermediate" v1.1 (Aug 2021), p.3.
pub const DWG: Chromaticities = Chromaticities {
    red: [0.8000, 0.3130],
    green: [0.1682, 0.9877],
    blue: [0.0790, -0.1155],
    white: D65,
};

/// ACES AP0, SMPTE ST 2065-1:2021 Table 1. White is the ACES white (~D60), not D65.
pub const AP0: Chromaticities = Chromaticities {
    red: [0.73470, 0.26530],
    green: [0.00000, 1.00000],
    blue: [0.00010, -0.07700],
    white: [0.32168, 0.33767],
};

/// ACES AP1, S-2014-004 / S-2016-001. Same white as AP0.
pub const AP1: Chromaticities = Chromaticities {
    red: [0.713, 0.293],
    green: [0.165, 0.830],
    blue: [0.128, 0.044],
    white: [0.32168, 0.33767],
};

pub fn vmul(v: V3, m: &M3) -> V3 {
    [
        v[0] * m[0][0] + v[1] * m[1][0] + v[2] * m[2][0],
        v[0] * m[0][1] + v[1] * m[1][1] + v[2] * m[2][1],
        v[0] * m[0][2] + v[1] * m[1][2] + v[2] * m[2][2],
    ]
}

pub fn mmul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    o
}

pub fn transpose(m: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = m[j][i];
        }
    }
    o
}

pub fn invert(m: &M3) -> M3 {
    let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
    let c01 = m[1][2] * m[2][0] - m[1][0] * m[2][2];
    let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];
    let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;
    let id = 1.0 / det;
    [
        [
            c00 * id,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * id,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * id,
        ],
        [
            c01 * id,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * id,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * id,
        ],
        [
            c02 * id,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * id,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * id,
        ],
    ]
}

/// RGB -> XYZ for white luminance Y = 1 (SMPTE RP 177 derivation; same as CTL `RGBtoXYZ_f33(C, 1.0)`).
pub fn rgb_to_xyz(c: &Chromaticities) -> M3 {
    let p = |xy: [f64; 2]| [xy[0] / xy[1], 1.0, (1.0 - xy[0] - xy[1]) / xy[1]];
    let (r, g, b, w) = (p(c.red), p(c.green), p(c.blue), p(c.white));
    // Columns are the primaries (column convention): solve P * s = W.
    let pm: M3 = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
    let s = {
        let inv = invert(&pm);
        [
            inv[0][0] * w[0] + inv[0][1] * w[1] + inv[0][2] * w[2],
            inv[1][0] * w[0] + inv[1][1] * w[1] + inv[1][2] * w[2],
            inv[2][0] * w[0] + inv[2][1] * w[1] + inv[2][2] * w[2],
        ]
    };
    // Row convention: row i is primary i scaled by s[i].
    [
        [r[0] * s[0], r[1] * s[0], r[2] * s[0]],
        [g[0] * s[1], g[1] * s[1], g[2] * s[1]],
        [b[0] * s[2], b[1] * s[2], b[2] * s[2]],
    ]
}

pub fn xyz_to_rgb(c: &Chromaticities) -> M3 {
    invert(&rgb_to_xyz(c))
}

/// Linear RGB of `from` to linear RGB of `to`. Both must share a white point (all the display spaces
/// here are D65 and so is DaVinci Wide Gamut), so no chromatic adaptation is needed.
pub fn rgb_to_rgb(from: &Chromaticities, to: &Chromaticities) -> M3 {
    mmul(&rgb_to_xyz(from), &xyz_to_rgb(to))
}

/// Luma weights of a space: the Y column of its RGB->XYZ matrix (ADR 0004).
pub fn luma_weights(c: &Chromaticities) -> V3 {
    let m = rgb_to_xyz(c);
    [m[0][1], m[1][1], m[2][1]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn rec709_matrix_matches_bt709_published_values() {
        // BT.709 luma coefficients are 0.2126 / 0.7152 / 0.0722 (4 d.p.); with the 4 d.p. white 0.3127,0.3290
        // the exact derivation gives 0.212639 / 0.715169 / 0.072192.
        let w = luma_weights(&REC709);
        assert!(
            close(w[0], 0.2126, 1e-4) && close(w[1], 0.7152, 1e-4) && close(w[2], 0.0722, 1e-4),
            "{w:?}"
        );
        assert!(close(w[0], 0.212639, 1e-6) && close(w[2], 0.072192, 1e-6));
    }

    #[test]
    fn rec2020_luma_weights_match_bt2020() {
        // BT.2020: 0.2627 0.6780 0.0593 (the standard rounds the exact 0.262700.. values)
        let w = luma_weights(&REC2020);
        assert!(
            close(w[0], 0.2627, 5e-5) && close(w[1], 0.6780, 5e-5) && close(w[2], 0.0593, 5e-5),
            "{w:?}"
        );
    }

    #[test]
    fn p3_d65_luma_weights_match_adr_0004() {
        let w = luma_weights(&P3_D65);
        assert!(
            close(w[0], 0.2290, 5e-5) && close(w[1], 0.6917, 5e-5) && close(w[2], 0.0793, 5e-5),
            "{w:?}"
        );
    }

    #[test]
    fn dwg_matrix_matches_blackmagic_whitepaper() {
        // DaVinci WG RGB -> XYZ, whitepaper p.3 (column convention, so compare the transpose).
        let wp: M3 = [
            [0.70062239, 0.14877482, 0.10105872],
            [0.27411851, 0.87363190, -0.14775041],
            [-0.09896291, -0.13789533, 1.32591599],
        ];
        let m = transpose(&rgb_to_xyz(&DWG));
        for i in 0..3 {
            for j in 0..3 {
                assert!(
                    close(m[i][j], wp[i][j], 2e-8),
                    "[{i}][{j}] {} vs {}",
                    m[i][j],
                    wp[i][j]
                );
            }
        }
        // XYZ -> DWG
        let wpi: M3 = [
            [1.51667204, -0.28147805, -0.14696363],
            [-0.46491710, 1.25142378, 0.17488461],
            [0.06484905, 0.10913934, 0.76141462],
        ];
        let mi = transpose(&xyz_to_rgb(&DWG));
        for i in 0..3 {
            for j in 0..3 {
                assert!(close(mi[i][j], wpi[i][j], 2e-8));
            }
        }
    }

    #[test]
    fn ap0_to_ap1_matches_ap1_spec() {
        // S-2014-004 AP0 -> AP1 (first row, column convention)
        let m = transpose(&rgb_to_rgb(&AP0, &AP1));
        assert!(close(m[0][0], 1.4514393161, 1e-9) && close(m[0][1], -0.2365107469, 1e-9));
        assert!(close(m[1][1], 1.1762296998, 1e-9) && close(m[2][2], 0.9977163014, 1e-9));
    }

    #[test]
    fn white_maps_to_white_and_inverse_round_trips() {
        let m = rgb_to_rgb(&DWG, &REC709);
        let w = vmul([1.0, 1.0, 1.0], &m);
        for c in w {
            assert!(close(c, 1.0, 1e-12), "{w:?}");
        }
        let back = vmul(vmul([0.3, 0.6, 0.1], &m), &rgb_to_rgb(&REC709, &DWG));
        assert!(
            close(back[0], 0.3, 1e-12) && close(back[1], 0.6, 1e-12) && close(back[2], 0.1, 1e-12)
        );
    }
}
