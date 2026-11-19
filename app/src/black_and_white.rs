//! The black-and-white Picture View: the Output picture with each pixel's grey equal to its Luma, using the same
//! weights as the waveform (the Measured Space's, ADR 0004), so a grey you see lines up exactly with its trace.
//!
//! The weights come from [`Measurement`], the waveform's own type, built from the same Colour Setup and the luma
//! weights chosen on the waveform (`DisplayContext::luma`). The grey is the luma value 0 to 1 itself: the display
//! surface is not sRGB-encoded, so it goes to the screen as it is, like the clean picture's values.
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{PictureStage, Rule};
use crate::measurement::Measurement;
use std::sync::{Arc, Mutex};

/// The colour rule: Luma, the weights being the values.
const RULE: &str = r#"
struct Values { weights: vec4<f32> };   // luma weights for R, G, B
fn colour(uv: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(dot(sample_picture(uv), values.weights.xyz));
}
"#;

/// The black-and-white Panel's GPU state. Cheap to clone, shared between the Feed and window threads.
#[derive(Clone)]
pub struct BlackAndWhite {
    stage: PictureStage,
    /// What Luma means for the picture: the waveform's own measurement, so the two cannot disagree.
    measurement: Arc<Mutex<Measurement>>,
}

impl BlackAndWhite {
    /// `filterable`: the device can smooth 32-bit float textures (else the picture is drawn nearest-neighbour).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool) -> BlackAndWhite {
        let stage = PictureStage::new::<[f32; 4]>(device, queue, target, filterable, Rule { label: "black and white", wgsl: RULE.into(), has_legend: false });
        BlackAndWhite { stage, measurement: Arc::new(Mutex::new(Measurement::default())) }
    }
}

impl Display for BlackAndWhite {
    fn name(&self) -> &'static str {
        "Black and white"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(encoder, frame);
    }

    /// The weights are the waveform's: the same Colour Setup and the same luma weights choice.
    fn configure(&self, context: &DisplayContext) {
        *self.measurement.lock().unwrap() = crate::output_conversion::measurement(context.colour_setup.clone(), context.luma);
    }

    fn label(&self) -> String {
        match self.stage.has_picture() {
            true => format!("Black and white  ·  Luma  ·  {}", self.measurement.lock().unwrap().weights_label()),
            false => "Black and white".to_string(),
        }
    }

    /// Draw the picture in grey, scaled to fit inside `area` with its shape kept.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], _scale: f32) {
        let [r, g, b] = self.measurement.lock().unwrap().weights();
        self.stage.draw(pass, area, surface, &[r as f32, g as f32, b as f32, 0.0], None);
    }
}
