//! Stops False Colour: a Picture View that paints each pixel by how many stops it is above or below middle grey,
//! measured in scene light (ADR 0009, ADR 0010).
//!
//! The Feed's Timeline Colour Space frames (after the grade) are decoded to linear light with that space's own
//! curve; luma is the Y of the space's primaries; middle grey is 0.18 linear in every scene-referred space, so
//! stops = log2(Y / 0.18). The view ignores the Measured Space switch: it always needs scene light. A display-
//! referred Timeline Colour Space (Rec.709 Gamma 2.4) has no scene light, so its display curve is inverted and the
//! label says it is approximated. A Timeline Colour Space with no curve here (or no Colour Setup) is never guessed
//! and never drawn black (ADR 0007, ADR 0019): the Output picture is measured instead, decoded through the Output
//! Colour Space's display curve (Rec.709 Gamma 2.4 assumed when the Colour Setup says nothing), and the label says
//! "approximated: display-referred" (and "assumed"). Only when even that curve cannot be inverted does the Panel
//! say so in a message, over a neutral background, and draw no picture.
//!
//! Nothing is measured ahead of drawing: the Panel keeps the latest Timeline Colour Space frame on the GPU and
//! decodes it as it draws, so the Colour Setup and the settings always apply to the frame on screen.
//! [`Display::read_back`] runs the same decoding into a stops map, for tests.
//!
//! Two modes: full zones (17 zones centred on the stop marks -6 to +6, with half-stop zones beside middle grey,
//! black beyond -6 1/2 and white beyond +6 1/2) and grey and +1 stop (a black and white picture, green within a
//! sixth of a stop of grey, pink within a sixth of a stop of one stop over: the ColorChecker routine). The legend
//! is a horizontal bar of swatches under the picture, with the stop numbers under them and captions over them.
use crate::colour_setup::{OutputSpace, TimelineSpace};
use crate::output_conversion::Plan;
use crate::display::{Display, DisplayContext, ScaleText};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{Legend, PictureStage, Rule};
use crate::settings::{Choice, SettingDef, Settings};
use crate::components::ChipMode;
use crate::toolbar::{BW_DEFAULT_GREY, BW_SETTING, BW_TOOLBAR, ToolbarChip, ToolbarDef};
use scopecreep_accuracy::colorchecker::luma_weights_adapted_to_d65;
use scopecreep_accuracy::colour::{AP1, DWG, P3_D65, REC2020, REC709};
use std::sync::{Arc, Mutex};

/// The display's name, which its settings are kept under.
pub const NAME: &str = "Stops False Colour";
pub const MODE_SETTING: &str = "mode";
pub const LEGEND_SETTING: &str = "legend";

/// Middle grey, linear.
pub const MIDDLE_GREY: f64 = 0.18;

/// The edges between the 17 zones, in stops from middle grey: the midpoints between neighbouring stop marks.
/// Black is below the first and white above the last.
pub const ZONE_EDGES: [f32; 16] = [-6.5, -5.5, -4.5, -3.5, -2.5, -1.5, -0.75, -0.25, 0.25, 0.75, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5];

/// The stop marks the 15 named zones go by, from the darkest (zone 1; zone 0 is black, zone 16 white).
pub const ZONE_NAMES: [&str; 15] = ["-6", "-5", "-4", "-3", "-2", "-1", "-1/2", "18 %", "+1/2", "+1", "+2", "+3", "+4", "+5", "+6"];

/// The colour of each zone, from black (more than 6 1/2 stops under) to white (more than 6 1/2 over). Cool for
/// under, plain grey for middle grey, warm for over; every zone its own colour. scopecreep's own scale (ADR 0010).
pub const ZONE_COLOURS: [[f32; 3]; 17] = [
    [0.00, 0.00, 0.00], // black
    [0.36, 0.08, 0.58], // purple
    [0.66, 0.56, 0.86], // lilac
    [0.12, 0.22, 0.84], // blue
    [0.32, 0.74, 0.96], // sky blue
    [0.04, 0.58, 0.24], // green
    [0.36, 0.78, 0.28], // mid green
    [0.68, 0.92, 0.42], // light green
    [0.50, 0.50, 0.50], // grey: middle grey
    [0.98, 0.88, 0.12], // yellow
    [1.00, 0.96, 0.62], // pale yellow
    [0.96, 0.52, 0.06], // orange
    [0.99, 0.72, 0.40], // light orange
    [0.84, 0.08, 0.08], // red
    [0.95, 0.40, 0.62], // pink
    [0.98, 0.76, 0.84], // light pink
    [1.00, 1.00, 1.00], // white
];

/// The zone a number of stops falls in (0 black to 16 white), as the shader works it out.
pub fn zone_of(stops: f32) -> usize {
    ZONE_EDGES.iter().filter(|e| stops >= **e).count()
}

/// The two colours of the grey and +1 stop mode, and how close (in stops) a pixel must be.
pub const GREY_GREEN: [f32; 3] = [0.0, 0.85, 0.25];
pub const PLUS_ONE_PINK: [f32; 3] = [1.0, 0.40, 0.70];
pub const CHART_TOLERANCE: f32 = 1.0 / 6.0;

/// How a Timeline Colour Space is decoded to linear light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    AcesCct,
    AcesCc,
    AcesCg,
    DaVinciIntermediate,
    /// Rec.709 Gamma 2.4: a display curve, inverted. Not scene light.
    Gamma24,
    /// A pure power of 2.6 (P3), inverted. Not scene light.
    Gamma26,
}

impl Curve {
    /// The curve for a Timeline Colour Space, `None` for one this view cannot decode.
    pub fn for_space(space: &TimelineSpace) -> Option<Curve> {
        Some(match space {
            TimelineSpace::Acescct => Curve::AcesCct,
            TimelineSpace::Acescc => Curve::AcesCc,
            TimelineSpace::DaVinciWideGamut => Curve::DaVinciIntermediate,
            TimelineSpace::Rec709Gamma24 => Curve::Gamma24,
            TimelineSpace::Other(name) if name == "ACEScg" => Curve::AcesCg,
            TimelineSpace::Other(_) => return None,
        })
    }

    fn code(self) -> u32 {
        match self {
            Curve::AcesCct => 0,
            Curve::AcesCc => 1,
            Curve::DaVinciIntermediate => 2,
            Curve::Gamma24 => 3,
            Curve::AcesCg => 4,
            Curve::Gamma26 => 5,
        }
    }

    /// The Y of the space's primaries, as weights for linear RGB, for the colour as seen on a D65 display. A D65
    /// space's weights are the usual luma weights; ACES's white is not D65, so its weights are adapted to it
    /// (neutrals are unchanged).
    pub fn luma_weights(self) -> [f64; 3] {
        match self {
            Curve::AcesCct | Curve::AcesCc | Curve::AcesCg => luma_weights_adapted_to_d65(&AP1),
            Curve::DaVinciIntermediate => luma_weights_adapted_to_d65(&DWG),
            Curve::Gamma24 => luma_weights_adapted_to_d65(&REC709),
            Curve::Gamma26 => luma_weights_adapted_to_d65(&P3_D65),
        }
    }

    /// Whether the stops are only approximate: a display-referred space has no scene light to decode.
    pub fn approximated(self) -> bool {
        matches!(self, Curve::Gamma24 | Curve::Gamma26)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Zones,
    GreyAndPlusOne,
}

pub static SETTINGS: [SettingDef; 3] = [
    SettingDef {
        key: MODE_SETTING,
        label: "Mode",
        choices: &[Choice { value: "zones", text: "Full zones" }, Choice { value: "grey-plus-one", text: "Grey and +1 stop" }],
    },
    SettingDef { key: LEGEND_SETTING, label: "Legend", choices: &[Choice { value: "on", text: "Legend on" }, Choice { value: "off", text: "Legend off" }] },
    BW_DEFAULT_GREY,
];

/// The mode and B/W are chips on the title bar (#126, #118): the mode one at a time (the chart routine is one
/// click), B/W only in the grey and +1 stop mode, because full zones paint every pixel.
pub static TOOLBAR: [ToolbarDef; 2] = [
    ToolbarDef {
        key: MODE_SETTING,
        mode: ChipMode::Single,
        chips: &[
            ToolbarChip { value: "zones", text: "Zones", tooltip: "Full zones: every pixel painted by its stop", channel: None },
            ToolbarChip { value: "grey-plus-one", text: "Grey +1", tooltip: "Grey and +1 stop: 18 % grey green, one stop over pink", channel: None },
        ],
    },
    BW_TOOLBAR[0],
];

/// The chips under `settings`: the mode, and B/W only in grey and +1 stop mode.
pub fn toolbar_for(settings: &Settings) -> &'static [ToolbarDef] {
    match settings.get(MODE_SETTING).map(String::as_str) {
        Some("grey-plus-one") => &TOOLBAR,
        _ => &TOOLBAR[..1],
    }
}

/// What a Stops False Colour read back from the GPU: the stops of every pixel (log2 of Y over middle grey).
pub struct StopsMap {
    pub width: u32,
    pub height: u32,
    pub bottom_up: bool,
    /// In the frame's own row order.
    pub stops: Vec<f32>,
}

impl StopsMap {
    /// The stops of the pixel at `x` across and `y` down from the top of the picture.
    pub fn at(&self, x: u32, y: u32) -> f32 {
        let row = if self.bottom_up { self.height - 1 - y } else { y };
        self.stops[(row * self.width + x) as usize]
    }
}

/// Logical pixels: the legend under the picture is a caption line, the swatches and a line of numbers.
const SWATCH_ROW: f32 = 16.0;
const GAP: f32 = 3.0;
/// The numbers' size, as the waveform's, and a rough width of one of their characters.
const TEXT_LINE: f32 = crate::waveform::SCALE_SIZE * 1.3;
const CHAR_WIDTH: f32 = crate::waveform::SCALE_SIZE * 0.6;

/// The zones the ColorChecker's dark skin (-0.89 stops) and light skin (+0.89) patches fall in, where the two skin
/// captions go (research table; a test checks the zones).
pub const DARK_SKIN_ZONE: usize = 6;
pub const LIGHT_SKIN_ZONE: usize = 10;

/// Where the legend goes in a Panel (the picture is the Picture stage's).
struct Geometry {
    /// The strip the legend takes under the picture, and its bar of swatches in it (none: no legend).
    legend: Option<Legend>,
    /// Left and width of the bar.
    bar_left: f32,
    bar_width: f32,
    swatches: usize,
    /// The middle of the caption line and of the numbers' line.
    caption_y: f32,
    number_y: f32,
    /// Whether there is room for text under and over the swatches.
    texts: bool,
}

impl Geometry {
    fn new(area: [f32; 4], scale: f32, mode: Mode, legend_on: bool) -> Geometry {
        let [left, top, width, height] = area;
        let swatches = if mode == Mode::Zones { ZONE_COLOURS.len() } else { 2 };
        let (swatch, line, gap) = (SWATCH_ROW * scale, TEXT_LINE * scale, GAP * scale);
        let full = 2.0 * line + swatch + 2.0 * gap;
        let (strip, texts) = if !legend_on {
            (0.0, false)
        } else if height >= 2.5 * full {
            (full, true)
        } else if height >= 4.0 * swatch {
            (swatch, false)
        } else {
            (0.0, false)
        };
        let bottom = top + height;
        let swatch_top = if texts { bottom - strip + line + gap } else { bottom - strip };
        let legend = (strip > 0.0).then_some(Legend { strip, bar_offset: swatch_top - (bottom - strip), bar: swatch });
        Geometry { legend, bar_left: left, bar_width: if strip > 0.0 { width } else { 0.0 }, swatches, caption_y: bottom - strip + line / 2.0, number_y: swatch_top + swatch + gap + line / 2.0, texts }
    }

    /// The captions and numbers of the legend: each number centred under its own swatch, captions over the zones
    /// they name. Text is dropped, never overlapped: every second number when they are too wide for their
    /// swatches, then all of them; a caption that would touch another or leave the bar.
    fn legend_texts(&self, scale: f32, mode: Mode) -> Vec<ScaleText> {
        if !self.texts || self.bar_width <= 0.0 {
            return Vec::new();
        }
        let (left, width) = (self.bar_left, self.bar_width);
        let cell = width / self.swatches as f32;
        let char_width = CHAR_WIDTH * scale;
        let text_width = |text: &str| text.chars().count() as f32 * char_width;
        let under = |text: &str, i: usize| ScaleText { text: text.to_string(), left: left + i as f32 * cell, top: self.number_y, width: cell, right_aligned: false, centred: true };
        if mode == Mode::GreyAndPlusOne {
            return if text_width("18 % grey") + 2.0 * char_width <= cell { vec![under("18 % grey", 0), under("+1", 1)] } else { Vec::new() };
        }
        let mut texts = Vec::new();
        // Zone 8 is middle grey: the numbers of every second zone from it are the ones that stay.
        let widest = ZONE_NAMES.iter().map(|n| text_width(n)).fold(0.0, f32::max);
        let step: i32 = if widest + char_width <= cell {
            1
        } else if widest + char_width <= 2.0 * cell {
            2
        } else {
            0
        };
        if step > 0 {
            for (n, name) in ZONE_NAMES.iter().enumerate() {
                let zone = n + 1;
                if (zone as i32 - 8).rem_euclid(step) == 0 {
                    texts.push(under(name, zone));
                }
            }
        }
        // Captions, most important first; each is kept when it clears the ones kept before it.
        let wanted: [(&str, f32, i8); 4] = [
            ("Underexposed", left, -1),
            ("Overexposed", left + width, 1),
            ("Dark skin", left + (DARK_SKIN_ZONE as f32 + 0.5) * cell, 0),
            ("Light skin", left + (LIGHT_SKIN_ZONE as f32 + 0.5) * cell, 0),
        ];
        let mut kept: Vec<(f32, f32)> = Vec::new();
        for (text, at, side) in wanted {
            let w = text_width(text) + char_width;
            let x0 = match side {
                -1 => at,
                1 => at - w,
                _ => at - w / 2.0,
            };
            if x0 < left - 0.5 || x0 + w > left + width + 0.5 || kept.iter().any(|(a, b)| x0 < b + char_width && x0 + w > a - char_width) {
                continue;
            }
            kept.push((x0, x0 + w));
            texts.push(ScaleText { text: text.to_string(), left: x0, top: self.caption_y, width: w, right_aligned: false, centred: true });
        }
        texts
    }
}

fn wgsl_vec3(c: [f32; 3]) -> String {
    format!("vec3<f32>({:.4}, {:.4}, {:.4})", c[0], c[1], c[2])
}

/// The decoding and the zones, shared by drawing and by the read-back.
fn common_shader() -> String {
    let edges = ZONE_EDGES.iter().map(|e| format!("{e:.3}")).collect::<Vec<_>>().join(", ");
    let colours = ZONE_COLOURS.iter().map(|c| wgsl_vec3(*c)).collect::<Vec<_>>().join(", ");
    format!(
        r#"
struct Values {{
    weights: vec4<f32>,     // the Y weights of the Timeline Colour Space's primaries
    info: vec4<u32>,        // curve, picture width, picture height, mode (0 full zones, 1 grey and +1 stop)
    flags: vec4<f32>,       // x: swatches in the legend; y: 1 to fill the Panel with the neutral background; z: 1 for the unpainted picture in grey, 0 in colour
}};

var<private> EDGES: array<f32, 16> = array<f32, 16>({edges});
var<private> COLOURS: array<vec3<f32>, 17> = array<vec3<f32>, 17>({colours});
const GREEN: vec3<f32> = {green};
const PINK: vec3<f32> = {pink};
const TOLERANCE: f32 = {tolerance:.6};
const MIDDLE_GREY: f32 = {middle_grey};

fn decode(v: f32, curve: u32) -> f32 {{
    switch curve {{
        case 0u: {{
            if v <= 0.155251141552511 {{ return (v - 0.0729055341958355) / 10.5402377416545; }}
            return exp2(v * 17.52 - 9.72);
        }}
        case 1u: {{
            if v < -0.3013698630 {{ return (exp2(v * 17.52 - 9.72) - exp2(-16.0)) * 2.0; }}
            return exp2(min(v, 1.4679) * 17.52 - 9.72);
        }}
        case 2u: {{
            if v > 0.02740668 {{ return exp2(v / 0.07329248 - 7.0) - 0.0075; }}
            return v / 10.44426855;
        }}
        case 3u: {{ return sign(v) * pow(abs(v), 2.4); }}
        case 5u: {{ return sign(v) * pow(abs(v), 2.6); }}
        default: {{ return v; }}
    }}
}}

// Linear Y of a Timeline Colour Space pixel.
fn luma(texel: vec4<f32>) -> f32 {{
    let c = values.info.x;
    return dot(values.weights.xyz, vec3<f32>(decode(texel.r, c), decode(texel.g, c), decode(texel.b, c)));
}}

fn stops_of(y: f32) -> f32 {{
    return log2(max(y, 1e-10) / MIDDLE_GREY);
}}
"#,
        green = wgsl_vec3(GREY_GREEN),
        pink = wgsl_vec3(PLUS_ONE_PINK),
        tolerance = CHART_TOLERANCE,
        middle_grey = MIDDLE_GREY,
    )
}

fn measure_shader() -> String {
    common_shader()
        + r#"
@group(0) @binding(0) var<uniform> values: Values;
@group(0) @binding(1) var frame: texture_2d<f32>;
@group(0) @binding(2) var stops_out: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn measure(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= values.info.y || id.y >= values.info.z { return; }
    let texel = textureLoad(frame, vec2<i32>(id.xy), 0);
    textureStore(stops_out, vec2<i32>(id.xy), vec4<f32>(stops_of(luma(texel)), 0.0, 0.0, 1.0));
}
"#
}

/// The colour rule: the zones (or the grey and +1 stop highlight) of the pixel's stops; the legend is the swatches.
fn draw_shader() -> String {
    common_shader()
        + r#"
fn zone_colour(stops: f32) -> vec3<f32> {
    var zone = 0u;
    for (var i = 0u; i < 16u; i++) {
        if stops >= EDGES[i] { zone = i + 1u; }
    }
    return COLOURS[zone];
}

fn legend(uv: vec2<f32>) -> vec3<f32> {
    let count = u32(values.flags.x);
    let swatch = min(u32(uv.x * f32(count)), count - 1u);
    if values.info.w == 1u {
        return select(GREEN, PINK, swatch == 1u);
    }
    return COLOURS[swatch];
}

fn colour(uv: vec2<f32>) -> vec3<f32> {
    if values.flags.y > 0.5 { return vec3<f32>(0.09); }
    let rgb = load_picture(uv);
    let y_linear = luma(vec4<f32>(rgb, 1.0));
    let stops = stops_of(y_linear);
    if values.info.w == 1u {
        if abs(stops) < TOLERANCE { return GREEN; }
        if abs(stops - 1.0) < TOLERANCE { return PINK; }
        if values.flags.z > 0.5 { return vec3<f32>(pow(clamp(y_linear, 0.0, 1.0), 1.0 / 2.4)); }
        // Its own colour: each channel decoded to light and shown as the grey is, so the two differ only in colour.
        let c = values.info.x;
        let light = clamp(vec3<f32>(decode(rgb.r, c), decode(rgb.g, c), decode(rgb.b, c)), vec3<f32>(0.0), vec3<f32>(1.0));
        return pow(light, vec3<f32>(1.0 / 2.4));
    }
    return zone_colour(stops);
}
"#
}

/// The words of `Values` (and of the measuring pass's parameters): 3 vec4s.
const VALUES_WORDS: usize = 12;

/// What the display measures, and with which curve.
#[derive(Clone, Debug, PartialEq)]
enum Basis {
    /// The Timeline Colour Space frame, decoded with its own curve (scene light, or a display curve inverted).
    Timeline(Curve),
    /// The Output picture, decoded through a display curve: the Timeline Colour Space is not known or not one this
    /// view decodes. `note` says which curve and why, for the label.
    Output { curve: Curve, weights: [f64; 3], note: String },
    /// Nothing can be measured. The Panel says so in `lines`; `why` is for the label.
    Unreadable { lines: Vec<String>, why: String },
}

/// The curve, primaries and name of an Output Colour Space this view can decode as display light.
fn output_curve(space: &OutputSpace) -> Option<(Curve, [f64; 3], &'static str)> {
    let rec709 = (Curve::Gamma24, luma_weights_adapted_to_d65(&REC709), "gamma 2.4");
    Some(match space {
        OutputSpace::Rec709Gamma24 | OutputSpace::Rec709Bt1886 | OutputSpace::Srgb => rec709,
        OutputSpace::P3D65 | OutputSpace::P3Dci => (Curve::Gamma26, luma_weights_adapted_to_d65(&P3_D65), "gamma 2.6"),
        OutputSpace::Rec2020Gamma24 => (Curve::Gamma24, luma_weights_adapted_to_d65(&REC2020), "gamma 2.4"),
        OutputSpace::Other(name) if name.starts_with("Rec.709") => rec709,
        _ => return None,
    })
}

impl Basis {
    fn for_setup(known: Option<&crate::colour_setup::KnownColourSetup>) -> Basis {
        let setup = known.map(|k| &k.setup);
        if let Some(curve) = setup.and_then(|s| Curve::for_space(&s.timeline_space)) {
            return Basis::Timeline(curve);
        }
        let reason = match setup {
            None => "the Timeline Colour Space is not known".to_string(),
            Some(s) => format!("this view cannot decode {}", s.timeline),
        };
        // Only a converted picture is in the Output Colour Space; a Feed shown as sent is not, so Rec.709 is assumed.
        let converted = setup.filter(|s| Plan::for_setup(Some(s)).method().is_some());
        match converted {
            Some(s) => match output_curve(&s.output_space) {
                Some((curve, weights, name)) => Basis::Output { curve, weights, note: format!("the Output picture decoded as {name} ({}), because {reason}", s.output) },
                None => Basis::Unreadable {
                    lines: vec![
                        "Cannot measure this picture".to_string(),
                        format!("Neither the Timeline Colour Space ({}) nor the Output Colour Space ({}) is one it can decode.", s.timeline, s.output),
                        "Timeline Colour Spaces it can: ACEScct, ACEScc, ACEScg, DaVinci WG/Intermediate, Rec.709 Gamma 2.4.".to_string(),
                        "Change the Colour Setup in Resolve, or set it on the Sender node.".to_string(),
                    ],
                    why: format!("not measured: cannot decode {} or the Output Colour Space {}", s.timeline, s.output),
                },
            },
            None => Basis::Output {
                curve: Curve::Gamma24,
                weights: luma_weights_adapted_to_d65(&REC709),
                note: format!("the Output picture, Rec.709 Gamma 2.4 assumed (the picture is shown as sent), because {reason}"),
            },
        }
    }
}

/// The tags Stops False Colour's title bar needs (#99): its mode when not the default, "approximated" when it reads
/// a display curve, "assumed" and "unconverted" when it reads a picture shown as sent, "not measured" when it
/// cannot measure.
pub fn reading_tags(context: &DisplayContext) -> Vec<String> {
    let mut tags = Vec::new();
    if context.settings.get(MODE_SETTING).map(String::as_str) == Some("grey-plus-one") {
        tags.push("grey and +1 stop".to_string());
        tags.extend(crate::window_text::bw_tag(context, true));
    }
    match Basis::for_setup(context.colour_setup.as_ref()) {
        Basis::Timeline(curve) if curve.approximated() => tags.push("approximated".into()),
        Basis::Timeline(_) => {}
        Basis::Output { .. } => {
            tags.push("approximated".into());
            if context.colour_setup.as_ref().is_none_or(|c| crate::output_conversion::Plan::for_setup(Some(&c.setup)).method().is_none()) {
                tags.push("assumed".into());
                tags.push("unconverted".into());
            }
        }
        Basis::Unreadable { .. } => tags.push("not measured".into()),
    }
    tags
}

/// What the Colour Setup and the settings made of the display.
struct Config {
    basis: Basis,
    mode: Mode,
    legend: bool,
    /// B/W in the grey and +1 stop mode: the unpainted picture in grey, else in colour.
    grey: bool,
    /// The Colour Setup's part of the label.
    colour_label: String,
    timeline: String,
}

struct State {
    /// The Timeline Colour Space frame, and the Output picture.
    timeline: Option<GpuFrame>,
    output: Option<GpuFrame>,
}

impl State {
    /// The frame, curve and luma weights to measure with, `None` when nothing can be measured.
    fn source(&self, config: &Config) -> Option<(GpuFrame, Curve, [f64; 3])> {
        match &config.basis {
            Basis::Timeline(curve) => self.timeline.clone().map(|f| (f, *curve, curve.luma_weights())),
            Basis::Output { curve, weights, .. } => self.output.clone().map(|f| (f, *curve, *weights)),
            Basis::Unreadable { .. } => None,
        }
    }
}

/// Stops False Colour. Cheap to clone; the Feed thread hands it frames while the window thread draws.
#[derive(Clone)]
pub struct StopsFalseColour {
    device: wgpu::Device,
    queue: wgpu::Queue,
    stage: PictureStage,
    measure: wgpu::ComputePipeline,
    /// The parameters of the measuring pass.
    params: wgpu::Buffer,
    config: Arc<Mutex<Config>>,
    state: Arc<Mutex<State>>,
}

impl StopsFalseColour {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> StopsFalseColour {
        let stage = PictureStage::new::<[u32; VALUES_WORDS]>(device, queue, target, false, Rule { label: "stops false colour", wgsl: draw_shader(), has_legend: true });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("stops false colour measure"), source: wgpu::ShaderSource::Wgsl(measure_shader().into()) });
        let measure = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("stops false colour measure"),
            layout: None,
            module: &module,
            entry_point: Some("measure"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("stops false colour params"), size: (VALUES_WORDS * 4) as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let view = StopsFalseColour {
            device: device.clone(),
            queue: queue.clone(),
            stage,
            measure,
            params,
            config: Arc::new(Mutex::new(Config { basis: Basis::for_setup(None), mode: Mode::Zones, legend: true, grey: true, colour_label: String::new(), timeline: String::new() })),
            state: Arc::new(Mutex::new(State { timeline: None, output: None })),
        };
        view.configure(&DisplayContext::default());
        view
    }

    /// The values the shaders read, for `frame` decoded with `curve` and `weights`; `swatches` in the legend, and
    /// `background` fills the Panel with the neutral background instead of a picture.
    fn values(config: &Config, frame: &GpuFrame, curve: Curve, weights: [f64; 3], swatches: usize, background: bool) -> [u32; VALUES_WORDS] {
        let f = |v: f32| v.to_bits();
        [
            f(weights[0] as f32),
            f(weights[1] as f32),
            f(weights[2] as f32),
            0,
            curve.code(),
            frame.width,
            frame.height,
            (config.mode == Mode::GreyAndPlusOne) as u32,
            f(swatches as f32),
            f(background as u32 as f32),
            f(config.grey as u32 as f32),
            0,
        ]
    }

    /// What the display measures, for tests and diagnostics: the stops of every pixel of the latest frame, decoded
    /// now with the Colour Setup in use (waits for the GPU).
    pub fn stops(&self) -> Option<StopsMap> {
        let state = self.state.lock().unwrap();
        let config = self.config.lock().unwrap();
        let (frame, curve, weights) = state.source(&config)?;
        self.queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&Self::values(&config, &frame, curve, weights, 0, false)));
        let (width, height) = (frame.width, frame.height);
        let out = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stops map"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stops measure"),
            layout: &self.measure.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&out.create_view(&Default::default())) },
            ],
        });
        let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("stops readback"), size: (row * height) as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.measure);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        encoder.copy_texture_to_buffer(
            out.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("the GPU finishes");
        let mapped = readback.get_mapped_range(..).expect("mapped");
        let stops = (0..height as usize).flat_map(|y| bytemuck::cast_slice::<u8, f32>(&mapped[y * row as usize..y * row as usize + width as usize * 4]).to_vec()).collect();
        Some(StopsMap { width, height, bottom_up: frame.bottom_up, stops })
    }
}

impl Display for StopsFalseColour {
    fn name(&self) -> &'static str {
        NAME
    }

    /// The Output picture, for when the Timeline Colour Space cannot be decoded.
    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.state.lock().unwrap().output = Some(frame.clone());
    }

    fn on_timeline_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.state.lock().unwrap().timeline = Some(frame.clone());
    }

    fn configure(&self, context: &DisplayContext) {
        let mut config = self.config.lock().unwrap();
        config.basis = Basis::for_setup(context.colour_setup.as_ref());
        config.timeline = context.colour_setup.as_ref().map(|c| c.setup.timeline.clone()).unwrap_or_default();
        config.colour_label = context.colour_label.clone();
        config.mode = if context.settings.get(MODE_SETTING).map(String::as_str) == Some("grey-plus-one") { Mode::GreyAndPlusOne } else { Mode::Zones };
        config.grey = context.settings.get(BW_SETTING).is_none_or(|v| v == "on");
        config.legend = context.settings.get(LEGEND_SETTING).map(String::as_str) != Some("off");
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn toolbar_defs(&self) -> &'static [ToolbarDef] {
        &TOOLBAR
    }

    fn toolbar_defs_for(&self, settings: &Settings) -> &'static [ToolbarDef] {
        toolbar_for(settings)
    }

    /// What is shown and how far it can be trusted: scene light decoded from the Timeline Colour Space, the Output
    /// picture through a display curve (approximated), or the reason nothing is measured.
    fn label(&self) -> String {
        let config = self.config.lock().unwrap();
        let mode = if config.mode == Mode::Zones { "full zones" } else { "grey and +1 stop" };
        let basis = match &config.basis {
            Basis::Timeline(curve) if curve.approximated() => format!("stops from 18 % grey, approximated: {} is a display curve, inverted, not scene light", config.timeline),
            Basis::Timeline(_) => format!("stops from 18 % grey, scene light decoded from {}", config.timeline),
            Basis::Output { note, .. } => format!("stops from 18 % grey, approximated: display-referred, {note}"),
            Basis::Unreadable { why, .. } => why.clone(),
        };
        format!("{NAME}  ·  {mode}  ·  {basis}  ·  {}", config.colour_label)
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let state = self.state.lock().unwrap();
        let config = self.config.lock().unwrap();
        if area[2] < 1.0 || area[3] < 1.0 {
            return;
        }
        // The message's background needs some frame to bind; with none there is no picture at all.
        let (frame, curve, weights, background) = match state.source(&config) {
            Some((frame, curve, weights)) => (frame, curve, weights, false),
            None => match (&config.basis, state.timeline.clone().or_else(|| state.output.clone())) {
                (Basis::Unreadable { .. }, Some(frame)) => (frame, Curve::Gamma24, [1.0 / 3.0; 3], true),
                _ => return,
            },
        };
        self.stage.show_now(&frame);
        let geometry = Geometry::new(area, scale, config.mode, config.legend);
        let values = Self::values(&config, &frame, curve, weights, geometry.swatches, background);
        if background {
            self.stage.fill(pass, area, surface, &values);
        } else {
            self.stage.draw(pass, area, surface, &values, geometry.legend);
        }
    }

    /// The legend's numbers and captions, or the message when nothing can be measured.
    fn scale_texts(&self, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        // Lock the state before the config, as everywhere else.
        let state = self.state.lock().unwrap();
        let config = self.config.lock().unwrap();
        if state.timeline.is_none() && state.output.is_none() {
            return Vec::new();
        }
        if let Basis::Unreadable { lines, .. } = &config.basis {
            let line = TEXT_LINE * scale;
            let middle = area[1] + area[3] / 2.0;
            let first = middle - line * (lines.len() as f32 - 1.0) / 2.0;
            return lines.iter().enumerate().map(|(i, text)| ScaleText { text: text.clone(), left: area[0], top: first + i as f32 * line, width: area[2], right_aligned: false, centred: true }).collect();
        }
        if !config.legend {
            return Vec::new();
        }
        Geometry::new(area, scale, config.mode, true).legend_texts(scale, config.mode)
    }

    fn read_back(&self) -> Option<Box<dyn std::any::Any>> {
        self.stops().map(|m| Box::new(m) as Box<dyn std::any::Any>)
    }
}
