//! The one shape every display has (ADR 0011, spec #49). A Scope or a Picture View is a module that implements
//! [`Display`] and is registered in [`DisplaySet::standard`]; nothing else changes to add it. Each display:
//!
//! - is created for the target format it draws into;
//! - measures (Scopes) or takes (Picture Views) the Viewer Frame already on the GPU, in [`Display::on_frame`];
//! - draws into its Panel's rectangle, in [`Display::draw`];
//! - takes its settings and the Colour Setup, in [`Display::configure`];
//! - reports its label, in [`Display::label`];
//! - and, for Scopes, are read back for tests through their own typed methods (see [`ScopeHandles`]).
//!
//! Every Panel's display sees the same frame: the app puts each Viewer Frame on the GPU once (and converts it
//! once) and hands that one [`GpuFrame`] to every display.
use crate::colour_setup::KnownColourSetup;
use crate::gpu_frame::GpuFrame;
use crate::measurement::LumaWeights;
use crate::settings::{PanelSettings, SettingDef, Settings};
use crate::toolbar::{Toolbar, ToolbarDef};
use crate::waveform::Waveform;
use std::sync::{Arc, RwLock, RwLockReadGuard};

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
        DisplayContext {
            colour_setup: None,
            colour_label: "Colour Setup unknown".into(),
            settings: Settings::new(),
            luma: crate::measurement::DEFAULT_LUMA_WEIGHTS,
        }
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
    /// The pointer moved here (with the left or middle button down, if it went down on the display).
    Move([f32; 2]),
    /// The left button went up.
    Up,
    /// The middle button went down here.
    MiddleDown([f32; 2]),
    /// The middle button went up.
    MiddleUp,
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

/// For tests: what a Loupe drew (the shared one in `loupe.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoupeDrawn {
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
    /// The settings the popover offers right now: [`Display::setting_defs`], less any choice that makes no sense
    /// for what is measured (nits on an Output Colour Space without light). The same by default.
    fn offered_defs(&self) -> Vec<SettingDef> {
        self.setting_defs().to_vec()
    }
    /// `stored` (the Panel's resolved settings) as the popover's sliders read them, for a display whose slider is
    /// in a unit other than the one it keeps. The same by default.
    fn popover_settings(&self, stored: &Settings) -> Settings {
        stored.clone()
    }
    /// What to keep for setting `key` to `value` from the popover (the inverse of [`Display::popover_settings`]).
    /// `value` by default.
    fn stored_value(&self, _stored: &Settings, _key: &str, value: &str) -> String {
        value.to_string()
    }
    /// The settings (from [`Display::setting_defs`]) the display shows as chips on its title bar. The popover
    /// holds the rest. None by default.
    fn toolbar_defs(&self) -> &'static [ToolbarDef] {
        &[]
    }
    /// The chips shown under `settings`, for a display whose chips depend on them (a subset of
    /// [`Display::toolbar_defs`]). The same by default.
    fn toolbar_defs_for(&self, _settings: &Settings) -> &'static [ToolbarDef] {
        self.toolbar_defs()
    }
    /// What the display remembers between sessions besides its menu choices (a Loupe's position and zoom). They
    /// reach [`Display::configure`] in the context's settings, and leave through [`PointerResult::remember`].
    fn state_keys(&self) -> &'static [&'static str] {
        &[]
    }
    /// A picture Panel's zoom (spec #163): the display itself, for a display that zooms (see
    /// [`crate::zoom::PictureZoom`]). The pointer's wheel and middle button, the zoom chip and Link all
    /// follow from it. `None` for a display that does not zoom (every Scope).
    fn picture_zoom(&self) -> Option<&dyn crate::zoom::PictureZoom> {
        None
    }
    /// The pointer over the display's `area` (see [`Display::draw`]). A zooming display's wheel and middle button
    /// zoom and pan; anything else is ignored by default.
    fn pointer(&self, area: [f32; 4], scale: f32, event: Pointer) -> PointerResult {
        self.picture_zoom()
            .and_then(|z| z.panel_zoom().pointer(z.geometry(area, scale), event))
            .unwrap_or_default()
    }
    /// What the zoom chip on the title bar says for a picture Panel whose content is `area` (spec #163): `None`
    /// for a display that does not zoom (every Scope), and before a frame has arrived.
    fn zoom(&self, area: [f32; 4], scale: f32) -> Option<crate::zoom::Readout> {
        let z = self.picture_zoom()?;
        z.panel_zoom().readout(z.geometry(area, scale))
    }
    /// Set the zoom, in percent of source pixels (see [`crate::zoom::ZoomModel::set_percent`]).
    /// Zoom is not a setting and is never saved.
    fn set_zoom(&self, area: [f32; 4], scale: f32, percent: f32) {
        if let Some(z) = self.picture_zoom() {
            z.panel_zoom().set_percent(percent, z.geometry(area, scale));
        }
    }
    /// The Fit toggle (Z, #169) on a picture Panel whose content is `area`; a display that does not zoom does
    /// nothing. Returns whether the view changed.
    fn toggle_fit(&self, area: [f32; 4], scale: f32) -> bool {
        self.picture_zoom()
            .is_some_and(|z| z.panel_zoom().toggle_fit(z.geometry(area, scale)))
    }
    /// Join the Window's Link (#168): the display set hands every display the one [`crate::zoom::Link`] on
    /// registering.
    fn link_to(&self, link: &crate::zoom::Link) {
        if let Some(z) = self.picture_zoom() {
            z.panel_zoom().link_to(link);
        }
    }
    /// The view of a picture Panel now (what Link takes when it is turned on from this Panel); `None` for a display
    /// that does not zoom.
    fn zoom_model(&self) -> Option<crate::zoom::ZoomModel> {
        self.picture_zoom().map(|z| z.panel_zoom().model())
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
}

/// One switch per display that can leave the Window: whether [`DisplaySet::standard`] registers it. A display
/// that is off keeps its module, its tests and its saved settings, so turning it back on is one word here.
pub struct InWindow {
    pub gamut_legal_warning: bool,
    pub highlight_neutrals: bool,
    pub cie_plot: bool,
    pub black_and_white: bool,
    pub colour_waveform: bool,
    pub channel_view: bool,
}

/// What the Window shows (ticket #95: the first three displays below are out of it for now; #118: the clean picture's
/// B/W toggle does the black and white Panel's job; #145: the Colour Waveform is out, Colorise on the luma waveform
/// does its job; #162: the Channel view is out, the clean picture's Y R G B chips and Loupe do its job).
pub const IN_WINDOW: InWindow = InWindow {
    gamut_legal_warning: false,
    highlight_neutrals: false,
    cie_plot: false,
    black_and_white: false,
    colour_waveform: false,
    channel_view: false,
};

/// The Scopes of [`DisplaySet::standard`] kept with their own types, so a test reads their counts through typed
/// methods (no untyped read-back on [`Display`]).
pub struct ScopeHandles {
    pub luma_waveform: Waveform,
    pub vectorscope: Arc<crate::vectorscope::Vectorscope>,
}

/// A Panel's stable id (ADR 0039): given when the Panel is created, never reused, and never follows the Panel's
/// position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelId(pub u64);

/// What makes a Panel an instance of a display (ADR 0039): its id, and its key, under which its settings,
/// remembered state and place in a saved layout are kept.
#[derive(Clone)]
struct Instance {
    id: PanelId,
    key: String,
    /// Makes another display of the same kind, for Duplicate (none for a display that cannot be copied).
    make: Option<Maker>,
}

/// Makes a new display of one kind, with its own state.
pub type Maker = Arc<dyn Fn() -> Arc<dyn Display> + Send + Sync>;

/// The key of the next instance of the display called `name`, given the instances in use (ADR 0039). The first
/// instance keeps the display's name, so everything saved before still applies; further ones are "Name 2",
/// "Name 3", the lowest number not in use.
fn instance_key(name: &str, taken: &[Instance]) -> String {
    let free = |key: &str| taken.iter().all(|i| i.key != key);
    if free(name) {
        return name.to_string();
    }
    (2..)
        .map(|n| format!("{name} {n}"))
        .find(|key| free(key))
        .expect("an unused number")
}

fn borrowed<'a>(
    panels: &'a [(String, &'static [SettingDef])],
) -> Vec<(&'a str, &'static [SettingDef])> {
    panels.iter().map(|(k, d)| (k.as_str(), *d)).collect()
}

/// Register the display `make` makes, with `make` to make another of it.
fn add(
    set: &mut DisplaySet,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    make: Box<dyn Fn(&wgpu::Device, &wgpu::Queue) -> Arc<dyn Display> + Send + Sync>,
) {
    let first = make(device, queue);
    let (d, q) = (device.clone(), queue.clone());
    set.register_with(first, Arc::new(move || make(&d, &q)));
}

/// Every display, in Panel order (the order of the grid, left to right then down).
///
/// Shared by clone, so the Window and the Feed thread always see the same Panels: a Panel made or removed from the
/// Window is measured by the Feed thread from the next frame on (see [`DisplaySet::generation`]).
#[derive(Clone, Default)]
pub struct DisplaySet {
    shared: Arc<RwLock<Shared>>,
    /// The Window's one Link: picture Panels zoom and pan together while it is on (#168).
    link: crate::zoom::Link,
}

#[derive(Default)]
struct Shared {
    displays: Vec<Arc<dyn Display>>,
    /// The instance each display is, in the same order as `displays`.
    instances: Vec<Instance>,
    next_id: u64,
    /// Goes up whenever a Panel is made or removed, or shown or hidden.
    generation: u64,
    /// Which Panels are hidden, by index (one not listed is shown): they are handed no frame.
    off: Vec<bool>,
}

impl DisplaySet {
    fn read(&self) -> RwLockReadGuard<'_, Shared> {
        self.shared.read().unwrap()
    }

    /// Say which Panels are shown (`shown`, in Panel order; a Panel past its end is shown). A hidden Panel (closed,
    /// or a tab that is not shown, ADR 0038) keeps its settings and state but is handed no frame: it costs nothing
    /// until shown again. A change bumps the generation, so the Feed thread measures the frame on screen again.
    pub fn set_shown(&self, shown: &[bool]) {
        let mut shared = self.shared.write().unwrap();
        let off: Vec<bool> = (0..shared.displays.len())
            .map(|i| !shown.get(i).copied().unwrap_or(true))
            .collect();
        if shared.off != off {
            shared.off = off;
            shared.generation += 1;
        }
    }

    /// Whether Panel `index` is shown (measures and draws).
    pub fn is_shown(&self, index: usize) -> bool {
        !self.read().off.get(index).copied().unwrap_or(false)
    }

    /// The displays the app shows, in Panel order. A new display is one module that implements [`Display`], and
    /// one line here.
    pub fn standard(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: wgpu::TextureFormat,
        filterable: bool,
    ) -> DisplaySet {
        Self::standard_with_handles(device, queue, target, filterable).0
    }

    /// [`DisplaySet::standard`], and the typed handles of its Scopes for tests.
    pub fn standard_with_handles(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: wgpu::TextureFormat,
        filterable: bool,
    ) -> (DisplaySet, ScopeHandles) {
        let mut set = DisplaySet::default();
        add(
            &mut set,
            device,
            queue,
            Box::new(move |d, q| {
                Arc::new(crate::picture_view::PictureView::new(
                    d, q, target, filterable,
                ))
            }),
        );
        let luma_waveform = {
            let first = Waveform::new(device, queue, target);
            let (d, q) = (device.clone(), queue.clone());
            set.register_with(
                Arc::new(first.clone()),
                Arc::new(move || Arc::new(Waveform::new(&d, &q, target))),
            );
            first
        };
        if IN_WINDOW.gamut_legal_warning {
            add(
                &mut set,
                device,
                queue,
                Box::new(move |d, q| {
                    Arc::new(crate::gamut_warning::GamutWarning::new(d, q, target))
                }),
            );
        }
        if IN_WINDOW.highlight_neutrals {
            add(
                &mut set,
                device,
                queue,
                Box::new(move |d, q| {
                    Arc::new(crate::highlight_neutrals::HighlightNeutrals::new(
                        d, q, target, filterable,
                    ))
                }),
            );
        }
        if IN_WINDOW.channel_view {
            add(
                &mut set,
                device,
                queue,
                Box::new(move |d, q| {
                    Arc::new(crate::channel_view::ChannelView::new(
                        d, q, target, filterable,
                    ))
                }),
            );
        }
        add(
            &mut set,
            device,
            queue,
            Box::new(move |d, q| {
                Arc::new(crate::arri_false_colour::ArriFalseColour::new(d, q, target))
            }),
        );
        if IN_WINDOW.black_and_white {
            add(
                &mut set,
                device,
                queue,
                Box::new(move |d, q| {
                    Arc::new(crate::black_and_white::BlackAndWhite::new(
                        d, q, target, filterable,
                    ))
                }),
            );
        }
        let vectorscope = Arc::new(crate::vectorscope::Vectorscope::new(device, queue, target));
        {
            let (d, q) = (device.clone(), queue.clone());
            set.register_with(
                vectorscope.clone(),
                Arc::new(move || Arc::new(crate::vectorscope::Vectorscope::new(&d, &q, target))),
            );
        }
        add(
            &mut set,
            device,
            queue,
            Box::new(move |d, q| {
                Arc::new(crate::saturation_heatmap::SaturationHeatmap::new(
                    d, q, target,
                ))
            }),
        );
        add(
            &mut set,
            device,
            queue,
            Box::new(move |d, q| {
                Arc::new(crate::skin_tone_view::SkinTone::new(
                    d, q, target, filterable,
                ))
            }),
        );
        add(
            &mut set,
            device,
            queue,
            Box::new(move |d, q| {
                Arc::new(crate::stops_false_colour::StopsFalseColour::new(
                    d, q, target,
                ))
            }),
        );
        if IN_WINDOW.cie_plot {
            add(
                &mut set,
                device,
                queue,
                Box::new(move |d, q| Arc::new(crate::cie::CiePlot::new(d, q, target))),
            );
        }
        if IN_WINDOW.colour_waveform {
            set.register(Arc::new(Waveform::new_colour(
                device,
                queue,
                target,
                &luma_waveform,
            )));
        }
        (
            set,
            ScopeHandles {
                luma_waveform,
                vectorscope,
            },
        )
    }

    /// Add a display after the others, as a new Panel: it gets an id of its own, and the key of its settings and
    /// state (the display's name for the first of its kind, "Name 2" and on for the next, ADR 0039). It cannot be
    /// duplicated; see [`DisplaySet::register_with`].
    pub fn register(&mut self, display: Arc<dyn Display>) -> PanelId {
        self.push(display, None)
    }

    /// [`DisplaySet::register`] for a display that `make` can make another of, which is what Duplicate and Replace
    /// use.
    pub fn register_with(&mut self, display: Arc<dyn Display>, make: Maker) -> PanelId {
        self.push(display, Some(make))
    }

    fn push(&self, display: Arc<dyn Display>, make: Option<Maker>) -> PanelId {
        display.link_to(&self.link);
        let mut shared = self.shared.write().unwrap();
        let id = PanelId(shared.next_id);
        shared.next_id += 1;
        let key = instance_key(display.name(), &shared.instances);
        shared.instances.push(Instance { id, key, make });
        shared.displays.push(display);
        shared.generation += 1;
        id
    }

    /// Another Panel of the same display as Panel `index`, after the others: its own display, with its own id and
    /// the next free number in its key ("Name 2"). Returns its index, or `None` for a display that cannot be copied.
    /// Its settings are the caller's to copy ([`PanelSettings::copy_panel`]).
    pub fn duplicate(&self, index: usize) -> Option<usize> {
        let make = self.read().instances.get(index)?.make.clone()?;
        let display = make();
        self.push(display, Some(make));
        Some(self.len() - 1)
    }

    /// A new Panel of the display called `name`, made like the first of its kind (for Replace with and Add when
    /// that display is already on the page). Returns its index, or `None` when there is no such display.
    pub fn make_named(&self, name: &str) -> Option<usize> {
        let template = self.read().displays.iter().position(|d| d.name() == name)?;
        self.duplicate(template)
    }

    /// Remove Panel `index`'s display for good (a duplicate that was closed): every Panel after it moves up one
    /// index, and its number is free again. Its settings are the caller's to forget.
    pub fn remove(&self, index: usize) {
        let mut shared = self.shared.write().unwrap();
        shared.displays.remove(index);
        shared.instances.remove(index);
        if index < shared.off.len() {
            shared.off.remove(index);
        }
        shared.generation += 1;
    }

    /// Whether Panel `index` is a further instance ("Name 2"), which is removed when closed; the first instance of
    /// a display stays, closed, so the Add list can bring it back.
    pub fn is_duplicate(&self, index: usize) -> bool {
        let shared = self.read();
        shared.instances[index].key != shared.displays[index].name()
    }

    /// Goes up whenever a Panel is made or removed, so the Feed thread knows to configure and measure again.
    pub fn generation(&self) -> u64 {
        self.read().generation
    }

    /// Panel `index`'s stable id.
    pub fn id(&self, index: usize) -> PanelId {
        self.read().instances[index].id
    }

    /// Every Panel's key, in Panel order: what settings, state and saved layouts name it by.
    pub fn keys(&self) -> Vec<String> {
        self.read()
            .instances
            .iter()
            .map(|i| i.key.clone())
            .collect()
    }

    /// Whether Link is on.
    pub fn link_on(&self) -> bool {
        self.link.on()
    }

    /// Switch the Window's Link. Turning it on from Panel `from` brings every picture Panel to that Panel's zoom
    /// and spot; turning it off leaves each where it is.
    pub fn set_link(&self, on: bool, from: usize) {
        self.link.set(
            on,
            self.read().displays.get(from).and_then(|d| d.zoom_model()),
        );
    }

    pub fn len(&self) -> usize {
        self.read().displays.len()
    }

    pub fn is_empty(&self) -> bool {
        self.read().displays.is_empty()
    }

    /// What each Panel's display is called (not its key: see [`DisplaySet::keys`]).
    pub fn names(&self) -> Vec<&'static str> {
        self.read().displays.iter().map(|d| d.name()).collect()
    }

    /// What each Panel's display is, for Auto resize (picture-shaped or not), in Panel order.
    pub fn shapes(&self) -> Vec<crate::auto_resize::Shape> {
        self.read()
            .displays
            .iter()
            .map(|d| crate::auto_resize::shape_of(d.name()))
            .collect()
    }

    /// What each Panel's display weighs and how it uses its cell, for Auto layout, in Panel order.
    pub fn items(&self) -> Vec<crate::auto_layout::Item> {
        self.read()
            .displays
            .iter()
            .map(|d| crate::auto_layout::item_of(d.name()))
            .collect()
    }

    pub fn labels(&self) -> Vec<String> {
        self.read().displays.iter().map(|d| d.label()).collect()
    }

    /// Every Panel's title bar: its name and short tags (only state that changes how to read it), with the full
    /// label for the hover (#99). `context` and `settings` are what [`DisplaySet::configure`] was given.
    pub fn titles(
        &self,
        context: &DisplayContext,
        settings: &PanelSettings,
    ) -> Vec<crate::window_text::PanelTitle> {
        let luma = crate::measurement::luma_weights_of(
            &settings.resolve(crate::waveform::NAME, &crate::waveform::SETTINGS),
        );
        let shared = self.read();
        shared
            .displays
            .iter()
            .zip(&shared.instances)
            .map(|(display, instance)| {
                let mut own = DisplayContext {
                    settings: settings.resolve(&instance.key, display.setting_defs()),
                    luma,
                    ..context.clone()
                };
                own.settings
                    .extend(settings.state_of(&instance.key, display.state_keys()));
                let mut title =
                    crate::window_text::panel_title(display.name(), &own, display.label());
                // A further instance shows its number ("Vectorscope 2").
                if let Some(number) = instance.key.strip_prefix(display.name()) {
                    title.name.push_str(number);
                }
                title
            })
            .collect()
    }

    /// Hand `frame` to every display.
    pub fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let shared = self.read();
        for (i, display) in shared.displays.iter().enumerate() {
            if !shared.off.get(i).copied().unwrap_or(false) {
                display.on_frame(encoder, frame);
            }
        }
    }

    /// Hand the Timeline Colour Space frame (before Output Conversion) to every display.
    pub fn on_timeline_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let shared = self.read();
        for (i, display) in shared.displays.iter().enumerate() {
            if !shared.off.get(i).copied().unwrap_or(false) {
                display.on_timeline_frame(encoder, frame);
            }
        }
    }

    /// Give every display the Colour Setup and its own settings (`settings` holds every Panel's choices).
    pub fn configure(&self, context: &DisplayContext, settings: &PanelSettings) {
        let luma = crate::measurement::luma_weights_of(
            &settings.resolve(crate::waveform::NAME, &crate::waveform::SETTINGS),
        );
        let shared = self.read();
        for (display, instance) in shared.displays.iter().zip(&shared.instances) {
            let mut own = DisplayContext {
                settings: settings.resolve(&instance.key, display.setting_defs()),
                luma,
                ..context.clone()
            };
            own.settings
                .extend(settings.state_of(&instance.key, display.state_keys()));
            display.configure(&own);
        }
    }

    fn setting_panels(&self) -> Vec<(String, &'static [SettingDef])> {
        let shared = self.read();
        shared
            .displays
            .iter()
            .zip(&shared.instances)
            .map(|(d, i)| (i.key.clone(), d.setting_defs()))
            .collect()
    }

    /// Whether every Panel's legend is on (the View menu's entry is marked, #191).
    pub fn legends_on(&self, settings: &PanelSettings) -> bool {
        let panels = self.setting_panels();
        settings.legends_on(&borrowed(&panels))
    }

    /// Set the legend of every Panel that has one, on or off, through its own Panel setting (#191).
    pub fn set_legends(&self, settings: &mut PanelSettings, on: bool) {
        let panels = self.setting_panels();
        settings.set_legends(&borrowed(&panels), on);
    }

    /// The View menu's entry and its hotkey: every legend off if they are all on, else every legend on.
    pub fn toggle_legends(&self, settings: &mut PanelSettings) {
        let on = !self.legends_on(settings);
        self.set_legends(settings, on);
    }

    /// The settings the display in Panel `index` offers.
    pub fn setting_defs(&self, index: usize) -> &'static [SettingDef] {
        self.read().displays[index].setting_defs()
    }

    /// The settings the display in Panel `index` leaves for the popover: those that are not chips on its title bar.
    pub fn popover_defs(&self, index: usize) -> Vec<SettingDef> {
        let shared = self.read();
        let display = &shared.displays[index];
        crate::toolbar::popover_defs(&display.offered_defs(), display.toolbar_defs())
    }

    /// The popover of the display in Panel `index` under its title bar `toolbar`: the settings of the chips that
    /// did not fit the bar first (#171), then those that are never chips.
    pub fn popover_for(&self, index: usize, toolbar: &Toolbar) -> Vec<SettingDef> {
        let shared = self.read();
        let display = &shared.displays[index];
        crate::toolbar::popover_with(
            &display.offered_defs(),
            display.toolbar_defs(),
            &toolbar.collapsed,
        )
    }

    /// The chips on the title bar of the display in Panel `index`, given everyone's choices. Zoom and Link are not
    /// chips: they are in the view strip ([`DisplaySet::view_strip`], ADR 0040).
    pub fn toolbar(&self, index: usize, settings: &PanelSettings) -> Toolbar {
        let own = self.settings_of(index, settings);
        Toolbar::new(self.read().displays[index].toolbar_defs_for(&own), &own)
    }

    /// What the zoom chip of the display in Panel `index` says, with its content in `area`.
    pub fn zoom(&self, index: usize, area: [f32; 4], scale: f32) -> Option<crate::zoom::Readout> {
        self.read().displays[index].zoom(area, scale)
    }

    /// The read-only zoom tag a picture Panel shows with its other tags ("Fit 17 %"); `None` for a Scope.
    pub fn zoom_tag(&self, index: usize, area: [f32; 4], scale: f32) -> Option<String> {
        self.zoom(index, area, scale).map(|r| r.text)
    }

    /// The picture Panel Link takes the view of when the strip turns it on (there is no clicked Panel any more):
    /// the first one zoomed in, else the first one shown (`contents` is each Panel's content area, in Panel order).
    pub fn first_picture_panel(&self, contents: &[[f32; 4]], scale: f32) -> Option<usize> {
        let readouts: Vec<_> = contents
            .iter()
            .zip(&self.read().displays)
            .map(|(area, d)| d.zoom(*area, scale))
            .collect();
        readouts
            .iter()
            .position(|r| r.as_ref().is_some_and(|r| r.percent > r.fit + 0.01))
            .or_else(|| readouts.iter().position(Option::is_some))
    }

    /// The view strip as it reads now (ADR 0040): enabled while a picture Panel is shown; the readout is the shared
    /// zoom while Link is on ("Fit" or "200 %"), and while it is off the one zoom every picture Panel has, or
    /// "Mixed" when they differ.
    pub fn view_strip(&self, contents: &[[f32; 4]], scale: f32) -> crate::view_strip::ViewStrip {
        // Fit, or the zoom rounded to a percent: what the readout can say about one Panel.
        let says: Vec<String> = contents
            .iter()
            .zip(&self.read().displays)
            .filter_map(|(area, d)| d.zoom(*area, scale))
            .map(|r| {
                if r.percent > r.fit + 0.01 {
                    format!("{} %", r.percent.round())
                } else {
                    "Fit".to_string()
                }
            })
            .collect();
        let link = self.link.on();
        let readout = match says.first() {
            None => String::new(),
            Some(first) if link || says.iter().all(|s| s == first) => first.clone(),
            Some(_) => "Mixed".to_string(),
        };
        crate::view_strip::ViewStrip {
            enabled: !says.is_empty(),
            readout,
            link,
        }
    }

    /// The strip's Link: switch the Window's Link; turning it on takes the view of [`DisplaySet::first_picture_panel`].
    pub fn set_link_from_strip(&self, on: bool, contents: &[[f32; 4]], scale: f32) {
        self.set_link(on, self.first_picture_panel(contents, scale).unwrap_or(0));
    }

    /// The strip's Fit: every picture Panel back to Fit (`contents` is each Panel's content area, in Panel order).
    pub fn fit_all(&self, contents: &[[f32; 4]], scale: f32) {
        for (area, d) in contents.iter().zip(&self.read().displays) {
            d.set_zoom(*area, scale, 0.0);
        }
    }

    /// The Fit toggle (Z, #169) on the display in Panel `index`: Fit and back to the last zoom and spot; every
    /// picture Panel while Link is on, nothing over a Scope. Returns whether anything changed.
    pub fn toggle_fit(&self, index: usize, area: [f32; 4], scale: f32) -> bool {
        self.read().displays[index].toggle_fit(area, scale)
    }

    /// Set the zoom of the display in Panel `index` to `percent` (tests set a zoom this way; 0 is Fit).
    pub fn set_zoom(&self, index: usize, area: [f32; 4], scale: f32, percent: f32) {
        self.read().displays[index].set_zoom(area, scale, percent);
    }

    /// The settings of the display in Panel `index` as its popover shows them.
    pub fn popover_settings(&self, index: usize, settings: &PanelSettings) -> Settings {
        let own = self.settings_of(index, settings);
        self.read().displays[index].popover_settings(&own)
    }

    /// What to keep for Panel `index`'s `key` set to `value` from the popover or a chip.
    pub fn stored_value(
        &self,
        index: usize,
        settings: &PanelSettings,
        key: &str,
        value: &str,
    ) -> String {
        let own = self.settings_of(index, settings);
        self.read().displays[index].stored_value(&own, key, value)
    }

    /// The settings the display in Panel `index` is using, given everyone's choices.
    pub fn settings_of(&self, index: usize, settings: &PanelSettings) -> Settings {
        let shared = self.read();
        settings.resolve(
            &shared.instances[index].key,
            shared.displays[index].setting_defs(),
        )
    }

    /// What the display in Panel `index` is called.
    pub fn name(&self, index: usize) -> &'static str {
        self.read().displays[index].name()
    }

    /// Panel `index`'s key: what its settings and remembered state are kept under (the display's name for the
    /// first of its kind, "Name 2" and on for the next, ADR 0039).
    pub fn key(&self, index: usize) -> String {
        self.read().instances[index].key.clone()
    }

    pub fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        index: usize,
        area: [f32; 4],
        surface: [f32; 2],
        scale: f32,
    ) {
        let shared = self.read();
        // A hidden Panel (another page, a closed Panel, a tab not shown) draws nothing.
        if !shared.off.get(index).copied().unwrap_or(false) {
            shared.displays[index].draw(pass, area, surface, scale);
        }
    }

    pub fn scale_texts(&self, index: usize, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        self.read().displays[index].scale_texts(area, scale)
    }

    /// The pointer over the display in Panel `index`.
    pub fn pointer(
        &self,
        index: usize,
        area: [f32; 4],
        scale: f32,
        event: Pointer,
    ) -> PointerResult {
        self.read().displays[index].pointer(area, scale, event)
    }

    /// Whether the display in Panel `index`, with its content in `area`, wants the scroll wheel: it has state to move
    /// (a Loupe) or it zooms.
    pub fn takes_wheel(&self, index: usize, area: [f32; 4], scale: f32) -> bool {
        self.takes_pointer(index) || self.zoom(index, area, scale).is_some()
    }

    /// Whether the display in Panel `index` wants the pointer (it has state to move).
    pub fn takes_pointer(&self, index: usize) -> bool {
        !self.read().displays[index].state_keys().is_empty()
    }
}
