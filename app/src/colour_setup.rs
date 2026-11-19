//! The Colour Setup: what the app knows about the project's colour pipeline, where it came from, and the quiet
//! label that says so (ADR 0005, ADR 0006).
//!
//! It arrives by two roads. The project's Colour Setup is read from Resolve's own setting keys by the scripting
//! helper ([`ColourSetup::from_resolve_settings`]). A Colour Setup set by hand on the Sender node travels in the
//! Feed, in the protocol's `key=value` vocabulary ([`ColourSetup::from_feed`]). The manual one wins.
pub use scopecreep_feed::Provenance;
use crate::measurement::MeasuredSpace;
use scopecreep_feed::SentColourSetup;

/// How Resolve manages colour in the project.
#[derive(Clone, Debug, PartialEq)]
pub enum Science {
    /// DaVinci YRGB: Timeline and Output Colour Space are tags only, nothing is converted.
    Yrgb,
    /// DaVinci YRGB Color Managed.
    ColourManaged,
    /// ACES (ACEScct or ACEScc).
    Aces,
    /// A mode this app does not know, as Resolve names it.
    Other(String),
}

/// Whether code values use the legal range (video levels) or the full range (data levels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Levels {
    Video,
    Full,
}

/// A Timeline Colour Space, parsed once from the name Resolve (or the Sender node) gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimelineSpace {
    /// DaVinci Wide Gamut / DaVinci Intermediate (either of the names Resolve uses).
    DaVinciWideGamut,
    Acescct,
    Acescc,
    Rec709Gamma24,
    /// Any other name, as given.
    Other(String),
}

impl TimelineSpace {
    pub fn parse(name: &str) -> TimelineSpace {
        match name {
            "DaVinci WG/Intermediate" | "DaVinci Wide Gamut / DaVinci Intermediate" => TimelineSpace::DaVinciWideGamut,
            "ACEScct" => TimelineSpace::Acescct,
            "ACEScc" => TimelineSpace::Acescc,
            "Rec.709 Gamma 2.4" => TimelineSpace::Rec709Gamma24,
            other => TimelineSpace::Other(other.to_string()),
        }
    }

    pub fn is_aces(&self) -> bool {
        match self {
            TimelineSpace::Acescct | TimelineSpace::Acescc => true,
            TimelineSpace::Other(name) => name.starts_with("ACES"),
            _ => false,
        }
    }
}

/// An Output Colour Space (for ACES: the output transform), parsed once from Resolve's name for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputSpace {
    Rec709Gamma24,
    Rec709Bt1886,
    /// Resolve's "P3-D65", which is gamma 2.6.
    P3D65,
    P3Dci,
    Srgb,
    Rec2020Gamma24,
    /// "Rec.2100 ST2084".
    Rec2100Pq,
    /// "Rec.2100 ST2084 (1000 nit)", the ACES output transform.
    Rec2100Pq1000,
    Rec2100Hlg,
    /// Any other name, as given.
    Other(String),
}

impl OutputSpace {
    pub fn parse(name: &str) -> OutputSpace {
        match name {
            "Rec.709 Gamma 2.4" => OutputSpace::Rec709Gamma24,
            "Rec.709 BT.1886" => OutputSpace::Rec709Bt1886,
            "P3-D65" => OutputSpace::P3D65,
            "P3-DCI" => OutputSpace::P3Dci,
            "sRGB" => OutputSpace::Srgb,
            "Rec.2020 Gamma 2.4" => OutputSpace::Rec2020Gamma24,
            "Rec.2100 ST2084" => OutputSpace::Rec2100Pq,
            "Rec.2100 ST2084 (1000 nit)" => OutputSpace::Rec2100Pq1000,
            "Rec.2100 HLG" => OutputSpace::Rec2100Hlg,
            other => OutputSpace::Other(other.to_string()),
        }
    }

    /// The Measured Space by its primaries and encoding. `None` when the name is not one the Scopes know (the
    /// waveform then keeps its labelled Rec.709 assumption). Names Resolve builds from a colour space and a
    /// separate gamma ("Rec.709 / Gamma 2.4") are matched by their start.
    pub fn measured_space(&self) -> Option<MeasuredSpace> {
        Some(match self {
            OutputSpace::Rec709Gamma24 | OutputSpace::Rec709Bt1886 | OutputSpace::Srgb => MeasuredSpace::Rec709,
            OutputSpace::P3D65 => MeasuredSpace::P3D65,
            OutputSpace::P3Dci => MeasuredSpace::P3Dci,
            OutputSpace::Rec2020Gamma24 => MeasuredSpace::Rec2020,
            OutputSpace::Rec2100Pq | OutputSpace::Rec2100Pq1000 => MeasuredSpace::Rec2100Pq,
            OutputSpace::Rec2100Hlg => MeasuredSpace::Rec2100Hlg,
            OutputSpace::Other(o) => {
                let o = o.as_str();
                if o.starts_with("Rec.709") {
                    MeasuredSpace::Rec709
                } else if o.starts_with("P3-D65") {
                    MeasuredSpace::P3D65
                } else if o.starts_with("P3-DCI") {
                    MeasuredSpace::P3Dci
                } else if o.starts_with("Rec.2020") {
                    MeasuredSpace::Rec2020
                } else if o.starts_with("Rec.2100 ST2084") || o.starts_with("Rec.2100 PQ") {
                    MeasuredSpace::Rec2100Pq
                } else if o.starts_with("Rec.2100 HLG") {
                    MeasuredSpace::Rec2100Hlg
                } else {
                    return None;
                }
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColourSetup {
    /// `None` when not known (a manual override does not say).
    pub science: Option<Science>,
    /// Timeline Colour Space, as Resolve names it (for ACES: `ACEScct` or `ACEScc`).
    pub timeline: String,
    /// Output Colour Space, as Resolve names it (for ACES: the output transform, e.g. `Rec.709 BT.1886`).
    pub output: String,
    /// `timeline` and `output` parsed once, for the app's decisions; the strings are for display.
    pub timeline_space: TimelineSpace,
    pub output_space: OutputSpace,
    /// Output tone mapping, `None` when off or not known.
    pub tone_mapping: Option<String>,
    /// Output gamut mapping, `None` when off or not known.
    pub gamut_mapping: Option<String>,
    /// ACES version when known. Resolve 21.2 does not tell scripts; an ACES setup without it assumes 2.0.
    pub aces_version: Option<String>,
    pub levels: Option<Levels>,
}

/// The ACES version assumed when it is not known (Resolve's default, Stage 0).
pub const ASSUMED_ACES_VERSION: &str = "2.0";

fn mapping(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|v| !v.is_empty() && *v != "None").map(str::to_string)
}

fn levels(value: Option<&str>) -> Option<Levels> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "video" => Some(Levels::Video),
        "full" | "data" => Some(Levels::Full),
        _ => None,
    }
}

impl ColourSetup {
    pub fn new(
        science: Option<Science>,
        timeline: String,
        output: String,
        tone_mapping: Option<String>,
        gamut_mapping: Option<String>,
        aces_version: Option<String>,
        levels: Option<Levels>,
    ) -> ColourSetup {
        ColourSetup {
            science,
            timeline_space: TimelineSpace::parse(&timeline),
            output_space: OutputSpace::parse(&output),
            timeline,
            output,
            tone_mapping,
            gamut_mapping,
            aces_version,
            levels,
        }
    }

    /// From Resolve's own setting keys and values (`Timeline.GetSetting()`, keys in #6's closing comment).
    /// `None` when there is no colour science mode to go on.
    pub fn from_resolve_settings(settings: &[(String, String)]) -> Option<ColourSetup> {
        let get = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        let mode = get("colorScienceMode")?.trim();
        // A colour space and its gamma are separate settings when "separateColorSpaceAndGamma" is on.
        let space = |name: &str| {
            let s = get(name).unwrap_or("").trim().to_string();
            match get(&format!("{name}Gamma")).map(str::trim) {
                Some(g) if get("separateColorSpaceAndGamma") == Some("1") && !g.is_empty() => format!("{s} / {g}"),
                _ => s,
            }
        };
        let (science, timeline, output) = match mode {
            // In ACES modes Resolve still reports the YRGB colour space keys, with values that mean nothing.
            "acescct" | "acescc" => {
                let timeline = if mode == "acescct" { "ACEScct" } else { "ACEScc" };
                (Science::Aces, timeline.to_string(), get("colorAcesODT").unwrap_or("").trim().to_string())
            }
            "davinciYRGB" => (Science::Yrgb, space("colorSpaceTimeline"), space("colorSpaceOutput")),
            m if m.starts_with("davinciYRGBColorManaged") => (Science::ColourManaged, space("colorSpaceTimeline"), space("colorSpaceOutput")),
            m => (Science::Other(m.to_string()), space("colorSpaceTimeline"), space("colorSpaceOutput")),
        };
        let aces = science == Science::Aces;
        Some(ColourSetup::new(
            Some(science),
            timeline,
            output,
            if aces { None } else { mapping(get("colorSpaceOutputToneMapping")) },
            if aces { None } else { mapping(get("colorSpaceOutputGamutMapping")) },
            if aces { mapping(get("acesVersion")) } else { None },
            levels(get("videoDataLevels")),
        ))
    }

    /// From a Feed slot's Colour Setup block (protocol 1.1 vocabulary). `None` without a Timeline and Output
    /// Colour Space.
    pub fn from_feed(sent: &SentColourSetup) -> Option<ColourSetup> {
        let timeline = sent.get("timeline").filter(|v| !v.is_empty())?.to_string();
        let output = sent.get("output").filter(|v| !v.is_empty())?.to_string();
        let science = match sent.get("science") {
            Some("yrgb") => Some(Science::Yrgb),
            Some("rcm") => Some(Science::ColourManaged),
            Some("aces") => Some(Science::Aces),
            Some(other) if !other.is_empty() => Some(Science::Other(other.to_string())),
            _ if timeline.starts_with("ACES") => Some(Science::Aces),
            _ => None,
        };
        Some(ColourSetup::new(
            science,
            timeline,
            output,
            mapping(sent.get("tone_mapping")),
            mapping(sent.get("gamut_mapping")),
            mapping(sent.get("aces_version")),
            levels(sent.get("levels")),
        ))
    }

    pub fn is_aces(&self) -> bool {
        self.science == Some(Science::Aces) || self.timeline_space.is_aces()
    }

    /// The ACES version the Scopes should use: the known one, else the assumed 2.0. `None` when not ACES.
    pub fn effective_aces_version(&self) -> Option<&str> {
        self.is_aces().then(|| self.aces_version.as_deref().unwrap_or(ASSUMED_ACES_VERSION))
    }

    /// The Measured Space by default: the Output Colour Space, by its primaries and encoding. `None` when the name
    /// is not one the Scopes know (the waveform then keeps its labelled Rec.709 assumption).
    pub fn measured_space(&self) -> Option<MeasuredSpace> {
        self.output_space.measured_space()
    }

    /// Output Colour Space as the label shows it. Resolve's "P3-D65" is gamma 2.6 (Stage 0, check 4), which its
    /// name does not say.
    pub fn output_label(&self) -> String {
        if self.output_space == OutputSpace::P3D65 { "P3-D65 (gamma 2.6)".to_string() } else { self.output.clone() }
    }
}

/// A Colour Setup and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct KnownColourSetup {
    pub setup: ColourSetup,
    pub provenance: Provenance,
}

impl KnownColourSetup {
    /// The quiet label, e.g. "ACEScct → Rec.709 BT.1886 (from project, assumes ACES 2.0)".
    /// Video levels are the norm and not mentioned; full levels are.
    pub fn label(&self) -> String {
        let s = &self.setup;
        let mut text = format!("{} → {}", s.timeline, s.output_label());
        if let Some(t) = &s.tone_mapping {
            text += &format!(", {t} tone mapping");
        }
        if let Some(g) = &s.gamut_mapping {
            text += &format!(", {g} gamut mapping");
        }
        if s.levels == Some(Levels::Full) {
            text += ", full levels";
        }
        let mut notes = vec![match self.provenance {
            Provenance::Project => "from project".to_string(),
            Provenance::Manual => "set manually".to_string(),
        }];
        if s.is_aces() {
            notes.push(match &s.aces_version {
                Some(v) => format!("ACES {v}"),
                None => format!("assumes ACES {ASSUMED_ACES_VERSION}"),
            });
        }
        format!("{text} ({})", notes.join(", "))
    }
}
