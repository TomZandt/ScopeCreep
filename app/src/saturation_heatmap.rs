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
use crate::muted_palette;
use crate::output_conversion;
use crate::picture_stage::{PictureStage, Rule};
use crate::scale_bar::ScaleBar;
use crate::settings::{Amount, Choice, SettingDef};
use crate::vectorscope::RING;
use std::sync::{Arc, Mutex};

pub const SCALE_SETTING: &str = "scale";
pub const LEGEND_SETTING: &str = "legend";
/// The scale slider (#90): the percentage of the vectorscope's ring at the hot end. 75 % is the default (ADR 0023);
/// the lists earlier versions saved (25, 50, 75, 100, 150, 200) are the same numbers, which a reader takes as they are.
pub const SCALE_AMOUNT: Amount = Amount {
    min: 25.0,
    max: 200.0,
    step: 5.0,
    default: 75.0,
    unit: " %",
    decimals: 0,
    legacy: &[],
};

/// The hot end in percent for a stored setting (out of the range is clamped, unreadable is the default).
pub fn scale_of(stored: Option<&str>) -> f64 {
    SCALE_AMOUNT.read(stored) as f64
}

const SETTINGS: [SettingDef; 2] = [
    SettingDef {
        key: SCALE_SETTING,
        label: "Scale (hot end)",
        amount: Some(SCALE_AMOUNT),
        choices: &[],
    },
    SettingDef {
        key: LEGEND_SETTING,
        label: "Legend",
        amount: None,
        choices: &[
            Choice {
                value: "on",
                text: "On",
            },
            Choice {
                value: "off",
                text: "Off",
            },
        ],
    },
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

/// The legend: the same ruler as ARRI False Colour's, from 0 % to the scale's top, with no marks (#122).
fn bar(top: f64) -> ScaleBar<'static> {
    ScaleBar {
        from: 0.0,
        to: top,
        unit: " %",
        steps: &[10.0, 20.0, 25.0, 50.0, 100.0],
        marks: &[],
    }
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
    zoom: crate::zoom::PanelZoom,
}

impl SaturationHeatmap {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: wgpu::TextureFormat,
    ) -> SaturationHeatmap {
        let stage = PictureStage::new::<[f32; 4]>(
            device,
            queue,
            target,
            false,
            Rule {
                label: "saturation heatmap",
                wgsl: rule(),
                has_legend: true,
            },
        );
        let display = SaturationHeatmap {
            stage,
            zoom: Default::default(),
            state: Arc::new(Mutex::new(State {
                legend: true,
                scale: 75.0,
                weights: crate::measurement::REC709_LUMA,
                label: String::new(),
            })),
        };
        display.configure(&DisplayContext::default());
        display
    }
}

impl crate::zoom::PictureZoom for SaturationHeatmap {
    fn panel_zoom(&self) -> &crate::zoom::PanelZoom {
        &self.zoom
    }

    fn picture_size(&self) -> Option<[u32; 2]> {
        self.stage.size()
    }

    /// The height the legend takes under the picture: what zoom keeps free.
    fn strip(&self, area: [f32; 4], scale: f32) -> f32 {
        let state = self.state.lock().unwrap();
        bar(state.scale)
            .layout(state.legend, area, scale, false)
            .map_or(0.0, |l| l.legend.strip)
    }
}

impl Display for SaturationHeatmap {
    fn name(&self) -> &'static str {
        "Saturation heatmap"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(encoder, frame);
    }

    fn configure(&self, context: &DisplayContext) {
        let measurement =
            output_conversion::measurement(context.colour_setup.clone(), context.luma);
        let weights = measurement.weights();
        let space = if measurement.assumed {
            format!("{} assumed, unconverted", measurement.space.name())
        } else {
            measurement.space.name().to_string()
        };
        let scale = scale_of(context.settings.get(SCALE_SETTING).map(String::as_str));
        let mut state = self.state.lock().unwrap();
        state.weights = weights;
        state.scale = scale;
        state.legend = context
            .settings
            .get(LEGEND_SETTING)
            .is_none_or(|v| v == "on");
        state.label = format!(
            "Saturation heatmap  ·  Output picture, {space}  ·  distance from neutral in Cb/Cr (Y = {:.4} R + {:.4} G + {:.4} B), 100 % = the vectorscope's 100 % ring  ·  grey 0 % to hot {} %",
            weights[0],
            weights[1],
            weights[2],
            SCALE_AMOUNT.text(scale as f32).trim_end_matches(" %")
        );
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn picture_zoom(&self) -> Option<&dyn crate::zoom::PictureZoom> {
        Some(self)
    }

    fn label(&self) -> String {
        self.state.lock().unwrap().label.clone()
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let state = self.state.lock().unwrap();
        let gain = (100.0 / (state.scale * RING)) as f32;
        let w = state.weights;
        self.stage.draw_zoomed(
            pass,
            area,
            surface,
            &[w[0] as f32, w[1] as f32, w[2] as f32, gain],
            bar(state.scale)
                .layout(state.legend, area, scale, false)
                .map(|l| l.legend),
            &self.zoom.model(),
        );
    }

    /// The ruler's tick numbers, from 0 % to the scale's top.
    fn scale_texts(&self, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        let state = self.state.lock().unwrap();
        bar(state.scale)
            .layout(state.legend, area, scale, self.stage.has_picture())
            .map_or(Vec::new(), |l| l.ticks)
    }
}
