//! The one shape every display has (ADR 0011, spec #49). A Scope or a Picture View is a module that implements
//! [`Display`] and is registered in [`DisplaySet::standard`]; nothing else changes to add it. Each display:
//!
//! - is created for the target format it draws into;
//! - measures (Scopes) or takes (Picture Views) the Viewer Frame already on the GPU, in [`Display::on_frame`];
//! - draws into its Panel's rectangle, in [`Display::draw`];
//! - takes its settings and the Colour Setup, in [`Display::configure`];
//! - reports its label, in [`Display::label`];
//! - and, for Scopes, can be read back for tests, in [`Display::read_back`].
//!
//! Every Panel's display sees the same frame: the app puts each Viewer Frame on the GPU once (and converts it
//! once) and hands that one [`GpuFrame`] to every display.
use crate::colour_setup::KnownColourSetup;
use crate::gpu_frame::GpuFrame;
use crate::waveform::{DEFAULT_LUMA_WEIGHTS, LumaWeights, Waveform};
use std::any::Any;
use std::sync::Arc;

/// What a display needs to know besides the frame: the Colour Setup and the user's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayContext {
    /// The Colour Setup in use, with where it came from (`None` when not known).
    pub colour_setup: Option<KnownColourSetup>,
    /// What the Colour Setup part of a label says, including why it is missing when it is.
    pub colour_label: String,
    /// How Luma is weighted (ADR 0004).
    pub luma: LumaWeights,
}

impl Default for DisplayContext {
    fn default() -> DisplayContext {
        DisplayContext { colour_setup: None, colour_label: "Colour Setup unknown".into(), luma: DEFAULT_LUMA_WEIGHTS }
    }
}

/// A piece of text on a display's scale, such as one of the waveform's code numbers. Positions are in device
/// pixels of the window; `top` is the middle of the text's line.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleText {
    pub text: String,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub right_aligned: bool,
}

/// A Scope or Picture View. Cheap to clone and shared between the Feed thread (measures) and the window thread
/// (draws), so every method takes `&self`.
pub trait Display: Send + Sync {
    /// What the display is called.
    fn name(&self) -> &'static str;
    /// Measure (Scopes) or take (Picture Views) `frame`, which is already on the GPU. Called on the Feed thread
    /// for every frame, into the one command encoder of that frame.
    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame);
    /// Take the Colour Setup and the settings, from the next frame on.
    fn configure(&self, context: &DisplayContext);
    /// The quiet label: what the display shows, and how far it can be trusted (ADR 0007).
    fn label(&self) -> String;
    /// Draw into `area` (left, top, width, height in pixels of a `surface`-sized target) at `scale` device pixels
    /// per logical pixel. Draws nothing until a frame has arrived.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32);
    /// The text on the display's scale when drawn into `area`, for the window to render. Only what fits.
    fn scale_texts(&self, _area: [f32; 4], _scale: f32) -> Vec<ScaleText> {
        Vec::new()
    }
    /// For tests: what a Scope measured, read back from the GPU. `None` for a Picture View.
    fn read_back(&self) -> Option<Box<dyn Any>> {
        None
    }
}

/// Every display, in Panel order (the order of the grid, left to right then down).
#[derive(Clone, Default)]
pub struct DisplaySet {
    displays: Vec<Arc<dyn Display>>,
}

impl DisplaySet {
    /// The displays the app shows, in Panel order. A new display is one module that implements [`Display`], and
    /// one line here.
    pub fn standard(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool) -> DisplaySet {
        let mut set = DisplaySet::default();
        set.register(Arc::new(crate::picture_view::PictureView::new(device, queue, target, filterable)));
        set.register(Arc::new(Waveform::new(device, queue, target)));
        set
    }

    /// Add a display after the others.
    pub fn register(&mut self, display: Arc<dyn Display>) {
        self.displays.push(display);
    }

    pub fn len(&self) -> usize {
        self.displays.len()
    }

    pub fn is_empty(&self) -> bool {
        self.displays.is_empty()
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.displays.iter().map(|d| d.name()).collect()
    }

    pub fn labels(&self) -> Vec<String> {
        self.displays.iter().map(|d| d.label()).collect()
    }

    /// Hand `frame` to every display.
    pub fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        for display in &self.displays {
            display.on_frame(encoder, frame);
        }
    }

    pub fn configure(&self, context: &DisplayContext) {
        for display in &self.displays {
            display.configure(context);
        }
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, index: usize, area: [f32; 4], surface: [f32; 2], scale: f32) {
        self.displays[index].draw(pass, area, surface, scale);
    }

    pub fn scale_texts(&self, index: usize, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        self.displays[index].scale_texts(area, scale)
    }

    pub fn read_back(&self, index: usize) -> Option<Box<dyn Any>> {
        self.displays[index].read_back()
    }
}
