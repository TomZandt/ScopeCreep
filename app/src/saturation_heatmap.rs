//! The saturation heatmap (spec #49 stories 75-78, ticket #64): a Picture View that draws the Output picture as its
//! Luma in grey with saturation laid over it in a colour that gets stronger with saturation, cool to hot (#100).
//!
//! Saturation is the distance from neutral on the vectorscope's scale: Cb/Cr from the Measured Space's luma
//! weights (ADR 0004), each running -0.5 to 0.5, and the distance `sqrt(Cb^2 + Cr^2)` as a share of the
//! vectorscope's outer ring ([`crate::vectorscope::RING`], 0.6). So 100 % is the vectorscope's 100 % ring, near
//! where 100 % colour bars sit (ADR 0023), and a grey is 0 %. The scale setting is the percentage at the hot
//! end; more saturated pixels stay at the hot end.
use crate::display::{Display, DisplayContext, ScaleText};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{BAR, BAR_STRIP, Legend, PictureStage, Rule};
use crate::muted_palette;
use crate::output_conversion;
use crate::settings::{Choice, SettingDef};
use crate::vectorscope::RING;
use std::sync::{Arc, Mutex};

pub const SCALE_SETTING: &str = "scale";
pub const LEGEND_SETTING: &str = "legend";

const SETTINGS: [SettingDef; 2] = [
    SettingDef {
        key: SCALE_SETTING,
        label: "Scale (hot end)",
        choices: &[
            Choice { value: "75", text: "75 % (default)" },
            Choice { value: "25", text: "25 %" },
            Choice { value: "50", text: "50 %" },
            Choice { value: "100", text: "100 %" },
            Choice { value: "150", text: "150 %" },
            Choice { value: "200", text: "200 %" },
        ],
    },
    SettingDef { key: LEGEND_SETTING, label: "Legend", choices: &[Choice { value: "on", text: "On" }, Choice { value: "off", text: "Off" }] },
];

/// Saturation as a percentage of the vectorscope's 100 % ring, for an encoded `rgb` and luma `weights`.
pub fn saturation_percent(weights: [f64; 3], rgb: [f64; 3]) -> f64 {
    let [cb, cr] = crate::vectorscope::chroma(weights, rgb);
    cb.hypot(cr) / RING * 100.0
}

/// The colour rule: Luma in grey with saturation laid over it; the legend is the same palette at mid grey.
fn rule() -> String {
    format!(
        r#"
struct Values {{ weights: vec4<f32> }};   // luma weights for R, G, B; w: 1 / (scale in Cb/Cr)

{palette}

fn legend(uv: vec2<f32>) -> vec3<f32> {{ return overlay(0.5, uv.x); }}

fn colour(uv: vec2<f32>) -> vec3<f32> {{
    let rgb = load_picture(uv);
    let w = values.weights.xyz;
    let y = dot(rgb, w);
    let cb = (rgb.b - y) / (2.0 * (1.0 - w.z));
    let cr = (rgb.r - y) / (2.0 * (1.0 - w.x));
    return overlay(y, length(vec2<f32>(cb, cr)) * values.weights.w);
}}
"#,
        palette = muted_palette::wgsl()
    )
}

struct State {
    legend: bool,
    /// The percentage at the hot end.
    scale: f64,
    weights: [f64; 3],
    label: String,
}

/// The heatmap's GPU state. Cheap to clone: the Feed thread takes frames through one clone while the
/// window thread draws through another.
#[derive(Clone)]
pub struct SaturationHeatmap {
    stage: PictureStage,
    state: Arc<Mutex<State>>,
}

impl SaturationHeatmap {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> SaturationHeatmap {
        let stage = PictureStage::new::<[f32; 4]>(device, queue, target, false, Rule { label: "saturation heatmap", wgsl: rule(), has_legend: true });
        let display = SaturationHeatmap { stage, state: Arc::new(Mutex::new(State { legend: true, scale: 75.0, weights: [0.2126, 0.7152, 0.0722], label: String::new() })) };
        display.configure(&DisplayContext::default());
        display
    }
}

impl Display for SaturationHeatmap {
    fn name(&self) -> &'static str {
        "Saturation heatmap"
    }

    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(frame);
    }

    fn configure(&self, context: &DisplayContext) {
        let measurement = output_conversion::measurement(context.colour_setup.clone(), context.luma);
        let weights = measurement.weights();
        let space = if measurement.assumed { format!("{} assumed, unconverted", measurement.space.name()) } else { measurement.space.name().to_string() };
        let scale = context.settings.get(SCALE_SETTING).and_then(|v| v.parse::<f64>().ok()).unwrap_or(75.0);
        let mut state = self.state.lock().unwrap();
        state.weights = weights;
        state.scale = scale;
        state.legend = context.settings.get(LEGEND_SETTING).is_none_or(|v| v == "on");
        state.label = format!(
            "Saturation heatmap  ·  Output picture, {space}  ·  distance from neutral in Cb/Cr (Y = {:.4} R + {:.4} G + {:.4} B), 100 % = the vectorscope's 100 % ring  ·  grey 0 % to hot {scale} %",
            weights[0], weights[1], weights[2]
        );
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn label(&self) -> String {
        self.state.lock().unwrap().label.clone()
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let state = self.state.lock().unwrap();
        let gain = (100.0 / (state.scale * RING)) as f32;
        let w = state.weights;
        self.stage.draw(pass, area, surface, &[w[0] as f32, w[1] as f32, w[2] as f32, gain], Legend::bar(state.legend, area, scale));
    }

    /// The two ends of the legend: 0 % under the cool end and the scale's top under the hot end.
    fn scale_texts(&self, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        let state = self.state.lock().unwrap();
        let Some(legend) = Legend::bar(state.legend, area, scale).filter(|_| self.stage.has_picture() && area[2] >= 80.0 * scale) else { return Vec::new() };
        let top = area[1] + area[3] - legend.strip + (BAR + (BAR_STRIP - BAR) / 2.0) * scale;
        let width = area[2] / 2.0;
        vec![
            ScaleText { text: "0 %".into(), left: area[0], top, width, right_aligned: false, centred: false },
            ScaleText { text: format!("{} %", state.scale), left: area[0] + width, top, width, right_aligned: true, centred: false },
        ]
    }
}
