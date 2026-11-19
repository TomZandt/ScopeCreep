//! What a display measures in: the Measured Space (primaries and white point), the luma weights taken from it
//! (ADR 0004), and the wording that states both (ADR 0007). No display depends on the waveform for these; the
//! display context resolves the luma weights from the Luma waveform's setting once.
use crate::colour_setup::{KnownColourSetup, Levels, Provenance};
use crate::settings::Settings;

/// The Rec.709 luma weights (R, G, B): the starting value of every display's weights before a Measurement sets them.
pub const REC709_LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];

/// The luma weights the waveform uses until the user picks otherwise (the first choice of its menu).
pub const DEFAULT_LUMA_WEIGHTS: LumaWeights = LumaWeights::FromPrimaries;

/// How the luma weights are chosen for the Measured Space (ADR 0004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LumaWeights {
    /// The Y row of the Measured Space's RGB-to-XYZ matrix, from its primaries and white point.
    FromPrimaries,
    /// The weights Resolve's own waveform uses: Rec.709 weights for Rec.709, P3-D65 and P3-DCI; Rec.2020 weights
    /// for Rec.2020 and Rec.2100.
    MatchResolve,
}

/// A colour space the waveform can measure in, by its primaries and white point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasuredSpace {
    Rec709,
    P3D65,
    P3Dci,
    Rec2020,
    Rec2100Pq,
    Rec2100Hlg,
}

/// CIE 1931 xy chromaticities: red, green, blue, white.
type Chromaticities = [[f64; 2]; 4];
const D65: [f64; 2] = [0.3127, 0.3290];
const REC709: Chromaticities = [[0.640, 0.330], [0.300, 0.600], [0.150, 0.060], D65];
const P3_D65: Chromaticities = [[0.680, 0.320], [0.265, 0.690], [0.150, 0.060], D65];
const P3_DCI: Chromaticities = [[0.680, 0.320], [0.265, 0.690], [0.150, 0.060], [0.314, 0.351]];
const REC2020: Chromaticities = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046], D65];

impl MeasuredSpace {
    pub fn name(self) -> &'static str {
        match self {
            MeasuredSpace::Rec709 => "Rec.709",
            MeasuredSpace::P3D65 => "P3-D65",
            MeasuredSpace::P3Dci => "P3-DCI",
            MeasuredSpace::Rec2020 => "Rec.2020",
            MeasuredSpace::Rec2100Pq => "Rec.2100 PQ",
            MeasuredSpace::Rec2100Hlg => "Rec.2100 HLG",
        }
    }

    fn chromaticities(self) -> Chromaticities {
        match self {
            MeasuredSpace::Rec709 => REC709,
            MeasuredSpace::P3D65 => P3_D65,
            MeasuredSpace::P3Dci => P3_DCI,
            MeasuredSpace::Rec2020 | MeasuredSpace::Rec2100Pq | MeasuredSpace::Rec2100Hlg => REC2020,
        }
    }

    /// The space whose primaries Resolve's own waveform takes its weights from (Stage 0 report, section 6).
    fn resolve_weights_space(self) -> MeasuredSpace {
        match self {
            MeasuredSpace::Rec709 | MeasuredSpace::P3D65 | MeasuredSpace::P3Dci => MeasuredSpace::Rec709,
            MeasuredSpace::Rec2020 | MeasuredSpace::Rec2100Pq | MeasuredSpace::Rec2100Hlg => MeasuredSpace::Rec2020,
        }
    }
}

/// The Y row of the RGB-to-XYZ matrix for these primaries and white point: how much luminance each primary adds
/// at full strength when the three together make the white point at Y = 1.
fn y_row([r, g, b, w]: Chromaticities) -> [f64; 3] {
    let xyz = |[x, y]: [f64; 2]| [x / y, 1.0, (1.0 - x - y) / y];
    let (r, g, b, w) = (xyz(r), xyz(g), xyz(b), xyz(w));
    // Solve [r g b] · s = w for each primary's scale s (Cramer's rule); each primary's Y is then its s.
    let det = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| {
        a[0] * (b[1] * c[2] - b[2] * c[1]) - b[0] * (a[1] * c[2] - a[2] * c[1]) + c[0] * (a[1] * b[2] - a[2] * b[1])
    };
    let d = det(r, g, b);
    [det(w, g, b) / d, det(r, w, b) / d, det(r, g, w) / d]
}

/// What the waveform measures in, and how: everything its label states. Until the Colour Setup is known, the
/// Measured Space (Rec.709) and the Levels (video, Resolve's default) are assumed, and the label says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Measurement {
    pub space: MeasuredSpace,
    /// The space is an assumption, not known from the Colour Setup.
    pub assumed: bool,
    pub luma: LumaWeights,
    /// How values map to the 10-bit scale.
    pub levels: Levels,
    /// Where the Levels came from; `None` when they are assumed.
    pub levels_from: Option<Provenance>,
}

impl Default for Measurement {
    fn default() -> Measurement {
        Measurement { space: MeasuredSpace::Rec709, assumed: true, luma: DEFAULT_LUMA_WEIGHTS, levels: Levels::Video, levels_from: None }
    }
}

impl Measurement {
    /// What to measure in for this Colour Setup (`None` when not known): its Output Colour Space and its Levels,
    /// each assumed when the Colour Setup does not say.
    pub fn for_colour_setup(known: Option<KnownColourSetup>, luma: LumaWeights) -> Measurement {
        let known = known.as_ref();
        let space = known.and_then(|c| c.setup.measured_space());
        let levels = known.and_then(|c| c.setup.levels.map(|levels| (levels, c.provenance)));
        Measurement {
            space: space.unwrap_or(MeasuredSpace::Rec709),
            assumed: space.is_none(),
            luma,
            levels: levels.map_or(Levels::Video, |(levels, _)| levels),
            levels_from: levels.map(|(_, from)| from),
        }
    }

    /// Luma weights for red, green and blue.
    pub fn weights(&self) -> [f64; 3] {
        match self.luma {
            LumaWeights::FromPrimaries => y_row(self.space.chromaticities()),
            LumaWeights::MatchResolve => y_row(self.space.resolve_weights_space().chromaticities()),
        }
    }

    /// The part of a label that says what Luma is: the Measured Space and the weights in use. The waveform's and
    /// the black-and-white Panel's labels share it, so the two always name the same weights.
    pub fn weights_label(&self) -> String {
        let [r, g, b] = self.weights();
        let name = self.space.name();
        // Assumed means the picture is not converted into a known Output Colour Space: the values are measured as
        // sent, like the Picture View's "as sent, no Output Conversion", and must not read as a Rec.709 measurement.
        let space = if self.assumed { format!("{name} assumed, measured as sent, unconverted") } else { name.to_string() };
        format!("{space}  ·  Y = {r:.4} R + {g:.4} G + {b:.4} B {}", self.weights_how())
    }

    /// How the weights were chosen, for a label: "from primaries" or "to match Resolve".
    pub fn weights_how(&self) -> &'static str {
        match self.luma {
            LumaWeights::FromPrimaries => "from primaries",
            LumaWeights::MatchResolve => "to match Resolve",
        }
    }

    /// The waveform's quiet label: the scale, the Measured Space and the luma weights in use.
    pub fn label(&self) -> String {
        self.label_titled("Luma")
    }

    /// The same label under another title (the Colour Waveform's).
    pub fn label_titled(&self, title: &str) -> String {
        self.label_for(title, true)
    }

    /// The label under `title`, without the luma weights when Luma is not drawn.
    pub fn label_for(&self, title: &str, with_luma: bool) -> String {
        let range = self.levels.range_label();
        let scale = match self.levels_from {
            None => format!("{range} assumed"),
            Some(Provenance::Project) => range.to_string(),
            Some(Provenance::Manual) => format!("{range} set manually"),
        };
        if with_luma {
            format!("{title}  ·  10-bit {scale}  ·  {}", self.weights_label())
        } else {
            format!("{title}  ·  10-bit {scale}")
        }
    }
}

/// The key of the luma weights setting (kept on the Luma waveform's Panel).
pub const LUMA_SETTING: &str = "luma";

/// The luma weights the Luma waveform's `luma` setting chooses. Every display that shows Luma follows it, so a
/// grey you see lines up with its trace on the waveform.
pub fn luma_weights_of(settings: &Settings) -> LumaWeights {
    match settings.get(LUMA_SETTING).map(String::as_str) {
        Some("match-resolve") => LumaWeights::MatchResolve,
        _ => LumaWeights::FromPrimaries,
    }
}
