//! The gamut/legal warning (spec #49): the Output picture's Luma in black and white, with problem pixels coloured.
//!
//! Luma (from the Output Colour Space's primaries, ADR 0004) is drawn as grey. From the upper threshold (90 %
//! by default) up to legal white a pixel fades towards red, and above legal white it is solid red; from the lower
//! threshold (10 %) down to legal black it fades towards blue, and below legal black it is solid blue. Every
//! channel is checked, not just Luma: a channel above the upper limit or below the lower limit makes the pixel
//! solid red or blue, so a colour outside the Output Colour Space's gamut is caught too (ADR 0009: clipping is
//! judged on the Output picture).
//!
//! Percentages are of the range of the project's Levels (video 64 to 940, data 0 to 1023). Inside Resolve every
//! value is full range, so 0 % is value 0.0 and 100 % is 1.0 whichever Levels are set; the Levels decide the
//! codes the label names. EBU R103 tolerance moves the limits to -5 % and 105 %.
use crate::colour_setup::Provenance;
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{PictureStage, Rule};
use crate::settings::{Choice, SettingDef};
use crate::measurement::Measurement;
use std::sync::{Arc, Mutex};

/// EBU R103: signals up to -5 % and 105 % are tolerated.
pub const R103_TOLERANCE: f32 = 0.05;

/// The colour rule: Luma in grey, with the pixels past a limit solid or fading to red and blue.
const RULE: &str = r#"
struct Values {
    limits: vec4<f32>,    // blue starts at x and is solid at y; red starts at z and is solid at w
    weights: vec4<f32>,   // luma weights for red, green, blue
};

const RED = vec3<f32>(1.0, 0.0, 0.0);
const BLUE = vec3<f32>(0.0, 0.0, 1.0);

fn colour(uv: vec2<f32>) -> vec3<f32> {
    let rgb = load_picture(uv);
    let y = dot(rgb, values.weights.xyz);
    let grey = vec3<f32>(clamp(y, 0.0, 1.0));
    // Every channel: past a limit in any of them is solid, whatever the luma says.
    if (max(rgb.r, max(rgb.g, rgb.b)) > values.limits.w) {
        return RED;
    }
    if (min(rgb.r, min(rgb.g, rgb.b)) < values.limits.y) {
        return BLUE;
    }
    if (y > values.limits.z) {
        return mix(grey, RED, clamp((y - values.limits.z) / (values.limits.w - values.limits.z), 0.0, 1.0));
    }
    if (y < values.limits.x) {
        return mix(grey, BLUE, clamp((values.limits.x - y) / (values.limits.x - values.limits.y), 0.0, 1.0));
    }
    return grey;
}
"#;

const LOW_SETTING: &str = "low";
const HIGH_SETTING: &str = "high";
const R103_SETTING: &str = "r103";

static SETTINGS: [SettingDef; 3] = [
    SettingDef {
        key: LOW_SETTING,
        label: "Blue starts at",
        choices: &[
            Choice { value: "10", text: "10 %" },
            Choice { value: "5", text: "5 %" },
            Choice { value: "15", text: "15 %" },
            Choice { value: "20", text: "20 %" },
        ],
    },
    SettingDef {
        key: HIGH_SETTING,
        label: "Red starts at",
        choices: &[
            Choice { value: "90", text: "90 %" },
            Choice { value: "95", text: "95 %" },
            Choice { value: "85", text: "85 %" },
            Choice { value: "80", text: "80 %" },
        ],
    },
    SettingDef {
        key: R103_SETTING,
        label: "EBU R103 tolerance (-5 % to 105 %)",
        choices: &[Choice { value: "off", text: "Off" }, Choice { value: "on", text: "On" }],
    },
];

/// What the warning is set to: the thresholds (fractions of the range), whether EBU R103 tolerance is on, and the
/// Measurement (Output Colour Space and Levels) it judges in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warning {
    /// Fade to blue starts at this fraction of the range (default 0.10).
    pub low: f32,
    /// Fade to red starts at this fraction of the range (default 0.90).
    pub high: f32,
    pub r103: bool,
    pub measurement: Measurement,
}

impl Default for Warning {
    fn default() -> Warning {
        Warning { low: 0.10, high: 0.90, r103: false, measurement: Measurement::default() }
    }
}

impl Warning {
    pub fn from_context(context: &DisplayContext) -> Warning {
        let percent = |key: &str, default: f32| context.settings.get(key).and_then(|v| v.parse::<f32>().ok()).map_or(default, |v| v / 100.0);
        Warning {
            low: percent(LOW_SETTING, 0.10),
            high: percent(HIGH_SETTING, 0.90),
            r103: context.settings.get(R103_SETTING).is_some_and(|v| v == "on"),
            measurement: crate::output_conversion::measurement(context.colour_setup.clone(), context.luma),
        }
    }

    /// The value a pixel (or a channel) must pass to be solid blue.
    pub fn lower_limit(&self) -> f32 {
        if self.r103 { -R103_TOLERANCE } else { 0.0 }
    }

    /// The value a pixel (or a channel) must pass to be solid red.
    pub fn upper_limit(&self) -> f32 {
        if self.r103 { 1.0 + R103_TOLERANCE } else { 1.0 }
    }

    /// The code a fraction of the range is, on the 10-bit scale of the Levels.
    fn code(&self, fraction: f32) -> i32 {
        let (scale, offset) = self.measurement.levels.scale_and_offset();
        (fraction as f64 * scale + offset).round() as i32
    }

    /// The quiet label: the Levels and the tolerance in use, and what is judged in (ADR 0007).
    pub fn label(&self) -> String {
        let m = &self.measurement;
        let range = m.levels.range_label();
        let levels = match m.levels_from {
            None => format!("{range} assumed"),
            Some(Provenance::Project) => range.to_string(),
            Some(Provenance::Manual) => format!("{range} set manually"),
        };
        let space = if m.assumed { format!("{} assumed, judged as sent, unconverted", m.space.name()) } else { m.space.name().to_string() };
        let percent = |f: f32| format!("{:.0} %", f * 100.0);
        let tolerance = if self.r103 {
            format!("EBU R103 tolerance on ({} to {})", percent(self.lower_limit()), percent(self.upper_limit()))
        } else {
            "no tolerance".to_string()
        };
        format!(
            "Gamut/legal warning  ·  {levels}  ·  {space}  ·  blue below {} (code {}), red above {} (code {})  ·  {tolerance}  ·  every channel checked",
            percent(self.low),
            self.code(self.low),
            percent(self.high),
            self.code(self.high),
        )
    }
}

/// The gamut/legal warning's GPU state. Cheap to clone, like the other displays.
#[derive(Clone)]
pub struct GamutWarning {
    stage: PictureStage,
    warning: Arc<Mutex<Warning>>,
}

impl GamutWarning {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> GamutWarning {
        let stage = PictureStage::new::<[f32; 8]>(device, queue, target, false, Rule { label: "gamut warning", wgsl: RULE.into(), has_legend: false });
        GamutWarning { stage, warning: Arc::new(Mutex::new(Warning::default())) }
    }
}

impl Display for GamutWarning {
    fn name(&self) -> &'static str {
        "Gamut/legal warning"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(encoder, frame);
    }

    fn configure(&self, context: &DisplayContext) {
        *self.warning.lock().unwrap() = Warning::from_context(context);
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn label(&self) -> String {
        self.warning.lock().unwrap().label()
    }

    /// Draw the picture's warning, scaled to fit inside `area` with the frame's shape kept.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], _scale: f32) {
        let warning = *self.warning.lock().unwrap();
        let [r, g, b] = warning.measurement.weights();
        self.stage.draw(pass, area, surface, &[warning.low, warning.lower_limit(), warning.high, warning.upper_limit(), r as f32, g as f32, b as f32, 0.0], None);
    }
}
