//! Transfer functions (encode = linear to code value, decode = code value to linear).

// ---- DaVinci Intermediate ---------------------------------------------------------------------------------
// Source: Blackmagic Design, "DaVinci Resolve 17 Wide Gamut Intermediate" v1.1 (Aug 2021), p.4.
pub const DI_A: f64 = 0.0075;
pub const DI_B: f64 = 7.0;
pub const DI_C: f64 = 0.07329248;
pub const DI_M: f64 = 10.44426855;
pub const DI_LIN_CUT: f64 = 0.00262409;
pub const DI_LOG_CUT: f64 = 0.02740668;

pub fn di_encode(l: f64) -> f64 {
    if l > DI_LIN_CUT {
        ((l + DI_A).log2() + DI_B) * DI_C
    } else {
        l * DI_M
    }
}

pub fn di_decode(v: f64) -> f64 {
    if v > DI_LOG_CUT {
        2f64.powf(v / DI_C - DI_B) - DI_A
    } else {
        v / DI_M
    }
}

// ---- ACEScct ----------------------------------------------------------------------------------------------
// Source: Academy S-2016-001, "ACEScct - A Quasi-Logarithmic Encoding of ACES Within the ACES Color Space".
pub const CCT_X_BRK: f64 = 0.0078125;
pub const CCT_Y_BRK: f64 = 0.155251141552511;
pub const CCT_A: f64 = 10.5402377416545;
pub const CCT_B: f64 = 0.0729055341958355;
pub const CCT_LOG_SCALE: f64 = 17.52;
pub const CCT_LOG_OFFSET: f64 = 9.72;

pub fn acescct_encode(l: f64) -> f64 {
    if l <= CCT_X_BRK {
        CCT_A * l + CCT_B
    } else {
        (l.log2() + CCT_LOG_OFFSET) / CCT_LOG_SCALE
    }
}

pub fn acescct_decode(v: f64) -> f64 {
    if v <= CCT_Y_BRK {
        (v - CCT_B) / CCT_A
    } else {
        2f64.powf(v * CCT_LOG_SCALE - CCT_LOG_OFFSET)
    }
}

// ---- SMPTE ST 2084 (PQ) ----------------------------------------------------------------------------------
// Source: SMPTE ST 2084:2014 Table / ITU-R BT.2100-2 Table 4 (constants m1, m2, c1, c2, c3).
pub const PQ_M1: f64 = 2610.0 / 16384.0;
pub const PQ_M2: f64 = 2523.0 / 4096.0 * 128.0;
pub const PQ_C1: f64 = 3424.0 / 4096.0;
pub const PQ_C2: f64 = 2413.0 / 4096.0 * 32.0;
pub const PQ_C3: f64 = 2392.0 / 4096.0 * 32.0;

/// Absolute luminance in cd/m^2 (0..10000) to PQ code value (0..1). Inverse EOTF.
pub fn pq_encode(nits: f64) -> f64 {
    let y = (nits / 10000.0).max(0.0);
    let ym = y.powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * ym) / (1.0 + PQ_C3 * ym)).powf(PQ_M2)
}

/// PQ code value to absolute luminance in cd/m^2. EOTF.
pub fn pq_decode(v: f64) -> f64 {
    let vp = v.max(0.0).powf(1.0 / PQ_M2);
    let num = (vp - PQ_C1).max(0.0);
    10000.0 * (num / (PQ_C2 - PQ_C3 * vp)).powf(1.0 / PQ_M1)
}

// ---- Display gamma ---------------------------------------------------------------------------------------

/// Pure power law inverse EOTF. Rec.709 "Gamma 2.4" and BT.1886 with Lb = 0 (BT.1886 Annex 1, eq. 1: L = a(V+b)^2.4
/// with b = 0, a = Lw) are both this.
pub fn gamma_encode(l: f64, gamma: f64) -> f64 {
    l.max(0.0).powf(1.0 / gamma)
}

pub fn gamma_decode(v: f64, gamma: f64) -> f64 {
    v.max(0.0).powf(gamma)
}

/// ITU-R BT.709-6 OETF (camera side). Kept for completeness and tests; Resolve's "Rec.709 Gamma 2.4" is not this.
pub fn bt709_oetf(l: f64) -> f64 {
    if l < 0.018 {
        4.5 * l
    } else {
        1.099 * l.powf(0.45) - 0.099
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn di_matches_whitepaper_mapping_table() {
        // Blackmagic whitepaper p.4 "Mapping Values"
        for (lin, di) in [
            (-0.01, -0.104443),
            (0.0, 0.0),
            (0.18, 0.336043),
            (1.0, 0.513837),
            (10.0, 0.756599),
            (40.0, 0.903125),
            (100.0, 1.0),
        ] {
            assert!(
                close(di_encode(lin), di, 1e-6),
                "{lin}: {} vs {di}",
                di_encode(lin)
            );
        }
    }

    #[test]
    fn di_round_trips_across_the_cut() {
        for l in [-0.5, -0.01, 0.0, 0.001, 0.0027, 0.18, 1.0, 50.0, 100.0] {
            assert!(
                close(di_decode(di_encode(l)), l, 1e-12 * l.abs().max(1.0)),
                "{l}"
            );
        }
    }

    #[test]
    fn di_is_continuous_at_the_cuts() {
        assert!(close(di_encode(DI_LIN_CUT), DI_LOG_CUT, 1e-7));
        assert!(close(
            di_encode(DI_LIN_CUT + 1e-12),
            di_encode(DI_LIN_CUT),
            1e-9
        ));
    }

    #[test]
    fn acescct_mid_grey_and_breakpoints() {
        // S-2016-001: 18% grey encodes to 0.4135884
        assert!(close(acescct_encode(0.18), 0.4135884, 1e-7));
        assert!(close(acescct_encode(CCT_X_BRK), CCT_Y_BRK, 1e-12));
        assert!(close(acescct_decode(0.4135884), 0.18, 1e-6));
        for l in [-0.0035, 0.0, 0.005, CCT_X_BRK, 0.01, 0.18, 16.0] {
            assert!(
                close(
                    acescct_decode(acescct_encode(l)),
                    l,
                    1e-12 * l.abs().max(1.0)
                ),
                "{l}"
            );
        }
    }

    #[test]
    fn pq_matches_st2084_reference_points() {
        // Widely tabulated PQ code values (BT.2100 / ST 2084): 100 nit 0.5081, 1000 nit 0.7518, 10000 nit 1.0
        assert!(
            close(pq_encode(100.0), 0.508078, 1e-6),
            "{}",
            pq_encode(100.0)
        );
        assert!(
            close(pq_encode(1000.0), 0.751827, 1e-6),
            "{}",
            pq_encode(1000.0)
        );
        assert!(close(pq_encode(10000.0), 1.0, 1e-12));
        assert!(close(pq_encode(0.0), 0.0, 1e-6)); // PQ(0) = 7.3e-7 offset (c1 term): ~0
        assert!(close(pq_decode(pq_encode(203.0)), 203.0, 1e-9));
    }

    #[test]
    fn gamma_round_trips_and_clamps_negatives() {
        assert!(close(
            gamma_decode(gamma_encode(0.18, 2.4), 2.4),
            0.18,
            1e-14
        ));
        assert_eq!(gamma_encode(-0.5, 2.4), 0.0);
        assert!(close(gamma_encode(0.5, 2.4), 0.74915, 1e-4));
    }

    #[test]
    fn bt709_oetf_known_points() {
        assert!(close(bt709_oetf(0.18), 0.4090077, 1e-6));
        assert!(close(bt709_oetf(1.0), 1.0, 1e-12));
    }
}
