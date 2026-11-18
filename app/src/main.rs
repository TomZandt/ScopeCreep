//! The scopecreep app: one window holding a grid of Panels, one per display (ADR 0011): the Picture View of the
//! latest Feed frame and the luma waveform for now, each with a quiet label under it, and a quiet status line
//! along the bottom (Resolve's playback rate, dropped frames, the Sender driving the Feed). The borders between
//! Panels can be dragged and are remembered. Every display shows the Feed after Output Conversion, in the Output
//! Colour Space, whenever the Colour Setup has one (`output_conversion.rs`). Right-click a Panel for its settings,
//! which are remembered per Panel (`~/.config/scopecreep/panel-settings`).
//!
//!   scopecreep [--feed <path>] [--no-scripting]
//!
//!   --feed           default: $SCOPECREEP_FEED, else /dev/shm/scopecreep-feed
//!   --no-scripting   never read the project by Resolve scripting (the Colour Setup then comes only from the
//!                    Sender node's manual override). Resolve's Python: $SCOPECREEP_RESOLVE_PYTHON, else
//!                    /opt/resolve/bin/ResolvePython.
//!
//! Two threads. The Feed thread polls the Feed every millisecond (the ring is short and the Sender never waits),
//! puts the frames to show on the GPU once, converts them once, hands that one frame to every display, and wakes
//! the window. The window thread only draws.
use glyphon::{Attrs, Buffer, Cache, Color, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
use scopecreep::display::{DisplayContext, DisplaySet, Pointer};
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::layout::MARGIN;
use scopecreep::layouts::{self, Action, Click, Layouts, Rename};
use scopecreep::monitor::{FeedState, Monitor};
use scopecreep::output_conversion::OutputConversion;
use scopecreep::panel_grid::{Border, PanelGrid, PanelLayout, Press, min_panel_size};
use scopecreep::resolve_helper::ScriptHelper;
use scopecreep::flat_rects::FlatRects;
use scopecreep::components::{self, Align, BarKind, Prim, Tooltip};
use scopecreep::window_text::{self, PanelTitle};
use scopecreep::settings::{Menu, PanelSettings};
use scopecreep::style;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{CursorIcon, Window, WindowId};

/// Neutral, dark, and the same on every channel: nothing around the picture may tint the eye.
/// Chrome colours and sizes come from the style tokens (ADR 0026); this gives text colours to glyphon.
fn text_colour(c: [u8; 3]) -> Color {
    Color::rgb(c[0], c[1], c[2])
}

/// What the labels say, as the Feed thread last saw it.
#[derive(Clone, Default, PartialEq)]
struct Texts {
    /// Each Panel's title bar (name, tags, and the full label for its hover), in Panel order.
    titles: Vec<PanelTitle>,
    /// The Colour Setup line across the top.
    top_bar: String,
    /// The status line's choices, longest first (the window picks what fits), and its hover.
    status: Vec<String>,
    status_hover: String,
    notice: Option<String>,
    has_picture: bool,
}

struct Wake;

/// What the pointer rests on that shows a hover: a Panel's title bar, or the status line.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hover {
    Title(usize),
    Status,
}

/// Where the border positions are remembered: `$XDG_CONFIG_HOME/scopecreep/panel-grid`, else
/// `~/.config/scopecreep/panel-grid`.
fn grid_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("scopecreep").join("panel-grid"))
}

/// Where the Panels' settings are remembered, next to the border positions.
fn settings_file() -> Option<PathBuf> {
    grid_file().map(|p| p.with_file_name("panel-settings"))
}

/// Where the named layouts are remembered, next to the border positions.
fn layouts_file() -> Option<PathBuf> {
    grid_file().map(|p| p.with_file_name("layouts"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut feed = scopecreep_feed::default_path();
    let mut scripting = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--feed" => feed = args.next().ok_or("--feed needs a path")?.into(),
            "--no-scripting" => scripting = false,
            "--version" => {
                println!("scopecreep {} (Feed protocol {}.{})", env!("CARGO_PKG_VERSION"), scopecreep_feed::VERSION_MAJOR, scopecreep_feed::VERSION_MINOR);
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}. Usage: scopecreep [--feed <path>] [--no-scripting]").into()),
        }
    }
    let event_loop = EventLoop::<Wake>::with_user_event().build()?;
    let mut app = App { feed, scripting, proxy: event_loop.create_proxy(), window: None };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    feed: PathBuf,
    scripting: bool,
    proxy: EventLoopProxy<Wake>,
    window: Option<AppWindow>,
}

/// How a piece of text is set: everything besides the words and the room that changes how it is shaped.
#[derive(Clone, Copy, PartialEq)]
struct TextStyle {
    /// Logical pixels.
    size: f32,
    /// Line height as a multiple of `size`.
    line: f32,
    align: Option<glyphon::cosmic_text::Align>,
    wrap: bool,
}

impl TextStyle {
    /// The quiet labels, the status line and the menu.
    const LABEL: TextStyle = TextStyle { size: style::TYPE_LABEL, line: style::TYPE_LABEL_LINE, align: None, wrap: true };
    /// A notice on its own over the Panel.
    const NOTICE: TextStyle = TextStyle { size: style::TYPE_NOTICE, line: style::TYPE_NOTICE_LINE, align: Some(glyphon::cosmic_text::Align::Center), wrap: true };
}

/// A piece of text kept shaped until what it says, how it is set, or the room it has, changes.
struct Text {
    buffer: Buffer,
    key: Option<(String, u32, u32, TextStyle)>,
}

impl Text {
    fn new(font_system: &mut FontSystem) -> Text {
        Text { buffer: Buffer::new(font_system, Metrics::new(style::TYPE_LABEL, style::TYPE_LABEL * style::TYPE_LABEL_LINE)), key: None }
    }

    /// Shape `text` in `style` (at `scale`) in `width` device pixels, unless it already is.
    fn set(&mut self, font_system: &mut FontSystem, text: &str, style: TextStyle, scale: f32, width: f32) {
        let width = width.round() as u32;
        if self.key.as_ref().is_some_and(|(t, w, s, st)| t == text && *w == width && *s == scale.to_bits() && *st == style) {
            return;
        }
        let attrs = Attrs::new().family(Family::SansSerif);
        self.buffer.set_metrics(Metrics::new(style.size * scale, style.size * scale * style.line));
        self.buffer.set_size(Some((width as f32).max(1.0)), None);
        self.buffer.set_wrap(if style.wrap { glyphon::cosmic_text::Wrap::WordOrGlyph } else { glyphon::cosmic_text::Wrap::None });
        self.buffer.set_text(text, &attrs, Shaping::Advanced, style.align);
        self.buffer.shape_until_scroll(font_system, false);
        self.key = Some((text.to_string(), width, scale.to_bits(), style));
    }
}

/// What a Panel's text is made of: its label (one line, in the title bar), and the numbers on its display's scale.
struct PanelText {
    label: Text,
    tags: Text,
    scale: Vec<Text>,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    displays: DisplaySet,
    grid: PanelGrid,
    /// Every Panel's settings, shared with the Feed thread (which gives them to the displays).
    settings: Arc<Mutex<PanelSettings>>,
    /// The right-click menu, while open, with where it opened.
    menu: Option<(Menu, [f32; 2])>,
    /// The saved layouts (#98), and whether the open menu is their menu (opened by a right-click on the status
    /// line) and not a Panel's settings.
    layouts: Layouts,
    layout_menu: bool,
    /// A layout's name being typed in the layout menu.
    renaming: Option<Rename>,
    modifiers: ModifiersState,
    flat: FlatRects,
    menu_renderer: TextRenderer,
    menu_texts: Vec<Text>,
    /// Where each Panel was last drawn (device pixels), for a right-click to find its Panel in.
    cells: Vec<[f32; 4]>,
    /// Where each Panel's display was last drawn (device pixels), for the pointer to hand to displays that use it.
    contents: Vec<[f32; 4]>,
    /// The Panel whose display holds the pointer (a Loupe being dragged), if any.
    pointer_panel: Option<usize>,
    /// The border being dragged, if any.
    drag: Option<Border>,
    /// The Panel being dragged by its title bar, to swap with the one it is let go on.
    panel_drag: Option<usize>,
    /// Remembered display state (a Loupe's position) changed and is not written yet.
    state_unsaved: bool,
    cursor: [f32; 2],
    /// Where the grid and its gaps were last drawn (device pixels), for the pointer to find borders in.
    grid_area: [f32; 4],
    grid_gap: f32,
    scale: f32,
    texts: Arc<Mutex<Texts>>,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    panel_texts: Vec<PanelText>,
    status: Text,
    top_bar: Text,
    notice: Text,
    /// What the pointer rests on that shows a hover (#99).
    hover: Option<Hover>,
    /// Each Panel's title bar and the status line as last drawn (device pixels), and the text a hover shows for each.
    title_bars: Vec<[f32; 4]>,
    title_hovers: Vec<String>,
    status_bar: [f32; 4],
    status_hover: String,
    stop: Arc<AtomicBool>,
    feed_thread: Option<std::thread::JoinHandle<()>>,
    // Last, so it outlives the surface made from it.
    window: Arc<Window>,
}

impl AppWindow {
    fn new(window: Arc<Window>, event_loop: &ActiveEventLoop, feed: PathBuf, scripting: bool, proxy: EventLoopProxy<Wake>) -> Result<AppWindow, Box<dyn std::error::Error>> {
        // WGPU_BACKEND and friends are honoured, for trying another graphics backend.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(event_loop.owned_display_handle())));
        let surface = instance.create_surface(window.clone()).map_err(|e| format!("no drawing surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no usable GPU: {e}"))?;
        let filterable = adapter.features().contains(wgpu::Features::FLOAT32_FILTERABLE);
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("scopecreep"),
            required_features: if filterable { wgpu::Features::FLOAT32_FILTERABLE } else { wgpu::Features::empty() },
            // Resolve fills most of the graphics memory (Stage 0: 7.7 of 8 GB): take only what is used.
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| format!("{} ({:?}) would not give a device: {e}", info.name, info.backend))?;
        device.on_uncaptured_error(Arc::new(|e| eprintln!("scopecreep: graphics error: {e}")));
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        // Not an sRGB surface: the Feed's values are shown as they are, with no extra encoding.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).ok_or("the window surface is not supported by the GPU")?;
        config.format = format;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let displays = DisplaySet::standard(&device, &queue, format, filterable);
        let settings = Arc::new(Mutex::new(settings_file().and_then(|path| std::fs::read_to_string(path).ok()).map_or_else(PanelSettings::default, |text| PanelSettings::from_text(&text))));
        displays.configure(&DisplayContext::default(), &settings.lock().unwrap());
        let names = displays.names();
        let grid = grid_file().and_then(|path| std::fs::read_to_string(path).ok()).map_or_else(|| PanelGrid::default_for(&names), |text| PanelGrid::from_text(&names, &text));
        let mut font_system = FontSystem::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, if format.is_srgb() { ColorMode::Accurate } else { ColorMode::Web });
        let text_renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let menu_renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let flat = FlatRects::new(&device, &queue, format);
        let status = Text::new(&mut font_system);
        let top_bar = Text::new(&mut font_system);
        let notice = Text::new(&mut font_system);
        let panel_texts = (0..displays.len()).map(|_| PanelText { label: Text::new(&mut font_system), tags: Text::new(&mut font_system), scale: Vec::new() }).collect();

        let texts = Arc::new(Mutex::new(Texts::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let feed_thread = {
            let (texts, stop, displays, settings) = (texts.clone(), stop.clone(), displays.clone(), settings.clone());
            let mut uploader = FrameUploader::new(&device, &queue);
            let mut conversion = OutputConversion::new(&device, &queue);
            std::thread::Builder::new().name("feed".into()).spawn(move || {
                // The Resolve scripting helper is owned by this thread's Monitor: it is killed when the app quits.
                let mut monitor = if scripting { Monitor::with_project_reader(&feed, Box::new(ScriptHelper::from_env())) } else { Monitor::new(&feed) };
                // SCOPECREEP_FRAME_LOG=<file>: one line per Feed frame read and shown, with the Sender's timing (diagnostics).
                if let Some(path) = std::env::var_os("SCOPECREEP_FRAME_LOG").filter(|p| !p.is_empty()) {
                    match std::fs::File::create(&path) {
                        Ok(f) => monitor.set_trace(Box::new(std::io::LineWriter::new(f))),
                        Err(e) => eprintln!("scopecreep: cannot write the frame log: {e}"),
                    }
                }
                // Frames put on the GPU and handed to the displays.
                let mut scope_frames = 0u64;
                // Displays measure in the Colour Setup's Output Colour Space (the default Measured Space).
                let mut context = DisplayContext::default();
                let mut chosen = settings.lock().unwrap().clone();
                let (mut seen_version, mut seen_state_version) = (chosen.version(), chosen.state_version());
                // What the Feed's status said when last looked at, and the texts made from it.
                let mut seen: Option<scopecreep::monitor::Status> = None;
                let (mut status_text, mut status_hover, mut top_bar, mut notice, mut has_picture) = (Vec::new(), String::new(), String::new(), None, false);
                let mut last = Texts::default();
                // SCOPECREEP_STATUS_LOG=1: one line a second on stderr with what the app made of the Feed (diagnostics).
                let status_log = std::env::var_os("SCOPECREEP_STATUS_LOG").is_some_and(|v| v != "0");
                let mut next_log = scopecreep_feed::now_ns();
                while !stop.load(Ordering::Relaxed) {
                    if status_log && scopecreep_feed::now_ns() >= next_log {
                        next_log += 1_000_000_000;
                        let s = monitor.status();
                        let fed = s.feed_stats.map_or(0, |f| f.frames_published);
                        eprintln!(
                            "status feed={:?} fed={} viewer_frames={} scope_frames={} repeats={} cache_ignored={} dropped={} frame={:?} project={:?} timeline={:?} colour={:?} project_state={:?} reads={} rate={} line={:?} notice={:?}",
                            s.feed,
                            fed,
                            s.viewer_frames,
                            scope_frames,
                            s.repeat_renders,
                            s.cache_renders_ignored,
                            s.dropped,
                            s.picture.as_ref().map(|p| p.frame_time),
                            s.project.as_ref().map(|p| p.project.as_str()),
                            s.project.as_ref().and_then(|p| p.timeline.as_deref()),
                            s.colour_setup().map(|c| c.label()),
                            s.project_state,
                            s.project_reads,
                            s.playback_rate.map_or("-".to_string(), |r| format!("{r:.3}")),
                            s.status_line(),
                            s.notice()
                        );
                    }
                    let mut shown = false;
                    monitor.poll(scopecreep_feed::now_ns(), &mut |frame| {
                        uploader.upload(frame, |encoder, gpu_frame| {
                            // Every display sees the picture after Output Conversion: one upload, one conversion.
                            let picture = conversion.convert(encoder, gpu_frame);
                            displays.on_frame(encoder, picture);
                            // Stops False Colour measures scene light: the frame before Output Conversion.
                            displays.on_timeline_frame(encoder, gpu_frame);
                            scope_frames += 1;
                        });
                        shown = true;
                    });
                    // Idle costs a comparison, nothing more: the slower work below happens only when something it
                    // depends on changed (the Feed's status, a menu choice, remembered state, a frame shown).
                    let mut labels_stale = shown;
                    let status = monitor.status();
                    let status_changed = seen.as_ref() != Some(status);
                    if status_changed {
                        seen = Some(status.clone());
                        labels_stale = true;
                        let connected = status.feed == FeedState::Connected;
                        status_text = if connected { status.status_candidates() } else { Vec::new() };
                        status_hover = if connected { status.status_hover() } else { String::new() };
                        top_bar = status.top_bar();
                        notice = status.notice();
                        has_picture = status.picture.is_some();
                        let now_context = status.display_context();
                        if now_context != context {
                            context = now_context;
                            displays.configure(&context, &chosen);
                        }
                    }
                    // A setting changed in a menu measures the frame on screen again, even with Resolve stopped.
                    let (version, state_version) = {
                        let guard = settings.lock().unwrap();
                        (guard.version(), guard.state_version())
                    };
                    if state_version != seen_state_version {
                        seen_state_version = state_version;
                        labels_stale = true;
                    }
                    if version != seen_version {
                        seen_version = version;
                        chosen = settings.lock().unwrap().clone();
                        displays.configure(&context, &chosen);
                        let on_screen = uploader.frame();
                        conversion.again(on_screen, |encoder, picture| {
                            displays.on_frame(encoder, picture);
                            if let Some(t) = on_screen {
                                displays.on_timeline_frame(encoder, t);
                            }
                        });
                        shown = true;
                        labels_stale = true;
                    }
                    // A Colour Setup that arrives or changes while Resolve is stopped converts the frame on screen.
                    if status_changed {
                        let setup = status.colour_setup().map(|c| c.setup);
                        let on_screen = uploader.frame();
                        if conversion.follow(setup.as_ref(), on_screen, |encoder, picture| {
                            displays.on_frame(encoder, picture);
                            if let Some(t) = on_screen {
                                displays.on_timeline_frame(encoder, t);
                            }
                        }) {
                            shown = true;
                            labels_stale = true;
                        }
                    }
                    if labels_stale {
                        let now = Texts { titles: displays.titles(&context, &chosen), top_bar: top_bar.clone(), status: status_text.clone(), status_hover: status_hover.clone(), notice: notice.clone(), has_picture };
                        if now != last {
                            *texts.lock().unwrap() = now.clone();
                            last = now;
                            shown = true;
                        }
                    }
                    if shown && proxy.send_event(Wake).is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                // `monitor` is dropped here: the app detaches from the Feed and the Sender stops copying.
            })?
        };
        Ok(AppWindow {
            device,
            queue,
            surface,
            config,
            displays,
            grid,
            settings,
            menu: None,
            layouts: layouts_file().and_then(|path| std::fs::read_to_string(path).ok()).map_or_else(Layouts::default, |text| Layouts::from_text(&text)),
            layout_menu: false,
            renaming: None,
            modifiers: ModifiersState::empty(),
            flat,
            menu_renderer,
            menu_texts: Vec::new(),
            cells: Vec::new(),
            contents: Vec::new(),
            pointer_panel: None,
            drag: None,
            panel_drag: None,
            state_unsaved: false,
            cursor: [0.0, 0.0],
            grid_area: [0.0; 4],
            grid_gap: 0.0,
            scale: 1.0,
            texts,
            font_system,
            swash_cache: SwashCache::new(),
            viewport,
            atlas,
            text_renderer,
            panel_texts,
            status,
            top_bar,
            notice,
            hover: None,
            title_bars: Vec::new(),
            title_hovers: Vec::new(),
            status_bar: [0.0; 4],
            status_hover: String::new(),
            stop,
            feed_thread: Some(feed_thread),
            window,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }

    /// What a press at the pointer would start: a border drag or a Panel drag.
    fn press_under_pointer(&self) -> Option<Press> {
        self.grid.press_at(self.grid_area, self.grid_gap, self.cursor, style::SPACE_GRAB * self.scale, self.scale)
    }

    /// Remember the layout.
    fn save_grid(&self) {
        if let Some(path) = grid_file() {
            if let Err(e) = scopecreep::settings::save(&path, &self.grid.to_text(&self.displays.names())) {
                eprintln!("scopecreep: cannot remember the Panel layout: {e}");
            }
        }
    }

    /// Rest the hover on what is under the pointer: a Panel's title bar or the status line (never while a drag or
    /// a Loupe holds the pointer).
    fn update_hover(&mut self) {
        let busy = self.pointer_panel.is_some() || self.drag.is_some() || self.panel_drag.is_some();
        let hover = if busy {
            None
        } else if components::contains(self.status_bar, self.cursor) {
            Some(Hover::Status)
        } else {
            self.title_bars.iter().position(|t| components::contains(*t, self.cursor)).map(Hover::Title)
        };
        if hover != self.hover {
            self.hover = hover;
            self.window.request_redraw();
        }
    }

    fn pointer_moved(&mut self, x: f32, y: f32) {
        self.cursor = [x, y];
        self.update_hover();
        if let Some(panel) = self.pointer_panel {
            self.display_pointer(panel, Pointer::Move(self.cursor));
        } else if let Some(border) = self.drag {
            self.grid.drag(border, self.cursor, self.grid_area, self.grid_gap, min_panel_size(self.scale));
            self.window.request_redraw();
        } else if self.panel_drag.is_some() {
            self.window.request_redraw();
        } else {
            self.window.set_cursor(match self.press_under_pointer() {
                Some(Press::Border(Border::Column { .. })) => CursorIcon::ColResize,
                Some(Press::Border(Border::Row { .. })) => CursorIcon::RowResize,
                Some(Press::Panel(_)) => CursorIcon::Grab,
                None => CursorIcon::Default,
            });
        }
    }

    /// Right button: open the menu of the Panel under the pointer (none for a Panel without settings, or off a
    /// Panel), or close the one that is open.
    fn right_click(&mut self) {
        // In the layout menu, a right-click on a saved layout starts renaming it.
        if self.layout_menu {
            if let (Some((menu, _)), Some(rect)) = (self.menu.as_ref(), self.menu_rect()) {
                if let Some((_, value)) = menu.choice_at(rect, self.cursor, self.scale) {
                    if let Some(Action::Rename(slot)) = layouts::action(&self.layouts, value, Click::Right) {
                        self.renaming = Rename::start(&self.layouts, slot);
                        self.show_layout_menu();
                        return;
                    }
                }
            }
        }
        let was_open = self.menu.take().is_some();
        self.layout_menu = false;
        self.renaming = None;
        // On the status line: the layout menu.
        if !was_open && self.cursor[1] >= self.grid_area[1] + self.grid_area[3] {
            self.layout_menu = true;
            self.menu = Some((layouts::menu(&self.layouts, None), self.cursor));
            self.window.request_redraw();
            return;
        }
        let hit = self.cells.iter().position(|c| Menu::contains(*c, self.cursor));
        if let Some(panel) = hit {
            let current = self.displays.settings_of(panel, &self.settings.lock().unwrap());
            self.menu = Menu::new(panel, self.displays.setting_defs(panel), &current).map(|menu| (menu, self.cursor));
        }
        if was_open || self.menu.is_some() {
            self.window.request_redraw();
        }
    }

    /// Draw the layout menu again from the saved layouts, where it opened.
    fn show_layout_menu(&mut self) {
        if let Some((menu, _)) = self.menu.as_mut() {
            *menu = layouts::menu(&self.layouts, self.renaming.as_ref());
        }
        self.window.request_redraw();
    }

    /// Remember the named layouts.
    fn save_layouts(&self) {
        if let Some(path) = layouts_file() {
            if let Err(e) = scopecreep::settings::save(&path, &self.layouts.to_text()) {
                eprintln!("scopecreep: cannot remember the layouts: {e}");
            }
        }
    }

    /// Do one layout action: show a saved layout or the default, or save the arrangement on screen into a slot.
    fn layout_action(&mut self, action: Action) {
        let names = self.displays.names();
        match action {
            Action::Default => self.grid = PanelGrid::default_for(&names),
            Action::Recall(slot) => match self.layouts.recall(slot, &names) {
                Some(grid) => self.grid = grid,
                None => return,
            },
            Action::Save(slot) => {
                self.layouts.save(slot, &self.grid, &names);
                self.save_layouts();
                return;
            }
            Action::Rename(_) => return,
        }
        self.save_grid();
        self.window.request_redraw();
    }

    /// Left button in the layout menu: a click does what the row says; any click ends a rename being typed.
    fn layout_menu_click(&mut self) {
        let (Some((menu, _)), Some(rect)) = (self.menu.as_ref(), self.menu_rect()) else { return };
        let choice = menu.choice_at(rect, self.cursor, self.scale);
        self.finish_rename(true);
        let click = if self.modifiers.shift_key() { Click::Shift } else { Click::Plain };
        if let Some(action) = choice.and_then(|(_, value)| layouts::action(&self.layouts, value, click)) {
            self.layout_action(action);
            self.close_menu();
        } else if choice.is_none() && Menu::contains(rect, self.cursor) {
            self.show_layout_menu();
        } else {
            self.close_menu();
        }
    }

    fn close_menu(&mut self) {
        self.menu = None;
        self.layout_menu = false;
        self.renaming = None;
        self.window.request_redraw();
    }

    /// End a rename being typed: keep the name (`accept`) or drop it.
    fn finish_rename(&mut self, accept: bool) {
        if let Some(rename) = self.renaming.take() {
            if accept && rename.accept(&mut self.layouts) {
                self.save_layouts();
            }
        }
    }

    /// A key: Ctrl+1 to 9 recalls a layout and Ctrl+Shift+1 to 9 saves one (the Window is still, nothing shows);
    /// while a layout name is being typed, the keys type it.
    fn key(&mut self, event: &winit::event::KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        if self.renaming.is_some() {
            match &event.logical_key {
                Key::Named(NamedKey::Enter) => {
                    self.finish_rename(true);
                    self.show_layout_menu();
                }
                Key::Named(NamedKey::Escape) => {
                    self.renaming = None;
                    self.show_layout_menu();
                }
                Key::Named(NamedKey::Backspace) => {
                    if let Some(r) = self.renaming.as_mut() {
                        r.backspace();
                    }
                    self.show_layout_menu();
                }
                _ => {
                    if let (Some(r), Some(text)) = (self.renaming.as_mut(), event.text.as_ref()) {
                        r.type_text(text);
                        self.show_layout_menu();
                    }
                }
            }
            return;
        }
        if event.logical_key == Key::Named(NamedKey::Escape) && self.menu.is_some() {
            self.close_menu();
            return;
        }
        let digit = match event.physical_key {
            PhysicalKey::Code(KeyCode::Digit1) => Some(1),
            PhysicalKey::Code(KeyCode::Digit2) => Some(2),
            PhysicalKey::Code(KeyCode::Digit3) => Some(3),
            PhysicalKey::Code(KeyCode::Digit4) => Some(4),
            PhysicalKey::Code(KeyCode::Digit5) => Some(5),
            PhysicalKey::Code(KeyCode::Digit6) => Some(6),
            PhysicalKey::Code(KeyCode::Digit7) => Some(7),
            PhysicalKey::Code(KeyCode::Digit8) => Some(8),
            PhysicalKey::Code(KeyCode::Digit9) => Some(9),
            _ => None,
        };
        let m = self.modifiers;
        if let Some(shortcut) = digit.and_then(|d| layouts::shortcut(d, m.control_key(), m.shift_key(), m.alt_key())) {
            let action = match shortcut {
                layouts::Shortcut::Recall(slot) => Action::Recall(slot),
                layouts::Shortcut::Save(slot) => Action::Save(slot),
            };
            self.layout_action(action);
        }
    }

    fn menu_rect(&self) -> Option<[f32; 4]> {
        let (menu, at) = self.menu.as_ref()?;
        Some(menu.rect(*at, [self.config.width as f32, self.config.height as f32], self.scale))
    }

    /// Left button while a menu is open: a click on a choice picks it (and remembers it); a click on a heading or
    /// the menu's padding leaves it open; any other click closes it.
    fn menu_click(&mut self) {
        if self.layout_menu {
            return self.layout_menu_click();
        }
        let (Some((menu, _)), Some(rect)) = (self.menu.as_ref(), self.menu_rect()) else { return };
        let choice = menu.choice_at(rect, self.cursor, self.scale);
        if let Some((key, value)) = choice {
            let name = self.displays.name(menu.panel);
            let changed = {
                let mut settings = self.settings.lock().unwrap();
                let before = settings.version();
                settings.set(name, key, value);
                settings.version() != before
            };
            if changed {
                self.save_settings();
            }
        }
        if choice.is_none() && Menu::contains(rect, self.cursor) {
            return;
        }
        self.menu = None;
        self.window.request_redraw();
    }

    /// Write the Panel settings to their file. The text is made under the lock and written after it is let go, so
    /// the Feed thread never waits for the disk.
    fn save_settings(&self) {
        let Some(path) = settings_file() else { return };
        let text = self.settings.lock().unwrap().to_text();
        if let Err(e) = scopecreep::settings::save(&path, &text) {
            eprintln!("scopecreep: cannot remember the Panel settings: {e}");
        }
    }

    /// Give a display the pointer: what it asks to remember is kept with the Panel settings (and written to the
    /// file once the movement is done), and the window draws again. Returns whether the display took the event.
    fn display_pointer(&mut self, panel: usize, event: Pointer) -> bool {
        let Some(area) = self.contents.get(panel).copied() else { return false };
        let result = self.displays.pointer(panel, area, self.scale, event);
        if result.used {
            let name = self.displays.name(panel);
            {
                let mut settings = self.settings.lock().unwrap();
                for (key, value) in &result.remember {
                    self.state_unsaved |= settings.set_state(name, key, value);
                }
            }
            // A movement is written once it is done, and only if something changed.
            if self.state_unsaved && !matches!(event, Pointer::Move(_)) {
                self.state_unsaved = false;
                self.save_settings();
            }
            self.window.request_redraw();
        }
        result.used
    }

    /// The scroll wheel: over a display that uses it (a Loupe's zoom), else nothing.
    fn wheel(&mut self, lines: f32) {
        if self.menu.is_some() {
            return;
        }
        let panel = self.contents.iter().position(|c| Menu::contains(*c, self.cursor)).filter(|p| self.displays.takes_pointer(*p));
        if let Some(panel) = panel {
            self.display_pointer(panel, Pointer::Wheel { at: self.cursor, lines });
        }
    }

    fn button(&mut self, state: ElementState) {
        // Letting go always ends a drag, even with a menu open over it (a right click can open one meanwhile).
        if state == ElementState::Released {
            if let Some(panel) = self.pointer_panel.take() {
                self.display_pointer(panel, Pointer::Up);
            }
            if self.drag.take().is_some() {
                self.save_grid();
            }
            if let Some(from) = self.panel_drag.take() {
                // Let go on another Panel to swap; anywhere else cancels.
                if let Some(to) = self.grid.panel_at(self.grid_area, self.grid_gap, self.cursor).filter(|to| *to != from) {
                    self.grid.swap(from, to);
                    self.save_grid();
                }
                self.window.set_cursor(CursorIcon::Default);
                self.window.request_redraw();
            }
            return;
        }
        if self.menu.is_some() {
            self.menu_click();
            return;
        }
        match state {
            ElementState::Pressed => {
                // A display that uses the pointer (a Loupe) gets it first; else a border may be grabbed.
                let panel = self.contents.iter().position(|c| Menu::contains(*c, self.cursor)).filter(|p| self.displays.takes_pointer(*p));
                if let Some(panel) = panel {
                    if self.display_pointer(panel, Pointer::Down(self.cursor)) {
                        self.pointer_panel = Some(panel);
                        return;
                    }
                }
                match self.press_under_pointer() {
                    Some(Press::Border(border)) => self.drag = Some(border),
                    Some(Press::Panel(panel)) => {
                        self.panel_drag = Some(panel);
                        self.window.set_cursor(CursorIcon::Grabbing);
                    }
                    None => {}
                }
            }
            ElementState::Released => {}
        }
    }

    fn draw(&mut self) {
        let scale = self.window.scale_factor() as f32;
        self.scale = scale;
        let (width, height) = (self.config.width as f32, self.config.height as f32);
        let margin = MARGIN * scale;
        let texts = self.texts.lock().unwrap().clone();
        // Along the bottom, under the Panels: the status line, and above it, while a picture is on screen, a
        // notice in the same quiet size (it never covers the picture).
        let notice_in_status = texts.has_picture && texts.notice.is_some();
        let row = style::SPACE_STATUS_ROW * scale;
        let status_height = row * if notice_in_status { 2.0 } else { 1.0 };
        // The Panels fill the window above the status line, with a margin round them and between them.
        // The Colour Setup is said once, in the bar across the top.
        let top_height = BarKind::Top.height(scale);
        let area_top = top_height + margin;
        let area = [margin, area_top, (width - 2.0 * margin).max(1.0), (height - status_height - area_top).max(1.0)];
        let cells = self.grid.cells(area, margin);
        self.cells = cells.clone();
        self.grid_area = area;
        self.grid_gap = margin;

        let line = style::TYPE_LABEL * scale * style::TYPE_LABEL_LINE;
        // Each Panel: a thin border, its label on one line in the title bar (clipped to it), the display under
        // the bar, and the numbers on the display's scale.
        let mut panels = Vec::new();
        // Where each name and its tags sit in the title bar (the tags go to the hover when they do not fit).
        let mut title_parts: Vec<([f32; 4], Option<[f32; 4]>)> = Vec::new();
        self.title_hovers.clear();
        let max_chars = ((width / (style::SPACE_CHAR_WIDTH * scale)) as usize).saturating_sub(8).clamp(40, 110);
        for (i, cell) in cells.iter().enumerate() {
            let title = texts.titles.get(i).cloned().unwrap_or_default();
            let panel = PanelLayout::new(*cell, scale);
            let (l, tags, all_shown) = window_text::lay_out_title(&title, panel.title, scale);
            self.panel_texts[i].label.set(&mut self.font_system, &title.name, TextStyle { wrap: false, ..TextStyle::LABEL }, scale, l.name[2].max(1.0));
            let tags_style = TextStyle { size: style::TYPE_SCALE, line: style::TYPE_SCALE_LINE, align: None, wrap: false };
            self.panel_texts[i].tags.set(&mut self.font_system, &tags, tags_style, scale, l.tags.map_or(1.0, |t| t[2].max(1.0)));
            self.title_hovers.push(window_text::hover_text(&title, l.name_cut || !all_shown, max_chars));
            title_parts.push((l.name, l.tags));
            panels.push(panel);
        }
        self.title_bars = panels.iter().map(|p| p.title).collect();
        self.contents = panels.iter().map(|p| p.content).collect();
        let status_width = width - 2.0 * margin;
        // Playback and performance only: the longest choice that fits, whole parts only (never cut mid-word).
        let status_line = window_text::fit_status(&texts.status, status_width, scale);
        self.status.set(&mut self.font_system, status_line, TextStyle { wrap: false, ..TextStyle::LABEL }, scale, status_width);
        self.status_bar = [0.0, height - row, width, row];
        self.status_hover = texts.status_hover.clone();
        let top_line = window_text::fit_parts(&texts.top_bar, width - 2.0 * style::SPACE_BAR_PAD * scale, scale);
        self.top_bar.set(&mut self.font_system, &top_line, TextStyle { wrap: false, ..TextStyle::LABEL }, scale, (width - 2.0 * style::SPACE_BAR_PAD * scale).max(1.0));
        let (notice_style, notice_width) = if notice_in_status {
            (TextStyle::LABEL, status_width)
        } else {
            (TextStyle::NOTICE, (panels.first().map_or(1.0, |p| p.content[2]) - 2.0 * margin).max(1.0))
        };
        self.notice.set(&mut self.font_system, texts.notice.as_deref().unwrap_or(""), notice_style, scale, notice_width);
        let notice_height: f32 = self.notice.buffer.layout_runs().map(|run| run.line_height).sum();

        let whole = TextBounds { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        let status_top = height - row + (row - line) / 2.0 - 0.25 * margin;
        let (notice_left, notice_top) = if notice_in_status {
            (margin, status_top - row)
        } else {
            let [left, top, _, height] = panels.first().map_or([margin, margin, 1.0, 1.0], |p| p.content);
            (left + margin, (top + (height - notice_height) / 2.0).max(top))
        };
        // The scale's numbers, per Panel, shaped once and reused until they or the room change.
        let scale_mark_line = style::TYPE_SCALE * scale * style::TYPE_SCALE_LINE;
        let mut marks: Vec<(usize, scopecreep::display::ScaleText)> = Vec::new();
        if texts.has_picture {
            for (i, panel) in panels.iter().enumerate() {
                for mark in self.displays.scale_texts(i, panel.content, scale) {
                    marks.push((i, mark));
                }
            }
        }
        for (i, panel_text) in self.panel_texts.iter_mut().enumerate() {
            let wanted = marks.iter().filter(|(p, _)| *p == i).count();
            while panel_text.scale.len() < wanted {
                panel_text.scale.push(Text::new(&mut self.font_system));
            }
        }
        let mut used = vec![0usize; self.panel_texts.len()];
        let mut mark_areas = Vec::new();
        for (i, mark) in &marks {
            let n = used[*i];
            used[*i] += 1;
            let style = TextStyle { size: style::TYPE_SCALE, line: style::TYPE_SCALE_LINE, align: if mark.centred { Some(glyphon::cosmic_text::Align::Center) } else { mark.right_aligned.then_some(glyphon::cosmic_text::Align::Right) }, wrap: false };
            self.panel_texts[*i].scale[n].set(&mut self.font_system, &mark.text, style, scale, mark.width);
            mark_areas.push((*i, n, mark.left, mark.top - scale_mark_line / 2.0));
        }

        let mut areas = vec![
            TextArea { buffer: &self.status.buffer, left: margin, top: status_top, scale: 1.0, bounds: whole, default_color: text_colour(style::TEXT_SECONDARY), custom_glyphs: &[] },
            TextArea { buffer: &self.notice.buffer, left: notice_left, top: notice_top, scale: 1.0, bounds: whole, default_color: text_colour(style::TEXT_PRIMARY), custom_glyphs: &[] },
        ];
        let top_pad = style::SPACE_BAR_PAD * scale;
        areas.push(TextArea { buffer: &self.top_bar.buffer, left: top_pad, top: (top_height - line) / 2.0, scale: 1.0, bounds: whole, default_color: text_colour(style::TEXT_PRIMARY), custom_glyphs: &[] });
        let tags_line = style::TYPE_SCALE * scale * style::TYPE_SCALE_LINE;
        for ((panel, text), (name, tags)) in panels.iter().zip(&self.panel_texts).zip(&title_parts) {
            let [left, top, w, h] = panel.title;
            // A label never spills out of its title bar.
            let bounds = TextBounds { left: left as i32, top: top as i32, right: (left + w).ceil() as i32, bottom: (top + h).ceil() as i32 };
            areas.push(TextArea { buffer: &text.label.buffer, left: name[0], top: top + (h - line) / 2.0, scale: 1.0, bounds, default_color: text_colour(style::TEXT_PRIMARY), custom_glyphs: &[] });
            if let Some(tags) = tags {
                areas.push(TextArea { buffer: &text.tags.buffer, left: tags[0], top: top + (h - tags_line) / 2.0, scale: 1.0, bounds, default_color: text_colour(style::TEXT_MUTED), custom_glyphs: &[] });
            }
        }
        for (i, n, left, top) in mark_areas {
            let c = panels[i].content;
            let bounds = TextBounds { left: c[0] as i32, top: c[1] as i32, right: (c[0] + c[2]).ceil() as i32, bottom: (c[1] + c[3]).ceil() as i32 };
            areas.push(TextArea { buffer: &self.panel_texts[i].scale[n].buffer, left, top, scale: 1.0, bounds, default_color: text_colour(style::TEXT_MUTED), custom_glyphs: &[] });
        }
        self.viewport.update(&self.queue, Resolution { width: self.config.width, height: self.config.height });
        let prepared = self.text_renderer.prepare(&self.device, &self.queue, &mut self.font_system, &mut self.atlas, &self.viewport, areas, &mut self.swash_cache);
        if let Err(e) = prepared {
            eprintln!("scopecreep: text could not be prepared: {e}");
        }

        // The right-click menu, drawn as the popover component, over everything: its rectangles, then its own text
        // (so it sits above the Panels' text).
        let mut rects: Vec<([f32; 4], [f32; 4])> = Vec::new();
        // The top bar: its fill and the line under it.
        rects.push(([0.0, 0.0, width, top_height], style::rgba(style::SURFACE_PANEL)));
        rects.push(([0.0, top_height - (style::LINE_WEIGHT * scale).max(1.0), width, (style::LINE_WEIGHT * scale).max(1.0)], style::rgba(style::LINE_BORDER)));
        // Each Panel's thin border and the line under its title bar.
        for panel in &panels {
            for edge in panel.lines((style::LINE_WEIGHT * scale).max(1.0)) {
                rects.push((edge, style::rgba(style::LINE_BORDER)));
            }
        }
        // While a Panel is dragged, the Panel it would swap with is outlined, quietly.
        if let Some(target) = self.panel_drag.and_then(|from| self.grid.panel_at(area, margin, self.cursor).filter(|to| *to != from)) {
            let weight = (style::LINE_WEIGHT * scale).max(1.0);
            for edge in PanelLayout::new(cells[target], scale).lines(weight).iter().take(4) {
                rects.push((*edge, style::rgba(style::STATE_FOCUS)));
            }
        }
        let mut menu_areas: Vec<(usize, f32, f32, [u8; 3])> = Vec::new();
        // The menu over the Panels, or else the hover of what the pointer rests on, over everything.
        let mut over: Vec<Prim> = Vec::new();
        if let (Some((menu, _)), Some(rect)) = (self.menu.as_ref(), self.menu_rect()) {
            over = components::popover(menu, rect, self.cursor, scale);
        } else if let Some(hover) = self.hover {
            let (anchor, text) = match hover {
                Hover::Title(i) => (self.title_bars.get(i).copied(), self.title_hovers.get(i).cloned()),
                Hover::Status => (Some(self.status_bar), Some(self.status_hover.clone())),
            };
            if let (Some(anchor), Some(text)) = (anchor, text.filter(|t| !t.is_empty())) {
                let tip = Tooltip::new(&text);
                over = tip.draw(tip.rect(anchor, [width, height], scale), scale);
            }
        }
        let mut texts_used = 0;
        for prim in over {
            match prim {
                Prim::Rect { rect, colour } => rects.push((rect, colour)),
                Prim::Text { text, rect: at, colour, align, .. } => {
                    if self.menu_texts.len() <= texts_used {
                        self.menu_texts.push(Text::new(&mut self.font_system));
                    }
                    let align = match align {
                        Align::Left => None,
                        Align::Centre => Some(glyphon::cosmic_text::Align::Center),
                        Align::Right => Some(glyphon::cosmic_text::Align::Right),
                    };
                    self.menu_texts[texts_used].set(&mut self.font_system, &text, TextStyle { wrap: false, align, ..TextStyle::LABEL }, scale, at[2]);
                    menu_areas.push((texts_used, at[0], at[1] + (at[3] - line) / 2.0, colour));
                    texts_used += 1;
                }
            }
        }
        self.flat.set([width, height], &rects);
        let menu_text_areas: Vec<TextArea> = menu_areas
            .iter()
            .map(|(i, left, top, colour)| TextArea { buffer: &self.menu_texts[*i].buffer, left: *left, top: *top, scale: 1.0, bounds: whole, default_color: text_colour(*colour), custom_glyphs: &[] })
            .collect();
        if let Err(e) = self.menu_renderer.prepare(&self.device, &self.queue, &mut self.font_system, &mut self.atlas, &self.viewport, menu_text_areas, &mut self.swash_cache) {
            eprintln!("scopecreep: menu text could not be prepared: {e}");
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            _ => {
                // Outdated, suboptimal or lost: set the surface up again and draw on the next wake.
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("window"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(style::clear(style::SURFACE_WINDOW)), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for (i, panel) in panels.iter().enumerate() {
                self.displays.draw(&mut pass, i, panel.content, [width, height], scale);
            }
            if let Err(e) = self.text_renderer.render(&self.atlas, &self.viewport, &mut pass) {
                eprintln!("scopecreep: text could not be drawn: {e}");
            }
            self.flat.draw(&mut pass);
            if let Err(e) = self.menu_renderer.render(&self.atlas, &self.viewport, &mut pass) {
                eprintln!("scopecreep: menu text could not be drawn: {e}");
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        self.atlas.trim();
    }

    fn close(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.feed_thread.take() {
            let _ = thread.join();
        }
    }
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("scopecreep").with_inner_size(LogicalSize::new(1280.0, 760.0));
        #[cfg(target_os = "linux")]
        let attributes = {
            use winit::platform::wayland::WindowAttributesExtWayland;
            attributes.with_name("scopecreep", "scopecreep")
        };
        let made = event_loop
            .create_window(attributes)
            .map_err(Into::into)
            .and_then(|window| AppWindow::new(Arc::new(window), event_loop, self.feed.clone(), self.scripting, self.proxy.clone()));
        match made {
            Ok(window) => self.window = Some(window),
            Err(e) => {
                eprintln!("scopecreep: cannot open the window: {e}");
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _wake: Wake) {
        if let Some(w) = &self.window {
            w.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(w) = &mut self.window else { return };
        match event {
            WindowEvent::Resized(size) => w.resize(size.width, size.height),
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = w.window.inner_size();
                w.resize(size.width, size.height);
            }
            WindowEvent::CursorMoved { position, .. } => w.pointer_moved(position.x as f32, position.y as f32),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => w.button(state),
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => w.right_click(),
            WindowEvent::ModifiersChanged(m) => w.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => w.key(&event),
            WindowEvent::MouseWheel { delta, .. } => w.wheel(match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            }),
            WindowEvent::CursorLeft { .. } => {
                if w.hover.take().is_some() {
                    w.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => w.draw(),
            WindowEvent::CloseRequested => {
                w.close();
                event_loop.exit();
            }
            _ => {}
        }
    }
}
