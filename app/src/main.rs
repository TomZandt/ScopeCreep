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
use scopecreep::monitor::{FeedState, Monitor};
use scopecreep::output_conversion::OutputConversion;
use scopecreep::panel_grid::{Border, LABEL_SIZE, PanelGrid, PanelLayout, min_panel_size};
use scopecreep::resolve_helper::ScriptHelper;
use scopecreep::flat_rects::FlatRects;
use scopecreep::settings::{Menu, MenuRow, PanelSettings, MENU_ROW};
use scopecreep::waveform::SCALE_SIZE;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{CursorIcon, Window, WindowId};

/// Neutral, dark, and the same on every channel: nothing around the picture may tint the eye.
const BACKGROUND: wgpu::Color = wgpu::Color { r: 0.071, g: 0.071, b: 0.071, a: 1.0 };
const LABEL_COLOUR: Color = Color::rgb(138, 138, 138);
const NOTICE_COLOUR: Color = Color::rgb(176, 176, 176);
/// Sizes in logical pixels.
const NOTICE_SIZE: f32 = 15.0;
/// Height of one row of the status line, under the Panels.
const STATUS_ROW: f32 = 20.0;
const SCALE_COLOUR: Color = Color::rgb(112, 112, 112);
/// How far from a border's gap (logical pixels) a click still grabs it.
const GRAB: f32 = 3.0;

/// What the labels say, as the Feed thread last saw it.
#[derive(Clone, Default, PartialEq)]
struct Texts {
    /// Each Panel's label, in Panel order.
    labels: Vec<String>,
    status: String,
    notice: Option<String>,
    has_picture: bool,
}

struct Wake;

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

/// A piece of text kept shaped until what it says, or the room it has, changes.
struct Text {
    buffer: Buffer,
    key: (String, u32, u32),
}

impl Text {
    fn new(font_system: &mut FontSystem) -> Text {
        Text { buffer: Buffer::new(font_system, Metrics::new(LABEL_SIZE, LABEL_SIZE * 1.4)), key: (String::from("\u{0}"), 0, 0) }
    }

    /// Shape `text` at `size` logical pixels (at `scale`) in `width` device pixels, unless it already is.
    fn set(&mut self, font_system: &mut FontSystem, text: &str, size: f32, scale: f32, width: f32, align: Option<glyphon::cosmic_text::Align>, wrap: bool) {
        let key = (text.to_string(), width.round() as u32, scale.to_bits());
        if self.key == key {
            return;
        }
        let attrs = Attrs::new().family(Family::SansSerif);
        self.buffer.set_metrics(Metrics::new(size * scale, size * scale * if size == SCALE_SIZE { 1.3 } else { 1.4 }));
        self.buffer.set_size(Some(width.max(1.0)), None);
        self.buffer.set_wrap(if wrap { glyphon::cosmic_text::Wrap::WordOrGlyph } else { glyphon::cosmic_text::Wrap::None });
        self.buffer.set_text(text, &attrs, Shaping::Advanced, align);
        self.buffer.shape_until_scroll(font_system, false);
        self.key = key;
    }

    fn lines(&self) -> usize {
        self.buffer.layout_runs().count().max(1)
    }
}

/// What a Panel's text is made of: its label, and the numbers on its display's scale.
struct PanelText {
    label: Text,
    scale: Vec<Text>,
    /// The lines the label wrapped onto when last shaped: the Panel makes room for them.
    lines: usize,
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
    notice: Text,
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
        let grid = grid_file().and_then(|path| std::fs::read_to_string(path).ok()).map_or_else(|| PanelGrid::new(displays.len()), |text| PanelGrid::from_text(displays.len(), &text));
        let mut font_system = FontSystem::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, if format.is_srgb() { ColorMode::Accurate } else { ColorMode::Web });
        let text_renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let menu_renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let flat = FlatRects::new(&device, &queue, format);
        let status = Text::new(&mut font_system);
        let notice = Text::new(&mut font_system);
        let panel_texts = (0..displays.len()).map(|_| PanelText { label: Text::new(&mut font_system), scale: Vec::new(), lines: 1 }).collect();

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
                            scope_frames += 1;
                        });
                        shown = true;
                    });
                    let status = monitor.status();
                    let setup = status.colour_setup().map(|c| c.setup);
                    let now_context = status.display_context();
                    if now_context != context {
                        context = now_context;
                        displays.configure(&context, &chosen);
                    }
                    // A setting changed in a menu measures the frame on screen again, even with Resolve stopped.
                    let now_chosen = settings.lock().unwrap().clone();
                    if now_chosen != chosen {
                        chosen = now_chosen;
                        displays.configure(&context, &chosen);
                        conversion.again(uploader.frame(), |encoder, picture| displays.on_frame(encoder, picture));
                        shown = true;
                    }
                    // A Colour Setup that arrives or changes while Resolve is stopped converts the frame on screen.
                    shown |= conversion.follow(setup.as_ref(), uploader.frame(), |encoder, picture| displays.on_frame(encoder, picture));
                    let now = Texts {
                        labels: displays.labels(),
                        status: if status.feed == FeedState::Connected { status.status_line() } else { String::new() },
                        notice: status.notice(),
                        has_picture: status.picture.is_some(),
                    };
                    if now != last {
                        *texts.lock().unwrap() = now.clone();
                        last = now;
                        shown = true;
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
            flat,
            menu_renderer,
            menu_texts: Vec::new(),
            cells: Vec::new(),
            contents: Vec::new(),
            pointer_panel: None,
            drag: None,
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
            notice,
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

    /// The border under the pointer, if any.
    fn border_under_pointer(&self) -> Option<Border> {
        self.grid.border_at(self.grid_area, self.grid_gap, self.cursor, GRAB * self.scale)
    }

    fn pointer_moved(&mut self, x: f32, y: f32) {
        self.cursor = [x, y];
        if let Some(panel) = self.pointer_panel {
            self.display_pointer(panel, Pointer::Move(self.cursor));
        } else if let Some(border) = self.drag {
            self.grid.drag(border, self.cursor, self.grid_area, self.grid_gap, min_panel_size(self.scale));
            self.window.request_redraw();
        } else {
            self.window.set_cursor(match self.border_under_pointer() {
                Some(Border::Column { .. }) => CursorIcon::ColResize,
                Some(Border::Row { .. }) => CursorIcon::RowResize,
                None => CursorIcon::Default,
            });
        }
    }

    /// Right button: open the menu of the Panel under the pointer (none for a Panel without settings, or off a
    /// Panel), or close the one that is open.
    fn right_click(&mut self) {
        let was_open = self.menu.take().is_some();
        let hit = self.cells.iter().position(|c| Menu::contains(*c, self.cursor));
        if let Some(panel) = hit {
            let current = self.displays.settings_of(panel, &self.settings.lock().unwrap());
            self.menu = Menu::new(panel, self.displays.setting_defs(panel), &current).map(|menu| (menu, self.cursor));
        }
        if was_open || self.menu.is_some() {
            self.window.request_redraw();
        }
    }

    fn menu_rect(&self) -> Option<[f32; 4]> {
        let (menu, at) = self.menu.as_ref()?;
        Some(menu.rect(*at, [self.config.width as f32, self.config.height as f32], self.scale))
    }

    /// Left button while a menu is open: a click on a choice picks it (and remembers it); a click on a heading or
    /// the menu's padding leaves it open; any other click closes it.
    fn menu_click(&mut self) {
        let (Some((menu, _)), Some(rect)) = (self.menu.as_ref(), self.menu_rect()) else { return };
        let choice = menu.choice_at(rect, self.cursor, self.scale);
        if let Some((key, value)) = choice {
            let name = self.displays.name(menu.panel);
            let mut settings = self.settings.lock().unwrap();
            settings.set(name, key, value);
            if let Some(path) = settings_file() {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(&path, settings.to_text()) {
                    eprintln!("scopecreep: cannot remember the Panel settings: {e}");
                }
            }
        }
        if choice.is_none() && Menu::contains(rect, self.cursor) {
            return;
        }
        self.menu = None;
        self.window.request_redraw();
    }

    /// Give a display the pointer: what it asks to remember is kept with the Panel settings (and written to the
    /// file once the movement is done), and the window draws again. Returns whether the display took the event.
    fn display_pointer(&mut self, panel: usize, event: Pointer) -> bool {
        let Some(area) = self.contents.get(panel).copied() else { return false };
        let result = self.displays.pointer(panel, area, self.scale, event);
        if result.used {
            let mut settings = self.settings.lock().unwrap();
            let name = self.displays.name(panel);
            for (key, value) in &result.remember {
                settings.set_state(name, key, value);
            }
            if !result.remember.is_empty() && !matches!(event, Pointer::Move(_)) {
                if let Some(path) = settings_file() {
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    if let Err(e) = std::fs::write(&path, settings.to_text()) {
                        eprintln!("scopecreep: cannot remember the Panel settings: {e}");
                    }
                }
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
        if self.menu.is_some() {
            if state == ElementState::Pressed {
                self.menu_click();
            }
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
                self.drag = self.border_under_pointer();
            }
            ElementState::Released => {
                if let Some(panel) = self.pointer_panel.take() {
                    self.display_pointer(panel, Pointer::Up);
                    return;
                }
                if self.drag.take().is_some() {
                    // Remember the border positions.
                    if let Some(path) = grid_file() {
                        if let Some(dir) = path.parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        if let Err(e) = std::fs::write(&path, self.grid.to_text()) {
                            eprintln!("scopecreep: cannot remember the Panel borders: {e}");
                        }
                    }
                }
            }
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
        let row = STATUS_ROW * scale;
        let status_height = row * if notice_in_status { 2.0 } else { 1.0 };
        // The Panels fill the window above the status line, with a margin round them and between them.
        let area = [margin, margin, (width - 2.0 * margin).max(1.0), (height - status_height - margin).max(1.0)];
        let cells = self.grid.cells(area, margin);
        self.cells = cells.clone();
        self.grid_area = area;
        self.grid_gap = margin;

        let line = LABEL_SIZE * scale * 1.4;
        // Each Panel: its label wrapped to the Panel's width (the Panel makes room for the lines), the display
        // above it, and the numbers on the display's scale.
        let mut panels = Vec::new();
        for (i, cell) in cells.iter().enumerate() {
            let label = texts.labels.get(i).map(String::as_str).unwrap_or("");
            let panel_text = &mut self.panel_texts[i];
            panel_text.label.set(&mut self.font_system, label, LABEL_SIZE, scale, cell[2], None, true);
            let lines = panel_text.label.lines();
            if lines != panel_text.lines {
                panel_text.lines = lines;
                self.window.request_redraw();
            }
            panels.push(PanelLayout::new(*cell, scale, lines));
        }
        self.contents = panels.iter().map(|p| p.content).collect();
        let status_width = width - 2.0 * margin;
        self.status.set(&mut self.font_system, &texts.status, LABEL_SIZE, scale, status_width, None, true);
        let (notice_size, notice_width, align, notice_wrap) = if notice_in_status {
            (LABEL_SIZE, status_width, None, true)
        } else {
            (NOTICE_SIZE, (panels.first().map_or(1.0, |p| p.content[2]) - 2.0 * margin).max(1.0), Some(glyphon::cosmic_text::Align::Center), true)
        };
        self.notice.set(&mut self.font_system, texts.notice.as_deref().unwrap_or(""), notice_size, scale, notice_width, align, notice_wrap);
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
        let scale_mark_line = SCALE_SIZE * scale * 1.3;
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
            let align = mark.right_aligned.then_some(glyphon::cosmic_text::Align::Right);
            self.panel_texts[*i].scale[n].set(&mut self.font_system, &mark.text, SCALE_SIZE, scale, mark.width, align, false);
            mark_areas.push((*i, n, mark.left, mark.top - scale_mark_line / 2.0));
        }

        let mut areas = vec![
            TextArea { buffer: &self.status.buffer, left: margin, top: status_top, scale: 1.0, bounds: whole, default_color: LABEL_COLOUR, custom_glyphs: &[] },
            TextArea { buffer: &self.notice.buffer, left: notice_left, top: notice_top, scale: 1.0, bounds: whole, default_color: NOTICE_COLOUR, custom_glyphs: &[] },
        ];
        for (panel, text) in panels.iter().zip(&self.panel_texts) {
            let [left, top, w, h] = panel.label;
            // A label never spills out of its Panel.
            let bounds = TextBounds { left: left as i32, top: top as i32, right: (left + w).ceil() as i32, bottom: (top + h).ceil() as i32 };
            areas.push(TextArea { buffer: &text.label.buffer, left, top: top + (scopecreep::panel_grid::LABEL_STRIP * scale - line) / 2.0, scale: 1.0, bounds, default_color: LABEL_COLOUR, custom_glyphs: &[] });
        }
        for (i, n, left, top) in mark_areas {
            let c = panels[i].content;
            let bounds = TextBounds { left: c[0] as i32, top: c[1] as i32, right: (c[0] + c[2]).ceil() as i32, bottom: (c[1] + c[3]).ceil() as i32 };
            areas.push(TextArea { buffer: &self.panel_texts[i].scale[n].buffer, left, top, scale: 1.0, bounds, default_color: SCALE_COLOUR, custom_glyphs: &[] });
        }
        self.viewport.update(&self.queue, Resolution { width: self.config.width, height: self.config.height });
        let prepared = self.text_renderer.prepare(&self.device, &self.queue, &mut self.font_system, &mut self.atlas, &self.viewport, areas, &mut self.swash_cache);
        if let Err(e) = prepared {
            eprintln!("scopecreep: text could not be prepared: {e}");
        }

        // The right-click menu, over everything: its rectangles, then its own text (so it sits above the Panels' text).
        let mut rects: Vec<([f32; 4], [f32; 4])> = Vec::new();
        let mut menu_areas: Vec<(usize, f32, f32, bool)> = Vec::new();
        if let (Some((menu, _)), Some(rect)) = (self.menu.as_ref(), self.menu_rect()) {
            while self.menu_texts.len() < menu.rows.len() {
                self.menu_texts.push(Text::new(&mut self.font_system));
            }
            rects.push(([rect[0] - scale, rect[1] - scale, rect[2] + 2.0 * scale, rect[3] + 2.0 * scale], [0.30, 0.30, 0.30, 1.0]));
            rects.push((rect, [0.11, 0.11, 0.11, 1.0]));
            let text_left = rect[0] + 26.0 * scale;
            for (i, row) in menu.rows.iter().enumerate() {
                let top = Menu::row_top(rect, i, scale);
                let (text, heading) = match row {
                    MenuRow::Heading(text) => (text.as_str(), true),
                    MenuRow::Choice { text, on, .. } => {
                        let row_rect = [rect[0], top, rect[2], MENU_ROW * scale];
                        if Menu::contains(row_rect, self.cursor) {
                            rects.push((row_rect, [0.20, 0.20, 0.20, 1.0]));
                        }
                        if *on {
                            let side = 6.0 * scale;
                            rects.push(([rect[0] + 12.0 * scale, top + (MENU_ROW * scale - side) / 2.0, side, side], [0.86, 0.86, 0.86, 1.0]));
                        }
                        (text.as_str(), false)
                    }
                };
                self.menu_texts[i].set(&mut self.font_system, text, LABEL_SIZE, scale, rect[2] - 34.0 * scale, None, false);
                menu_areas.push((i, if heading { rect[0] + 10.0 * scale } else { text_left }, top + (MENU_ROW * scale - line) / 2.0, heading));
            }
        }
        self.flat.set([width, height], &rects);
        let menu_text_areas: Vec<TextArea> = menu_areas
            .iter()
            .map(|(i, left, top, heading)| TextArea { buffer: &self.menu_texts[*i].buffer, left: *left, top: *top, scale: 1.0, bounds: whole, default_color: if *heading { LABEL_COLOUR } else { NOTICE_COLOUR }, custom_glyphs: &[] })
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
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(BACKGROUND), store: wgpu::StoreOp::Store },
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
            WindowEvent::MouseWheel { delta, .. } => w.wheel(match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            }),
            WindowEvent::RedrawRequested => w.draw(),
            WindowEvent::CloseRequested => {
                w.close();
                event_loop.exit();
            }
            _ => {}
        }
    }
}
