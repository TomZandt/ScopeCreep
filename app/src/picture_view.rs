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
use std::sync::{Arc, Mutex};

/// The colour rule: the frame as it is.
const RULE: &str = r#"
struct Values { unused: vec4<f32> };
fn colour(uv: vec2<f32>) -> vec3<f32> { return sample_picture(uv); }
"#;

/// The Picture View's GPU state. Cheap to clone: the Feed thread uploads frames through one clone while the
/// window thread draws through another.
#[derive(Clone)]
pub struct PictureView {
    stage: PictureStage,
    /// The Colour Setup and conversion part of the label, from the last [`DisplayContext`].
    label_tail: Arc<Mutex<String>>,
}

impl PictureView {
    /// `filterable`: the device can smooth 32-bit float textures (else the picture is drawn nearest-neighbour).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool) -> PictureView {
        let stage = PictureStage::new::<[f32; 4]>(device, queue, target, filterable, Rule { label: "picture view", wgsl: RULE.into(), has_legend: false });
        let view = PictureView { stage, label_tail: Arc::new(Mutex::new(String::new())) };
        view.configure(&DisplayContext::default());
        view
    }

    /// Show the Feed frame [`FrameUploader`](scopecreep::gpu_frame::FrameUploader) put on the GPU. Called on
    /// the Feed thread.
    pub fn show(&self, frame: &GpuFrame) {
        self.stage.show(frame);
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

    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.show(frame);
    }

    fn configure(&self, context: &DisplayContext) {
        *self.label_tail.lock().unwrap() = Self::label_tail(context);
    }

    fn label(&self) -> String {
        Self::label_with(self.frame_size(), &self.label_tail.lock().unwrap())
    }

    /// Draw the picture, scaled to fit inside `area` with its shape kept.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], _scale: f32) {
        self.stage.draw(pass, area, surface, &[0.0f32; 4], None);
    }
}
