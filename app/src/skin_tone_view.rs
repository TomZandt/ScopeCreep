//! The Skin Tone Panel (spec #49 stories 68-74, #101): the Output picture in its own colours, with the pixels whose
//! hue is near the skin-tone line painted solid flat colours: yellow on the line, green or magenta as the hue drifts
//! to either side. Neutral pixels and hues far from skin keep their natural colour. Only hue matters, not
//! brightness, so it works for every skin tone.
//!
//! Hue is the angle of the pixel's Cb/Cr (the vectorscope's, with the same luma weights), measured from the
//! shared [`SKIN_TONE_LINE_DEGREES`], which is the angle Resolve 21.2's own vectorscope draws its line at (#60, ADR 0018). The
//! tolerance `t` is a list of choices (ADR 0013, ADR 0021): yellow within `t` of the line, green or magenta from `t`
//! out to 6 t, and the pixel's own colour beyond. At the default 5 degrees that is the spec's 5 and 30. Towards a
//! larger Cb/Cr angle (towards yellow) is green, the other way magenta.
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{PictureStage, Rule};
use crate::output_conversion::{Plan, measurement};
use crate::settings::{Choice, SettingDef};
use crate::toolbar::{BW_DEFAULT_COLOUR, BW_SETTING, BW_TOOLBAR, ToolbarDef};
use crate::skin_tone::SKIN_TONE_LINE_DEGREES;
use std::sync::{Arc, Mutex};

pub const TOLERANCE_SETTING: &str = "tolerance";

pub const DEFAULT_TOLERANCE: &str = "5";

pub static SETTINGS: [SettingDef; 2] = [SettingDef {
    key: TOLERANCE_SETTING,
    label: "On-line tolerance",
    choices: &[
        Choice { value: DEFAULT_TOLERANCE, text: "±5° (default)" },
        Choice { value: "2", text: "±2°" },
        Choice { value: "3", text: "±3°" },
        Choice { value: "8", text: "±8°" },
        Choice { value: "10", text: "±10°" },
    ],
}, BW_DEFAULT_COLOUR];
pub static TOOLBAR: [ToolbarDef; 1] = BW_TOOLBAR;

/// The colour rule: solid flat colours for the hues near the skin-tone line, the picture's own colour elsewhere.
const RULE: &str = r#"
struct Values {
    weights: vec4<f32>,   // luma weights for R, G, B
    skin: vec4<f32>,      // x: tolerance in degrees, y: skin-tone line angle in degrees, z: 1 for the unpainted picture in grey
};

const PI: f32 = 3.14159265358979;

fn colour(uv: vec2<f32>) -> vec3<f32> {
    let rgb = load_picture(uv);
    let w = values.weights.xyz;
    let y = dot(rgb, w);
    let cb = (rgb.b - y) / (2.0 * (1.0 - w.z));
    let cr = (rgb.r - y) / (2.0 * (1.0 - w.x));
    // Degrees from the skin-tone line, wrapped to -180..180: positive is towards yellow (green), negative towards red (magenta).
    let turn = atan2(cr, cb) - values.skin.y * PI / 180.0;
    let offset = atan2(sin(turn), cos(turn)) * 180.0 / PI;
    let distance = abs(offset);
    let t = values.skin.x;
    // Solid flat colours, not blended: yellow within t of the line, then green or magenta out to 6 t. Beyond that, and
    // where there is almost no hue, the pixel keeps its own colour.
    let lit = distance <= 6.0 * t && length(vec2<f32>(cb, cr)) >= 0.02;
    let yellow = vec3<f32>(1.0, 0.85, 0.0);
    let green = vec3<f32>(0.1, 0.85, 0.2);
    let magenta = vec3<f32>(0.95, 0.2, 0.85);
    let flat_colour = select(select(magenta, green, offset > 0.0), yellow, distance <= t);
    let unpainted = select(rgb, vec3<f32>(y), values.skin.z > 0.5);
    return select(unpainted, flat_colour, lit);
}
"#;

/// What the last [`DisplayContext`] set, for drawing and for the label.
#[derive(Clone)]
struct Config {
    weights: [f32; 3],
    /// The tolerance as the setting says it (degrees).
    degrees: String,
    tolerance: f32,
    /// B/W: the unpainted picture in Luma grey.
    bw: bool,
    /// How the picture was made, for the label (ADR 0007).
    source: String,
}

impl Default for Config {
    fn default() -> Config {
        Config { weights: crate::waveform::REC709_LUMA.map(|v| v as f32), degrees: DEFAULT_TOLERANCE.into(), tolerance: 5.0, bw: false, source: String::new() }
    }
}

/// The display's GPU state. Cheap to clone: the Feed thread takes frames through one clone while the window
/// thread draws through another.
#[derive(Clone)]
pub struct SkinTone {
    stage: PictureStage,
    config: Arc<Mutex<Config>>,
}

impl SkinTone {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, _filterable: bool) -> SkinTone {
        let stage = PictureStage::new::<[f32; 8]>(device, queue, target, false, Rule { label: "skin tone", wgsl: RULE.into(), has_legend: false });
        SkinTone { stage, config: Arc::new(Mutex::new(Config::default())) }
    }
}

impl Display for SkinTone {
    fn name(&self) -> &'static str {
        "Skin tone"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(encoder, frame);
    }

    fn configure(&self, context: &DisplayContext) {
        let setup = context.colour_setup.as_ref().map(|c| &c.setup);
        let plan = Plan::for_setup(setup);
        // The weights the vectorscope uses, so the hue here is the angle the vectorscope shows.
        let [r, g, b] = measurement(context.colour_setup.clone(), context.luma).weights();
        let degrees = context.settings.get(TOLERANCE_SETTING).cloned().unwrap_or_else(|| DEFAULT_TOLERANCE.into());
        let tolerance = degrees.parse::<f32>().unwrap_or(5.0);
        // The picture is the Output picture only once it has been converted (ADR 0007).
        let what = if plan.method().is_some() || plan.measured_space(setup).is_some() { "Output picture" } else { "Picture as sent" };
        let source = format!("{what}  ·  {}  ·  {}", context.colour_label, plan.label());
        *self.config.lock().unwrap() = Config { weights: [r as f32, g as f32, b as f32], degrees, tolerance, bw: context.settings.get(BW_SETTING).map(String::as_str) == Some("on"), source };
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn toolbar_defs(&self) -> &'static [ToolbarDef] {
        &TOOLBAR
    }

    /// States only what is proven (ADR 0007): the bands in degrees from the line, and that the line's angle is
    /// measured in Resolve 21.2 (#60).
    fn label(&self) -> String {
        if !self.stage.has_picture() {
            return "Skin tone".to_string();
        }
        let config = self.config.lock().unwrap();
        let t: f32 = config.tolerance;
        format!(
            "Skin tone  ·  yellow within ±{}° of the skin-tone line {SKIN_TONE_LINE_DEGREES}° (as Resolve 21.2 draws it), green or magenta out to ±{}°, the picture's own colour beyond and where saturation is low  ·  {}",
            config.degrees,
            t * 6.0,
            config.source
        )
    }

    /// Draw the picture, scaled to fit inside `area` with its shape kept.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], _scale: f32) {
        let config = self.config.lock().unwrap().clone();
        let [r, g, b] = config.weights;
        self.stage.draw(pass, area, surface, &[r, g, b, 0.0, config.tolerance, SKIN_TONE_LINE_DEGREES as f32, config.bw as u32 as f32, 0.0], None);
    }
}
