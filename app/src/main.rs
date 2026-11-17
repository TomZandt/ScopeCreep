//! The scopecreep app: one window holding a grid of Panels, one per display (ADR 0011): the Picture View of the
//! latest Feed frame and the luma waveform for now, each with a quiet label under it, and a quiet status line
//! along the bottom (Resolve's playback rate, dropped frames, the Sender driving the Feed). The borders between
//! Panels can be dragged and are remembered. Every display shows the Feed after Output Conversion, in the Output
//! Colour Space, whenever the Colour Setup has one (`output_conversion.rs`).
//!
//!   scopecreep [--feed <path>] [--luma primaries|match-resolve] [--no-scripting]
//!
//!   --feed           default: $SCOPECREEP_FEED, else /dev/shm/scopecreep-feed
//!   --luma           luma weights (ADR 0004): from the Measured Space's primaries (the default), or the weights
//!                    Resolve's own waveform uses
//!   --no-scripting   never read the project by Resolve scripting (the Colour Setup then comes only from the
//!                    Sender node's manual override). Resolve's Python: $SCOPECREEP_RESOLVE_PYTHON, else
//!                    /opt/resolve/bin/ResolvePython.
//!
//! Two threads. The Feed thread polls the Feed every millisecond (the ring is short and the Sender never waits),
//! puts the frames to show on the GPU once, converts them once, hands that one frame to every display, and wakes
//! the window. The window thread only draws.
use glyphon::{Attrs, Buffer, Cache, Color, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
use scopecreep::display::{DisplayContext, DisplaySet};
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::layout::MARGIN;
use scopecreep::monitor::{FeedState, Monitor};
use scopecreep::output_conversion::OutputConversion;
use scopecreep::panel_grid::{Border, LABEL_SIZE, PanelGrid, PanelLayout, min_panel_size};
use scopecreep::resolve_helper::ScriptHelper;
use scopecreep::waveform::{DEFAULT_LUMA_WEIGHTS, LumaWeights, SCALE_SIZE};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut feed = scopecreep_feed::default_path();
    let mut scripting = true;
    let mut luma = DEFAULT_LUMA_WEIGHTS;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--feed" => feed = args.next().ok_or("--feed needs a path")?.into(),
            "--no-scripting" => scripting = false,
            "--luma" => {
                luma = match args.next().as_deref() {
                    Some("primaries") => LumaWeights::FromPrimaries,
                    Some("match-resolve") => LumaWeights::MatchResolve,
                    _ => return Err("--luma is primaries or match-resolve".into()),
                }
            }
            "--version" => {
                println!("scopecreep {} (Feed protocol {}.{})", env!("CARGO_PKG_VERSION"), scopecreep_feed::VERSION_MAJOR, scopecreep_feed::VERSION_MINOR);
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}. Usage: scopecreep [--feed <path>] [--luma primaries|match-resolve] [--no-scripting]").into()),
        }
    }
    let event_loop = EventLoop::<Wake>::with_user_event().build()?;
    let mut app = App { feed, luma, scripting, proxy: event_loop.create_proxy(), window: None };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    feed: PathBuf,
    luma: LumaWeights,
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
    fn new(window: Arc<Window>, event_loop: &ActiveEventLoop, feed: PathBuf, luma: LumaWeights, scripting: bool, proxy: EventLoopProxy<Wake>) -> Result<AppWindow, Box<dyn std::error::Error>> {
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
        displays.configure(&DisplayContext { luma, ..DisplayContext::default() });
        let grid = grid_file().and_then(|path| std::fs::read_to_string(path).ok()).map_or_else(|| PanelGrid::new(displays.len()), |text| PanelGrid::from_text(displays.len(), &text));
        let mut font_system = FontSystem::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, if format.is_srgb() { ColorMode::Accurate } else { ColorMode::Web });
        let text_renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let status = Text::new(&mut font_system);
        let notice = Text::new(&mut font_system);
        let panel_texts = (0..displays.len()).map(|_| PanelText { label: Text::new(&mut font_system), scale: Vec::new(), lines: 1 }).collect();

        let texts = Arc::new(Mutex::new(Texts::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let feed_thread = {
            let (texts, stop, displays) = (texts.clone(), stop.clone(), displays.clone());
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
                let mut context = DisplayContext { luma, ..DisplayContext::default() };
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
                    let now_context = status.display_context(luma);
                    if now_context != context {
                        context = now_context;
                        displays.configure(&context);
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
        if let Some(border) = self.drag {
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

    fn button(&mut self, state: ElementState) {
        match state {
            ElementState::Pressed => self.drag = self.border_under_pointer(),
            ElementState::Released => {
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
            .and_then(|window| AppWindow::new(Arc::new(window), event_loop, self.feed.clone(), self.luma, self.scripting, self.proxy.clone()));
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
            WindowEvent::RedrawRequested => w.draw(),
            WindowEvent::CloseRequested => {
                w.close();
                event_loop.exit();
            }
            _ => {}
        }
    }
}
