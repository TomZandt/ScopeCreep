//! Drawing the Picture View: the latest Feed frame as an image, after Output Conversion.
//!
//! The frame is the 32-bit float texture at full resolution that `gpu_frame` uploads, or the converted one
//! `output_conversion` makes from it, drawn scaled to fit. Nothing else is done to it: the Output Colour Space
//! values go to the screen as they are (clamped to 0..1 by the display surface, which is not sRGB-encoded for
//! that reason).
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::output_conversion::Plan;
use crate::picture_stage::{PictureStage, Rule};
use crate::settings::SettingDef;
use crate::toolbar::{CHANNEL_DEF, CHANNEL_SETTING, CHANNEL_TOOLBAR, ToolbarDef};
use std::sync::{Arc, Mutex};

/// The colour rule: the frame as it is.
/// With a channel chosen (`values.weights.w`: 1 Luma by the weights the waveform uses (ADR 0004), 2 red, 3 green,
/// 4 blue) that channel as grey.
const RULE: &str = r#"
struct Values { weights: vec4<f32> };   // luma weights for R, G, B; w: 0 colour, 1 Luma, 2 R, 3 G, 4 B
fn colour(uv: vec2<f32>) -> vec3<f32> {
    let rgb = sample_picture(uv);
    let c = values.weights.w;
    if c < 0.5 { return rgb; }
    if c < 1.5 { return vec3<f32>(dot(rgb, values.weights.xyz)); }
    if c < 2.5 { return vec3<f32>(rgb.r); }
    if c < 3.5 { return vec3<f32>(rgb.g); }
    return vec3<f32>(rgb.b);
}
"#;

/// The channel is Y R G B chips on the title bar, none lit (colour) by default.
pub static SETTINGS: [SettingDef; 1] = [CHANNEL_DEF];
pub static TOOLBAR: [ToolbarDef; 1] = CHANNEL_TOOLBAR;

/// What the last [`DisplayContext`] set for drawing.
#[derive(Clone, Copy)]
struct Look {
    weights: [f32; 3],
    /// 0 colour, 1 Luma, 2 red, 3 green, 4 blue: what the rule reads.
    channel: f32,
}

/// The Picture View's GPU state. Cheap to clone: the Feed thread uploads frames through one clone while the
/// window thread draws through another.
#[derive(Clone)]
pub struct PictureView {
    stage: PictureStage,
    /// The Colour Setup and conversion part of the label, from the last [`DisplayContext`].
    label_tail: Arc<Mutex<String>>,
    look: Arc<Mutex<Look>>,
}

impl PictureView {
    /// `filterable`: the device can smooth 32-bit float textures (else the picture is drawn nearest-neighbour).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool) -> PictureView {
        let stage = PictureStage::new::<[f32; 4]>(device, queue, target, filterable, Rule { label: "picture view", wgsl: RULE.into(), has_legend: false });
        let view = PictureView { stage, label_tail: Arc::new(Mutex::new(String::new())), look: Arc::new(Mutex::new(Look { weights: crate::measurement::REC709_LUMA.map(|v| v as f32), channel: 0.0 })) };
        view.configure(&DisplayContext::default());
        view
    }

    /// Show the Feed frame [`FrameUploader`](scopecreep::gpu_frame::FrameUploader) put on the GPU. Called on
    /// the Feed thread.
    pub fn show(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(encoder, frame);
    }

    /// What was done to the picture, and how far it can be trusted (ticket #17): never more than is proven.
    fn label_tail(context: &DisplayContext) -> String {
        let conversion = Plan::for_setup(context.colour_setup.as_ref().map(|c| &c.setup));
        format!("{}  ·  {}", context.colour_label, conversion.label())
    }

    fn label_with(size: Option<[u32; 2]>, tail: &str) -> String {
        match size {
            Some([w, h]) => format!("Clean picture  ·  {w} × {h}  ·  {tail}"),
            None => "Clean picture".to_string(),
        }
    }

    /// The label the Picture View shows for a frame of `size` under `context`: the one source of that text, which
    /// [`Display::label`] gives too (for tests and tools that have no GPU to make a Picture View on).
    pub fn label_for(size: Option<[u32; 2]>, context: &DisplayContext) -> String {
        Self::label_with(size, &Self::label_tail(context))
    }

    /// The size of the frame on screen (width, height), once one has arrived: the window lays the panels out in
    /// its shape, so they follow a change of timeline resolution.
    pub fn frame_size(&self) -> Option<[u32; 2]> {
        self.stage.size()
    }
}

impl Display for PictureView {
    fn name(&self) -> &'static str {
        "Clean picture"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.show(encoder, frame);
    }

    fn configure(&self, context: &DisplayContext) {
        *self.label_tail.lock().unwrap() = Self::label_tail(context);
        let [r, g, b] = crate::output_conversion::measurement(context.colour_setup.clone(), context.luma).weights();
        *self.look.lock().unwrap() = Look { weights: [r as f32, g as f32, b as f32], channel: match context.settings.get(CHANNEL_SETTING).map(String::as_str) {
            Some("luma") => 1.0,
            Some("red") => 2.0,
            Some("green") => 3.0,
            Some("blue") => 4.0,
            _ => 0.0,
        },
        };
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn toolbar_defs(&self) -> &'static [ToolbarDef] {
        &TOOLBAR
    }

    fn label(&self) -> String {
        Self::label_with(self.frame_size(), &self.label_tail.lock().unwrap())
    }

    /// Draw the picture, scaled to fit inside `area` with its shape kept.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], _scale: f32) {
        let look = *self.look.lock().unwrap();
        self.stage.draw(pass, area, surface, &[look.weights[0], look.weights[1], look.weights[2], look.channel], None);
    }
}
