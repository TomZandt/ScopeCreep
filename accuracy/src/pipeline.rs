//! Output Conversions: Timeline Colour Space pixel -> Output Colour Space code value, built only from the
//! published definitions. One `Pipeline` per Resolve Colour Setup in the test set.
//!
//! What Resolve does that the standards leave open, found by comparing against Resolve's own renders and
//! recorded here as *observed behaviour*, not as published fact:
//!  * Float output is not clamped: out-of-range and negative values are carried through.
//!  * "Rec.709 Gamma 2.4" and "P3-D65" outputs in Colour Managed mode use a sign-preserving power law
//!    (negative linear values come out as `-|x|^(1/g)`); P3-D65 uses gamma 2.6 (DCI), not 2.4.
//!  * PQ output cannot carry negatives (PQ is defined for L >= 0), so those encode as 0.
//!  * 1.0 in DaVinci Wide Gamut / Intermediate scene linear is 100 nits (also what Blackmagic's table says).

use crate::aces2::AcesOutputTransform;
use crate::colour::{self, AP0, AP1, Chromaticities, DWG, P3_D65, REC709, REC2020, V3, vmul};
use crate::transfer;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Encoding {
    /// Sign-preserving power law with this gamma.
    SignedGamma(f64),
    /// SMPTE ST 2084 with `nits` cd/m^2 per unit of linear light.
    Pq { nits_per_unit: f64 },
}

#[derive(Clone, Debug)]
pub enum Pipeline {
    /// Timeline and output are the same space: nothing happens.
    Identity,
    /// DaVinci Wide Gamut / Intermediate -> display primaries -> display encoding. No tone or gamut mapping.
    DwgToDisplay {
        primaries: Chromaticities,
        matrix: colour::M3,
        encoding: Encoding,
    },
    /// ACEScct -> ACES2065-1 -> ACES 2.0 Output Transform.
    Aces2(Box<AcesOutputTransform>),
}

fn signed_pow(x: f64, e: f64) -> f64 {
    x.signum() * x.abs().powf(e)
}

impl Encoding {
    pub fn encode(&self, lin: f64) -> f64 {
        match *self {
            Encoding::SignedGamma(g) => signed_pow(lin, 1.0 / g),
            Encoding::Pq { nits_per_unit } => transfer::pq_encode(lin * nits_per_unit),
        }
    }
}

impl Encoding {
    /// Code value back to display-linear light, relative to 100 nit reference white (so 1.0 = 100 nit for PQ).
    pub fn decode(&self, cv: f64) -> f64 {
        match *self {
            Encoding::SignedGamma(g) => signed_pow(cv, g),
            Encoding::Pq { nits_per_unit } => transfer::pq_decode(cv) / nits_per_unit,
        }
    }
}

impl Pipeline {
    pub fn dwg_to_display(primaries: Chromaticities, encoding: Encoding) -> Self {
        Pipeline::DwgToDisplay {
            primaries,
            matrix: colour::rgb_to_rgb(&DWG, &primaries),
            encoding,
        }
    }
    pub fn dwg_rec709() -> Self {
        Self::dwg_to_display(REC709, Encoding::SignedGamma(2.4))
    }
    pub fn dwg_p3d65() -> Self {
        Self::dwg_to_display(P3_D65, Encoding::SignedGamma(2.6))
    }
    pub fn dwg_rec2100_pq() -> Self {
        Self::dwg_to_display(
            REC2020,
            Encoding::Pq {
                nits_per_unit: 100.0,
            },
        )
    }

    /// Output code value to display-linear light (1.0 = the transform's reference white). Used to judge errors in
    /// light as well as in code values.
    pub fn decode(&self, cv: f64) -> f64 {
        match self {
            Pipeline::Identity => signed_pow(cv, 2.4),
            Pipeline::DwgToDisplay { encoding, .. } => encoding.decode(cv),
            Pipeline::Aces2(ot) => ot.decode(cv),
        }
    }

    pub fn apply(&self, rgb: V3) -> V3 {
        match self {
            Pipeline::Identity => rgb,
            Pipeline::DwgToDisplay {
                matrix, encoding, ..
            } => {
                let lin = [
                    transfer::di_decode(rgb[0]),
                    transfer::di_decode(rgb[1]),
                    transfer::di_decode(rgb[2]),
                ];
                let disp = vmul(lin, matrix);
                [
                    encoding.encode(disp[0]),
                    encoding.encode(disp[1]),
                    encoding.encode(disp[2]),
                ]
            }
            Pipeline::Aces2(ot) => {
                let lin = [
                    transfer::acescct_decode(rgb[0]),
                    transfer::acescct_decode(rgb[1]),
                    transfer::acescct_decode(rgb[2]),
                ];
                // ACEScct is an AP1 encoding; the Output Transform takes ACES2065-1 (AP0).
                let aces = vmul(lin, &colour::rgb_to_rgb(&AP1, &AP0));
                ot.forward(aces)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    fn di(l: f64) -> f64 {
        transfer::di_encode(l)
    }

    #[test]
    fn identity_changes_nothing() {
        assert_eq!(Pipeline::Identity.apply([-0.3, 0.5, 4.0]), [-0.3, 0.5, 4.0]);
    }

    #[test]
    fn dwg_neutrals_to_rec709_gamma24() {
        let p = Pipeline::dwg_rec709();
        let o = p.apply([di(0.18); 3]);
        for c in o {
            assert!(close(c, 0.18f64.powf(1.0 / 2.4), 1e-9), "{o:?}");
        }
        let w = p.apply([di(1.0); 3]);
        assert!(close(w[0], 1.0, 1e-9) && close(w[1], 1.0, 1e-9) && close(w[2], 1.0, 1e-9));
    }

    #[test]
    fn negative_linear_is_sign_preserved_not_clamped() {
        let o = Pipeline::dwg_rec709().apply([di(-0.5); 3]);
        assert!(close(o[0], -(0.5f64.powf(1.0 / 2.4)), 1e-9), "{o:?}");
    }

    #[test]
    fn p3_d65_uses_gamma_26() {
        let o = Pipeline::dwg_p3d65().apply([di(0.18); 3]);
        assert!(close(o[1], 0.18f64.powf(1.0 / 2.6), 1e-9), "{o:?}");
    }

    #[test]
    fn pq_puts_scene_one_at_100_nits_and_clamps_negatives_to_zero() {
        let p = Pipeline::dwg_rec2100_pq();
        let o = p.apply([di(1.0); 3]);
        assert!(close(o[0], 0.508078, 1e-6), "{o:?}"); // PQ(100 nit)
        let n = p.apply([di(-1.0), di(1.0), di(1.0)]);
        assert!(n[0] < 1e-6, "{n:?}"); // PQ cannot carry a negative; only the 7e-7 PQ(0) offset remains
    }

    #[test]
    fn pure_dwg_red_leaves_rec709_gamut_so_a_channel_goes_negative() {
        // DWG primaries are wider than Rec.709: a saturated DWG red must produce a negative green in Rec.709 linear.
        let o = Pipeline::dwg_rec709().apply([di(1.0), di(0.0), di(0.0)]);
        assert!(o[1] < 0.0 && o[0] > 1.0, "{o:?}");
    }

    #[test]
    fn decode_inverts_encode_for_neutrals() {
        for p in [
            Pipeline::dwg_rec709(),
            Pipeline::dwg_p3d65(),
            Pipeline::dwg_rec2100_pq(),
        ] {
            for l in [0.01, 0.18, 1.0, 4.0] {
                let o = p.apply([di(l); 3]);
                assert!(
                    close(p.decode(o[1]), l, 1e-6 * l.max(1.0)),
                    "{l}: {}",
                    p.decode(o[1])
                );
            }
        }
    }
}
