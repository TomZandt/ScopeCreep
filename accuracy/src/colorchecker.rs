//! ColorChecker Classic test frames and the value every patch should read (milestone 2, ticket #52).
//!
//! The chart is built in code from X-Rite's "ColorChecker24 - After November 2014" L*a*b* values (D50, 2 degrees),
//! adapted with Bradford to the white of each Timeline Colour Space (D65, or the ACES white). Nothing is read from
//! or written to image files. Sources, formulas and the full table of expected values:
//! `docs/research/colorchecker-reference-values.md`.
//!
//! A [`ColorChecker`] knows each [`Patch`]'s expected values (linear Y, stops from 18 %, linear and encoded RGB
//! in the Timeline Colour Space, Rec.709 luma, ARRI-style band, hue, out-of-gamut channels). [`ColorChecker::frame`]
//! paints the 6 x 4 chart as a Feed-ready picture: RGBA 32-bit float, bottom row first, as the Sender writes it.

use crate::colour::{self, AP1, Chromaticities, DWG, M3, REC709, V3, vmul};
use crate::transfer;

/// A Timeline Colour Space a chart can be built in: the picture's primaries and encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineSpace {
    /// ACES AP1 primaries, ACEScct encoding.
    AcesCct,
    /// ACES AP1 primaries, linear.
    AcesCg,
    /// DaVinci Wide Gamut primaries, DaVinci Intermediate encoding.
    DavinciIntermediate,
    /// Rec.709 primaries, pure 2.4 gamma (Resolve's "Rec.709 Gamma 2.4").
    Rec709Gamma24,
}

impl TimelineSpace {
    pub const ALL: [TimelineSpace; 4] = [
        TimelineSpace::AcesCct,
        TimelineSpace::AcesCg,
        TimelineSpace::DavinciIntermediate,
        TimelineSpace::Rec709Gamma24,
    ];

    /// Resolve's name for the space, as the Colour Setup says it.
    pub fn name(self) -> &'static str {
        match self {
            TimelineSpace::AcesCct => "ACEScct",
            TimelineSpace::AcesCg => "ACEScg",
            TimelineSpace::DavinciIntermediate => "DaVinci WG/Intermediate",
            TimelineSpace::Rec709Gamma24 => "Rec.709 Gamma 2.4",
        }
    }

    fn primaries(self) -> &'static Chromaticities {
        match self {
            TimelineSpace::AcesCct | TimelineSpace::AcesCg => &AP1,
            TimelineSpace::DavinciIntermediate => &DWG,
            TimelineSpace::Rec709Gamma24 => &REC709,
        }
    }

    fn encode(self, linear: f64) -> f64 {
        match self {
            TimelineSpace::AcesCct => transfer::acescct_encode(linear),
            TimelineSpace::AcesCg => linear,
            TimelineSpace::DavinciIntermediate => transfer::di_encode(linear),
            TimelineSpace::Rec709Gamma24 => {
                linear.signum() * transfer::gamma_encode(linear.abs(), 2.4)
            }
        }
    }
}

/// The ARRI-style bands of the Output picture's Rec.709 luma (spec #49, user story 56). Modelled, never claimed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArriBand {
    Purple,
    Blue,
    Green,
    Pink,
    Yellow,
    Red,
}

impl ArriBand {
    /// The band a luma (0 to 1, full range) falls in, if any.
    pub fn of_luma(luma: f64) -> Option<ArriBand> {
        let pct = luma * 100.0;
        let bands = [
            (0.0, 2.5, ArriBand::Purple),
            (2.5, 4.0, ArriBand::Blue),
            (38.0, 42.0, ArriBand::Green),
            (52.0, 56.0, ArriBand::Pink),
            (97.0, 99.0, ArriBand::Yellow),
            (99.0, 100.0, ArriBand::Red),
        ];
        bands
            .iter()
            .find(|(from, to, _)| pct >= *from && pct < *to)
            .map(|b| b.2)
    }
}

/// One patch of the chart and what it should read.
#[derive(Clone, Debug)]
pub struct Patch {
    /// 1 to 24, left to right and top to bottom.
    pub number: usize,
    pub name: &'static str,
    /// Linear Y at true reflectance (white = 1), of the colour adapted to D65.
    pub y: f64,
    /// log2(Y / 0.18).
    pub stops: f64,
    /// Linear RGB in the Timeline Colour Space's primaries.
    pub linear: V3,
    /// The code values in the frame: `linear` through the Timeline Colour Space's encoding.
    pub encoded: V3,
    /// Linear Rec.709 RGB of the same light (a channel below 0 or above 1 is out of the Rec.709 gamut).
    pub rec709_linear: V3,
    /// Rec.709 luma (0.2126 R' + 0.7152 G' + 0.0722 B', full range, 0 to 1) of the BT.709-encoded patch.
    pub rec709_luma: f64,
    pub arri_band: Option<ArriBand>,
    /// atan2(Cr, Cb) in BT.709 Cb/Cr, 0 to 360 degrees, 0 at +Cb, counter-clockwise.
    pub hue_degrees: f64,
    /// Indices (0 = R, 1 = G, 2 = B) of the channels of `rec709_linear` below 0 or above 1.
    pub out_of_gamut_rec709: Vec<usize>,
}

/// X-Rite ColorChecker24, after November 2014: L*, a*, b* (D50, 2 degrees) in chart order. Source:
/// <https://zenodo.org/records/3245895>.
const LAB_D50: [(&str, [f64; 3]); 24] = [
    ("dark skin", [37.54, 14.37, 14.92]),
    ("light skin", [64.66, 19.27, 17.5]),
    ("blue sky", [49.32, -3.82, -22.54]),
    ("foliage", [43.46, -12.74, 22.72]),
    ("blue flower", [54.94, 9.61, -24.79]),
    ("bluish green", [70.48, -32.26, -0.37]),
    ("orange", [62.73, 35.83, 56.5]),
    ("purplish blue", [39.43, 10.75, -45.17]),
    ("moderate red", [50.57, 48.64, 16.67]),
    ("purple", [30.1, 22.54, -20.87]),
    ("yellow green", [71.77, -24.13, 58.19]),
    ("orange yellow", [71.51, 18.24, 67.37]),
    ("blue", [28.37, 15.42, -49.8]),
    ("green", [54.38, -39.72, 32.27]),
    ("red", [42.43, 51.05, 28.62]),
    ("yellow", [81.8, 2.67, 80.41]),
    ("magenta", [50.63, 51.28, -14.12]),
    ("cyan", [49.57, -29.71, -28.32]),
    ("white 9.5 (.05 D)", [95.19, -1.03, 2.93]),
    ("neutral 8 (.23 D)", [81.29, -0.57, 0.44]),
    ("neutral 6.5 (.44 D)", [66.89, -0.75, -0.06]),
    ("neutral 5 (.70 D)", [50.76, -0.13, 0.14]),
    ("neutral 3.5 (1.05 D)", [35.63, -0.46, -0.48]),
    ("black 2 (1.5 D)", [20.64, 0.07, -0.46]),
];

/// The D50 white of the ICC profile connection space (the convention for D50 L*a*b*).
const D50_XYZ: V3 = [0.9642, 1.0, 0.8249];

fn lab_to_xyz(lab: [f64; 3], white: V3) -> V3 {
    let fy = (lab[0] + 16.0) / 116.0;
    let (fx, fz) = (fy + lab[1] / 500.0, fy - lab[2] / 200.0);
    let inv = |f: f64| {
        if f.powi(3) > 216.0 / 24389.0 {
            f.powi(3)
        } else {
            (116.0 * f - 16.0) * 27.0 / 24389.0
        }
    };
    [white[0] * inv(fx), white[1] * inv(fy), white[2] * inv(fz)]
}

fn white_xyz(c: &Chromaticities) -> V3 {
    [
        c.white[0] / c.white[1],
        1.0,
        (1.0 - c.white[0] - c.white[1]) / c.white[1],
    ]
}

/// Bradford chromatic adaptation of `xyz` from the `from` white to the `to` white.
fn bradford(xyz: V3, from: V3, to: V3) -> V3 {
    // Column convention matrix, applied below as rows (v * transpose).
    const M: M3 = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    let m = colour::transpose(&M);
    let (cone, src, dst) = (vmul(xyz, &m), vmul(from, &m), vmul(to, &m));
    let scaled = [
        cone[0] * dst[0] / src[0],
        cone[1] * dst[1] / src[1],
        cone[2] * dst[2] / src[2],
    ];
    vmul(scaled, &colour::invert(&m))
}

/// The weights that give the Y of a colour in `c`'s linear RGB once it is adapted (Bradford) to the D65 white the
/// chart's table of Y uses. For a D65 space they are its luma weights; for ACES they differ by a hair for coloured
/// patches, because the ACES white is not D65. Neutrals are unchanged.
pub fn luma_weights_adapted_to_d65(c: &Chromaticities) -> V3 {
    let m = colour::rgb_to_xyz(c);
    let d65 = white_xyz(&REC709);
    let column = |j: usize| bradford(m[j], white_xyz(c), d65)[1];
    [column(0), column(1), column(2)]
}

fn linear_rgb(xyz_d50: V3, c: &Chromaticities) -> V3 {
    let adapted = bradford(xyz_d50, D50_XYZ, white_xyz(c));
    vmul(adapted, &colour::xyz_to_rgb(c))
}

/// Luma and hue of a linear Rec.709 colour, from its BT.709-encoded channels.
fn luma_and_hue(rec709_linear: V3) -> (f64, f64) {
    // Mirrored about zero, so an out-of-gamut channel keeps its sign.
    let e = rec709_linear.map(|l| l.signum() * transfer::bt709_oetf(l.abs()));
    let luma = 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
    let (cb, cr) = ((e[2] - luma) / 1.8556, (e[0] - luma) / 1.5748);
    (luma, cr.atan2(cb).to_degrees().rem_euclid(360.0))
}

fn patch(space: TimelineSpace, number: usize, name: &'static str, xyz_d50: V3) -> Patch {
    // Y is that of the colour adapted to D65: Bradford moves Y a little for coloured patches.
    let y = bradford(xyz_d50, D50_XYZ, white_xyz(&REC709))[1];
    let linear = linear_rgb(xyz_d50, space.primaries());
    let rec709_linear = linear_rgb(xyz_d50, &REC709);
    let (rec709_luma, hue_degrees) = luma_and_hue(rec709_linear);
    Patch {
        number,
        name,
        y,
        stops: (y / 0.18).log2(),
        linear,
        encoded: linear.map(|l| space.encode(l)),
        rec709_linear,
        rec709_luma,
        arri_band: ArriBand::of_luma(rec709_luma),
        hue_degrees,
        out_of_gamut_rec709: (0..3)
            .filter(|&c| rec709_linear[c] < 0.0 || rec709_linear[c] > 1.0)
            .collect(),
    }
}

/// A ColorChecker Classic chart in one Timeline Colour Space.
pub struct ColorChecker {
    space: TimelineSpace,
    patches: Vec<Patch>,
    grey_18: Patch,
}

impl ColorChecker {
    pub fn new(space: TimelineSpace) -> ColorChecker {
        let patches = LAB_D50
            .iter()
            .enumerate()
            .map(|(i, (name, lab))| {
                let xyz = lab_to_xyz(*lab, D50_XYZ);
                patch(space, i + 1, name, xyz)
            })
            .collect();
        // 18 % grey: a neutral of Y = 0.18 with the chart's D50 white.
        let grey_xyz = [D50_XYZ[0] * 0.18, 0.18, D50_XYZ[2] * 0.18];
        ColorChecker {
            space,
            patches,
            grey_18: patch(space, 0, "18 % grey", grey_xyz),
        }
    }

    pub fn space(&self) -> TimelineSpace {
        self.space
    }

    /// The 24 patches in chart order (patch `n` is `patches()[n - 1]`).
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    /// Middle grey (18 % linear), for reference: not a patch of the chart.
    pub fn grey_18(&self) -> &Patch {
        &self.grey_18
    }

    /// The chart as a picture of `width` x `height`: six patches across and four down, each patch inset in its
    /// cell, with linear black around them. RGBA 32-bit float, bottom row first.
    pub fn frame(&self, width: u32, height: u32) -> Frame {
        let black = self.space.encode(0.0) as f32;
        let mut pixels = vec![black; width as usize * height as usize * 4];
        for px in pixels.as_chunks_mut::<4>().0 {
            px[3] = 1.0;
        }
        let frame = Frame {
            width,
            height,
            pixels: Vec::new(),
        };
        for p in &self.patches {
            let r = frame.patch_rect(p.number);
            for y in r.y0..r.y1 {
                let row = (height - 1 - y) as usize; // rows are stored bottom first
                for x in r.x0..r.x1 {
                    let i = (row * width as usize + x as usize) * 4;
                    for c in 0..3 {
                        pixels[i + c] = p.encoded[c] as f32;
                    }
                }
            }
        }
        Frame {
            width,
            height,
            pixels,
        }
    }
}

/// A rectangle of picture pixels, from the top left: `x0..x1` across and `y0..y1` down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

/// A painted chart: the layout and the pixels.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// RGBA 32-bit float, tightly packed, bottom row first (the Feed's layout).
    pub pixels: Vec<f32>,
}

impl Frame {
    /// Where patch `number` (1 to 24) is painted, from the top left: its cell of the 6 x 4 grid inset by a tenth
    /// of the cell on every side.
    pub fn patch_rect(&self, number: usize) -> Rect {
        let (col, row) = ((number - 1) % 6, (number - 1) / 6);
        let cell = |i: usize, n: usize, size: u32| {
            (i as u32 * size / n as u32, (i as u32 + 1) * size / n as u32)
        };
        let (cx0, cx1) = cell(col, 6, self.width);
        let (cy0, cy1) = cell(row, 4, self.height);
        let (ix, iy) = ((cx1 - cx0) / 10, (cy1 - cy0) / 10);
        Rect {
            x0: cx0 + ix,
            y0: cy0 + iy,
            x1: cx1 - ix,
            y1: cy1 - iy,
        }
    }
}
