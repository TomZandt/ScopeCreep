//! Highlight Neutrals (spec #49 stories 79-80): the Output picture in grey, with the pixels whose red, green and
//! blue are nearly equal shown in green, for checking that whites and greys are balanced.
//!
//! The grey is Luma with the weights the luma waveform uses by default (ADR 0004), so a grey here lines up with
//! its trace. A pixel is neutral when its largest and smallest channel differ by no more than the tolerance, a
//! share of the full signal range (the Output Colour Space's encoded values, 0 to 1). The tolerance is a list of
//! choices (ADR 0013), the first being the default of 3 %.
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{PictureStage, Rule};
use crate::output_conversion::{Plan, measurement};
use crate::settings::{Choice, SettingDef};
use std::sync::{Arc, Mutex};

pub const TOLERANCE_SETTING: &str = "tolerance";

const SETTINGS: [SettingDef; 1] = [SettingDef {
    key: TOLERANCE_SETTING,
    label: "Neutral tolerance",
    amount: None,
    choices: &[
        Choice { value: "3", text: "3 % (default)" },
        Choice { value: "0.5", text: "0.5 %" },
        Choice { value: "1", text: "1 %" },
        Choice { value: "2", text: "2 %" },
        Choice { value: "5", text: "5 %" },
        Choice { value: "10", text: "10 %" },
    ],
}];

/// The colour rule: Luma in grey, with the near-neutral pixels green.
const RULE: &str = r#"
struct Values {
    weights: vec4<f32>,   // luma weights for R, G, B
    tolerance: vec4<f32>, // x: the largest channel difference still neutral
};

fn colour(uv: vec2<f32>) -> vec3<f32> {
    let rgb = load_picture(uv);
    let grey = clamp(dot(rgb, values.weights.xyz), 0.0, 1.0);
    let spread = max(max(rgb.r, rgb.g), rgb.b) - min(min(rgb.r, rgb.g), rgb.b);
    if (spread <= values.tolerance.x) {
        // Neutral: green, keeping the grey's lightness so the shape of the picture stays readable.
        return vec3<f32>(0.25 * grey, 0.4 + 0.6 * grey, 0.25 * grey);
    }
    return vec3<f32>(grey);
}
"#;

/// What the last [`DisplayContext`] set, for drawing and for the label.
#[derive(Clone)]
struct Config {
    weights: [f32; 3],
    /// The tolerance as the setting says it (a percentage).
    percent: String,
    tolerance: f32,
    /// How the picture was made, for the label (ADR 0007).
    source: String,
}

impl Default for Config {
    fn default() -> Config {
        Config { weights: crate::measurement::REC709_LUMA.map(|v| v as f32), percent: "3".into(), tolerance: 0.03, source: String::new() }
    }
}

/// The display's GPU state. Cheap to clone: the Feed thread takes frames through one clone while the window
/// thread draws through another.
#[derive(Clone)]
pub struct HighlightNeutrals {
    stage: PictureStage,
    config: Arc<Mutex<Config>>,
}

impl HighlightNeutrals {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, _filterable: bool) -> HighlightNeutrals {
        let stage = PictureStage::new::<[f32; 8]>(device, queue, target, false, Rule { label: "highlight neutrals", wgsl: RULE.into(), has_legend: false });
        HighlightNeutrals { stage, config: Arc::new(Mutex::new(Config::default())) }
    }
}

impl Display for HighlightNeutrals {
    fn name(&self) -> &'static str {
        "Highlight neutrals"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(encoder, frame);
    }

    fn configure(&self, context: &DisplayContext) {
        let setup = context.colour_setup.as_ref().map(|c| &c.setup);
        let plan = Plan::for_setup(setup);
        let [r, g, b] = measurement(context.colour_setup.clone(), context.luma).weights();
        let percent = context.settings.get(TOLERANCE_SETTING).cloned().unwrap_or_else(|| "3".into());
        let tolerance = percent.parse::<f32>().map_or(0.03, |p| p / 100.0);
        // The picture is the Output picture only once it has been converted (ADR 0007).
        let what = if plan.method().is_some() || plan.measured_space(setup).is_some() { "Output picture" } else { "Picture as sent" };
        let source = format!("{what}  ·  {}  ·  {}", context.colour_label, plan.label());
        *self.config.lock().unwrap() = Config { weights: [r as f32, g as f32, b as f32], percent, tolerance, source };
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn label(&self) -> String {
        if !self.stage.has_picture() {
            return "Highlight neutrals".to_string();
        }
        let config = self.config.lock().unwrap();
        format!("Highlight neutrals  ·  green where R, G and B are within {} % of each other, the rest in Luma grey  ·  {}", config.percent, config.source)
    }

    /// Draw the picture, scaled to fit inside `area` with its shape kept.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], _scale: f32) {
        let config = self.config.lock().unwrap().clone();
        let [r, g, b] = config.weights;
        self.stage.draw(pass, area, surface, &[r, g, b, 0.0, config.tolerance, 0.0, 0.0, 0.0], None);
    }
}
