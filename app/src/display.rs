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
use crate::settings::{PanelSettings, SettingDef, Settings};
use crate::waveform::{LumaWeights, Waveform};
use std::any::Any;
use std::sync::Arc;

/// What a display needs to know besides the frame: the Colour Setup and the user's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayContext {
    /// The Colour Setup in use, with where it came from (`None` when not known).
    pub colour_setup: Option<KnownColourSetup>,
    /// What the Colour Setup part of a label says, including why it is missing when it is.
    pub colour_label: String,
    /// This display's own settings (the right-click menu's choices), one valid value for each setting it offers.
    pub settings: Settings,
    /// The luma weights chosen on the luma waveform. Every display that shows Luma follows them, so it agrees
    /// with the waveform whatever Panel it is in.
    pub luma: LumaWeights,
}

impl Default for DisplayContext {
    fn default() -> DisplayContext {
        DisplayContext { colour_setup: None, colour_label: "Colour Setup unknown".into(), settings: Settings::new(), luma: crate::waveform::DEFAULT_LUMA_WEIGHTS }
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
    /// Centred in its width (wins over `right_aligned`).
    pub centred: bool,
}

/// What the pointer does over a Panel, in device pixels of the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    /// The left button went down here.
    Down([f32; 2]),
    /// The pointer moved here (with the left button down, if it went down on the display).
    Move([f32; 2]),
    /// The left button went up.
    Up,
    /// The scroll wheel turned over `at`; `lines` is positive for scrolling up (several for a fast turn, fractions
    /// for a smooth wheel).
    Wheel { at: [f32; 2], lines: f32 },
}

/// What a display made of a [`Pointer`] event.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointerResult {
    /// The display took the event: the window does not use it for anything else.
    pub used: bool,
    /// State to remember (key, value), for [`Display::state_keys`].
    pub remember: Vec<(&'static str, String)>,
}

/// For tests: what the Channel view drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelViewDrawn {
    /// The Loupe's square (left, top, width, height in device pixels).
    pub loupe: [f32; 4],
    /// Its zoom in percent of source pixels.
    pub zoom: u32,
}

/// A Scope or Picture View. Cheap to clone and shared between the Feed thread (measures) and the window thread
/// (draws), so every method takes `&self`.
pub trait Display: Send + Sync {
    /// What the display is called.
    fn name(&self) -> &'static str;
    /// Measure (Scopes) or take (Picture Views) `frame`, which is already on the GPU. Called on the Feed thread
    /// for every frame, into the one command encoder of that frame.
    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame);
    /// Take the Timeline Colour Space frame, before Output Conversion, for displays that measure scene light
    /// (Stops False Colour, ADR 0009). Called right after [`Display::on_frame`] for the same frame. Nothing by
    /// default.
    fn on_timeline_frame(&self, _encoder: &mut wgpu::CommandEncoder, _frame: &GpuFrame) {}
    /// Take the Colour Setup and the settings, from the next frame on.
    fn configure(&self, context: &DisplayContext);
    /// The settings the display offers in its right-click menu, each with its choices (the first is the
    /// default). Only sensible values: a display never offers a setting that could make it mislead. None by
    /// default, and a Panel with no settings shows no menu.
    fn setting_defs(&self) -> &'static [SettingDef] {
        &[]
    }
    /// What the display remembers between sessions besides its menu choices (a Loupe's position and zoom). They
    /// reach [`Display::configure`] in the context's settings, and leave through [`PointerResult::remember`].
    fn state_keys(&self) -> &'static [&'static str] {
        &[]
    }
    /// The pointer over the display's `area` (see [`Display::draw`]). Ignored by default.
    fn pointer(&self, _area: [f32; 4], _scale: f32, _event: Pointer) -> PointerResult {
        PointerResult::default()
    }
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

/// One switch per display that can leave the Window: whether [`DisplaySet::standard`] registers it. A display
/// that is off keeps its module, its tests and its saved settings, so turning it back on is one word here.
pub struct InWindow {
    pub gamut_legal_warning: bool,
    pub highlight_neutrals: bool,
    pub cie_plot: bool,
}

/// What the Window shows (ticket #95: the three displays below are out of it for now).
pub const IN_WINDOW: InWindow = InWindow { gamut_legal_warning: false, highlight_neutrals: false, cie_plot: false };

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
        let luma_waveform = Waveform::new(device, queue, target);
        set.register(Arc::new(luma_waveform.clone()));
        if IN_WINDOW.gamut_legal_warning {
            set.register(Arc::new(crate::gamut_warning::GamutWarning::new(device, queue, target)));
        }
        if IN_WINDOW.highlight_neutrals {
            set.register(Arc::new(crate::highlight_neutrals::HighlightNeutrals::new(device, queue, target, filterable)));
        }
        set.register(Arc::new(crate::channel_view::ChannelView::new(device, queue, target, filterable)));
        set.register(Arc::new(crate::arri_false_colour::ArriFalseColour::new(device, queue, target)));
        set.register(Arc::new(crate::black_and_white::BlackAndWhite::new(device, queue, target, filterable)));
        set.register(Arc::new(crate::vectorscope::Vectorscope::new(device, queue, target)));
        set.register(Arc::new(crate::saturation_heatmap::SaturationHeatmap::new(device, queue, target)));
        set.register(Arc::new(crate::skin_tone_view::SkinTone::new(device, queue, target, filterable)));
        set.register(Arc::new(crate::stops_false_colour::StopsFalseColour::new(device, queue, target)));
        if IN_WINDOW.cie_plot {
            set.register(Arc::new(crate::cie::CiePlot::new(device, queue, target)));
        }
        set.register(Arc::new(Waveform::new_colour(device, queue, target, &luma_waveform)));
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

    /// Hand the Timeline Colour Space frame (before Output Conversion) to every display.
    pub fn on_timeline_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        for display in &self.displays {
            display.on_timeline_frame(encoder, frame);
        }
    }

    /// Give every display the Colour Setup and its own settings (`settings` holds every Panel's choices).
    pub fn configure(&self, context: &DisplayContext, settings: &PanelSettings) {
        let luma = crate::waveform::luma_weights_of(&settings.resolve(crate::waveform::NAME, &crate::waveform::SETTINGS));
        for display in &self.displays {
            let mut own = DisplayContext { settings: settings.resolve(display.name(), display.setting_defs()), luma, ..context.clone() };
            own.settings.extend(settings.state_of(display.name(), display.state_keys()));
            display.configure(&own);
        }
    }

    /// The settings the display in Panel `index` offers.
    pub fn setting_defs(&self, index: usize) -> &'static [SettingDef] {
        self.displays[index].setting_defs()
    }

    /// The settings the display in Panel `index` is using, given everyone's choices.
    pub fn settings_of(&self, index: usize, settings: &PanelSettings) -> Settings {
        let display = &self.displays[index];
        settings.resolve(display.name(), display.setting_defs())
    }

    /// What the display in Panel `index` is called (its settings are kept under this name).
    pub fn name(&self, index: usize) -> &'static str {
        self.displays[index].name()
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, index: usize, area: [f32; 4], surface: [f32; 2], scale: f32) {
        self.displays[index].draw(pass, area, surface, scale);
    }

    pub fn scale_texts(&self, index: usize, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        self.displays[index].scale_texts(area, scale)
    }

    /// The pointer over the display in Panel `index`.
    pub fn pointer(&self, index: usize, area: [f32; 4], scale: f32, event: Pointer) -> PointerResult {
        self.displays[index].pointer(area, scale, event)
    }

    /// Whether the display in Panel `index` wants the pointer (it has state to move).
    pub fn takes_pointer(&self, index: usize) -> bool {
        !self.displays[index].state_keys().is_empty()
    }

    pub fn read_back(&self, index: usize) -> Option<Box<dyn Any>> {
        self.displays[index].read_back()
    }
}
