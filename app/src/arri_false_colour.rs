//! ARRI False Colour (spec #49, ticket #57): a Picture View that paints the Output picture by its Luma, in the
//! Rec.709 percentage bands third parties publish as ARRI's. Everything outside a band is shown as its Luma in
//! grey. Modelled, never claimed (ADR 0010); how the edges and the out-of-gamut channels are treated is ADR 0016.
//!
//! The Luma is the weighted sum of the Output picture's own channel values (the same number the luma waveform
//! traces, with the same weights from the Measured Space, ADR 0004), taken as a percentage of the full-range
//! signal. Nothing is clamped first: a channel below 0 counts as the negative number it is.
use crate::display::{Display, DisplayContext, ScaleText};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{PictureStage, Rule};
use crate::scale_bar::{BarMark, ScaleBar};
use crate::output_conversion;
use crate::settings::{Choice, SettingDef};
use crate::toolbar::{BW_DEFAULT_GREY, BW_SETTING, BW_TOOLBAR, ToolbarDef};
use std::sync::{Arc, Mutex};

/// One band of the table: a name, its edges in percent of full-range Luma, and the colour it is shown in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub name: &'static str,
    /// Lower edge, inclusive.
    pub from: f64,
    /// Upper edge, exclusive.
    pub to: f64,
    pub colour: [f32; 3],
}

/// The bands third parties publish as ARRI's Rec.709 look (SmallHD, Sigma), from the darkest up.
pub const BANDS: [Band; 6] = [
    Band { name: "purple", from: 0.0, to: 2.5, colour: [0.55, 0.22, 0.85] },
    Band { name: "blue", from: 2.5, to: 4.0, colour: [0.15, 0.40, 1.0] },
    Band { name: "green", from: 38.0, to: 42.0, colour: [0.0, 0.72, 0.22] },
    Band { name: "pink", from: 52.0, to: 56.0, colour: [1.0, 0.45, 0.72] },
    Band { name: "yellow", from: 97.0, to: 99.0, colour: [1.0, 0.90, 0.0] },
    Band { name: "red", from: 99.0, to: 100.0, colour: [0.95, 0.0, 0.0] },
];

/// The band a Luma (percent of the full-range signal) falls in, `None` where the picture is shown as grey. A band
/// includes its lower edge and excludes its upper edge. The ends are open: below 0 % is the purple band (crushed
/// past black) and 100 % and over is the red band (clipped), because those are the pixels the view exists to flag.
pub fn band_at(percent: f64) -> Option<&'static Band> {
    let last = BANDS.len() - 1;
    BANDS.iter().enumerate().find(|(i, b)| (percent >= b.from || *i == 0) && (percent < b.to || *i == last)).map(|(_, b)| b)
}

/// The settings: the legend (on by default) and B/W, a chip on the title bar (grey by default, as ARRI draws it).
pub static SETTINGS: [SettingDef; 2] = [
    SettingDef { key: "legend", label: "Legend", amount: None, choices: &[Choice { value: "on", text: "On" }, Choice { value: "off", text: "Off" }] },
    BW_DEFAULT_GREY,
];
pub static TOOLBAR: [ToolbarDef; 1] = BW_TOOLBAR;

/// The WGSL for the band lookup, from [`BANDS`] so there is one table.
fn band_function() -> String {
    let mut code = String::from("fn band(p: f32) -> vec4<f32> {\n");
    for (i, b) in BANDS.iter().enumerate() {
        let mut conditions = Vec::new();
        if i > 0 {
            conditions.push(format!("p >= {:?}", b.from as f32));
        }
        if i < BANDS.len() - 1 {
            conditions.push(format!("p < {:?}", b.to as f32));
        }
        let condition = if conditions.is_empty() { "true".to_string() } else { conditions.join(" && ") };
        code += &format!("    if ({condition}) {{ return vec4<f32>({:?}, {:?}, {:?}, 1.0); }}\n", b.colour[0], b.colour[1], b.colour[2]);
    }
    code += "    return vec4<f32>(0.0);\n}\n";
    code
}

/// The legend: a proportional ruler from 0 to 100 % of the Output Luma, ticked every 5, 10, 20, 25 or 50 % as many
/// as fit, with the meaningful points marked under it (#122). Most important mark first.
pub const BAR: ScaleBar<'static> = ScaleBar {
    from: 0.0,
    to: 100.0,
    unit: " %",
    steps: &[5.0, 10.0, 20.0, 25.0, 50.0],
    marks: &[
        BarMark { from: 2.5, to: 5.0, text: "near black clip" },
        BarMark { from: 38.0, to: 42.0, text: "18 % grey" },
        BarMark { from: 97.0, to: 99.0, text: "near white clip" },
        BarMark { from: 0.0, to: 0.0, text: "black clip" },
        BarMark { from: 100.0, to: 100.0, text: "white clip" },
    ],
};

/// The colour rule: the band the Luma falls in, else its grey. The legend is the same rule over the ruler: each
/// band at its true position and width, and the grey the picture shows in between (the picture's own colour
/// cannot be drawn on a ruler, so a Panel in colour mode shows the same greys).
fn rule() -> String {
    format!(
        r#"
struct Values {{ weights: vec4<f32> }};   // luma weights for R, G, B; w: 1 for the unpainted picture in grey, 0 in its own colour

{band}

fn legend(uv: vec2<f32>) -> vec3<f32> {{
    let percent = uv.x * 100.0;
    let coloured = band(percent);
    if (coloured.a > 0.5) {{ return coloured.rgb; }}
    return vec3<f32>(clamp(percent / 100.0, 0.0, 1.0));
}}

fn colour(uv: vec2<f32>) -> vec3<f32> {{
    let luma = dot(load_picture(uv), values.weights.xyz);
    let coloured = band(luma * 100.0);
    if (coloured.a > 0.5) {{ return coloured.rgb; }}
    if (values.weights.w > 0.5) {{ return vec3<f32>(clamp(luma, 0.0, 1.0)); }}
    return clamp(load_picture(uv), vec3<f32>(0.0), vec3<f32>(1.0));
}}
"#,
        band = band_function()
    )
}

struct State {
    legend: bool,
    grey: bool,
    weights: [f64; 3],
    label: String,
}

/// ARRI False Colour's GPU state. Cheap to clone: the Feed thread takes frames through one clone while the
/// window thread draws through another.
#[derive(Clone)]
pub struct ArriFalseColour {
    stage: PictureStage,
    state: Arc<Mutex<State>>,
    zoom: crate::zoom::PanelZoom,
}

impl ArriFalseColour {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> ArriFalseColour {
        let stage = PictureStage::new::<[f32; 4]>(device, queue, target, false, Rule { label: "arri false colour", wgsl: rule(), has_legend: true });
        let display = ArriFalseColour { stage, zoom: Default::default(), state: Arc::new(Mutex::new(State { legend: true, grey: true, weights: crate::measurement::REC709_LUMA, label: String::new() })) };
        display.configure(&DisplayContext::default());
        display
    }
}

impl ArriFalseColour {
    /// The height the legend takes under the picture in a Panel of content `area`: what zoom keeps free.
    fn strip(&self, area: [f32; 4], scale: f32) -> f32 {
        BAR.layout(self.state.lock().unwrap().legend, area, scale, false).map_or(0.0, |l| l.legend.strip)
    }
}

impl Display for ArriFalseColour {
    fn name(&self) -> &'static str {
        "ARRI False Colour"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(encoder, frame);
    }

    fn configure(&self, context: &DisplayContext) {
        // The Output picture's Luma, weighted as the luma waveform weights it (ADR 0004): Rec.709 is assumed, and
        // the label says so, until the Colour Setup is known and the picture is converted.
        let measurement = output_conversion::measurement(context.colour_setup.clone(), context.luma);
        let weights = measurement.weights();
        let space = if measurement.assumed { format!("{} assumed, unconverted", measurement.space.name()) } else { measurement.space.name().to_string() };
        let mut state = self.state.lock().unwrap();
        state.weights = weights;
        state.legend = context.settings.get("legend").is_none_or(|v| v == "on");
        state.grey = context.settings.get(BW_SETTING).is_none_or(|v| v == "on");
        state.label = format!(
            "ARRI-style False Colour  ·  Output Luma, {space}  ·  Y = {:.4} R + {:.4} G + {:.4} B  ·  Rec.709 percentage bands published by third parties; not ARRI's specification, not affiliated with ARRI",
            weights[0], weights[1], weights[2]
        );
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn toolbar_defs(&self) -> &'static [ToolbarDef] {
        &TOOLBAR
    }

    fn pointer(&self, area: [f32; 4], scale: f32, event: crate::display::Pointer) -> crate::display::PointerResult {
        self.zoom.pointer(area, self.stage.size(), self.strip(area, scale), event).unwrap_or_default()
    }

    fn zoom(&self, area: [f32; 4], scale: f32) -> Option<crate::zoom::Readout> {
        self.zoom.readout(area, self.stage.size(), self.strip(area, scale))
    }

    fn set_zoom(&self, area: [f32; 4], scale: f32, percent: f32) {
        self.zoom.set_percent(percent, area, self.stage.size(), self.strip(area, scale));
    }

    fn link_to(&self, link: &crate::zoom::Link) {
        self.zoom.link_to(link);
    }

    fn zoom_model(&self) -> Option<crate::zoom::ZoomModel> {
        Some(self.zoom.model())
    }

    fn label(&self) -> String {
        self.state.lock().unwrap().label.clone()
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let state = self.state.lock().unwrap();
        let w = state.weights;
        self.stage.draw_zoomed(pass, area, surface, &[w[0] as f32, w[1] as f32, w[2] as f32, state.grey as u32 as f32], BAR.layout(state.legend, area, scale, false).map(|l| l.legend), &self.zoom.model());
    }

    /// The ruler's tick numbers and the marks under the bar, where they fit.
    fn scale_texts(&self, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        let state = self.state.lock().unwrap();
        let Some(layout) = BAR.layout(state.legend, area, scale, self.stage.has_picture()) else { return Vec::new() };
        layout.ticks.into_iter().chain(layout.marks).collect()
    }
}
