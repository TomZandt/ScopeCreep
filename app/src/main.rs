//! The scopecreep app: one window with two panels, the Picture View of the latest Feed frame on the left and the
//! luma waveform on the right, each with a quiet label, and a quiet status line along the bottom (Resolve's
//! playback rate, dropped frames, the Sender driving the Feed). Both panels show the Feed after Output Conversion,
//! in the Output Colour Space, whenever the Colour Setup has one (`output_conversion.rs`).
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
//! puts the frames to show on the GPU, converts and measures them, and wakes the window. The window thread only draws.
mod picture_view;

use glyphon::{Attrs, Buffer, Cache, Color, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
use picture_view::PictureView;
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::waveform::{DEFAULT_LUMA_WEIGHTS, LumaWeights, Measurement, PlotLayout, Waveform};
use scopecreep::monitor::{FeedState, Monitor};
use scopecreep::output_conversion::{self, OutputConversion};
use scopecreep::resolve_helper::ScriptHelper;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

/// Neutral, dark, and the same on every channel: nothing around the picture may tint the eye.
const BACKGROUND: wgpu::Color = wgpu::Color { r: 0.071, g: 0.071, b: 0.071, a: 1.0 };
const LABEL_COLOUR: Color = Color::rgb(138, 138, 138);
const NOTICE_COLOUR: Color = Color::rgb(176, 176, 176);
/// Sizes in logical pixels.
const MARGIN: f32 = 12.0;
const LABEL_SIZE: f32 = 12.5;
const LABEL_STRIP: f32 = 30.0;
const NOTICE_SIZE: f32 = 15.0;
/// Height of one row of the status line, under the label strip.
const STATUS_ROW: f32 = 20.0;
const SCALE_COLOUR: Color = Color::rgb(112, 112, 112);
const SCALE_SIZE: f32 = 11.0;

/// The marks on the waveform's scale, in its gutter.
#[derive(Clone, Copy)]
enum Mark {
    Code(u32),
    Above,
    Below,
}
/// Codes 0 and 1023 are the plot's own edges, so the edge rows are named instead. The codes follow the Levels
/// (`Levels::scale_marks`) and are set again whenever the waveform's label changes.
const MARKS: [Mark; 5] = [Mark::Above, Mark::Code(940), Mark::Code(512), Mark::Code(64), Mark::Below];

/// What the labels say, as the Feed thread last saw it.
#[derive(Clone, Default, PartialEq)]
struct Texts {
    label: String,
    status: String,
    notice: Option<String>,
    has_picture: bool,
}

struct Wake;

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

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    picture_view: PictureView,
    waveform: Waveform,
    texts: Arc<Mutex<Texts>>,
    drawn: Texts,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    label: Buffer,
    status: Buffer,
    notice: Buffer,
    waveform_label: Buffer,
    /// The text of `waveform_label` as laid out.
    waveform_text: String,
    marks: Vec<(Mark, Buffer)>,
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

        let picture_view = PictureView::new(&device, &queue, format, filterable);
        let waveform = Waveform::new(&device, &queue, format);
        waveform.set_measurement(Measurement { luma, ..Measurement::default() });
        let mut font_system = FontSystem::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, if format.is_srgb() { ColorMode::Accurate } else { ColorMode::Web });
        let text_renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let label = Buffer::new(&mut font_system, Metrics::new(LABEL_SIZE, LABEL_SIZE * 1.4));
        let notice = Buffer::new(&mut font_system, Metrics::new(NOTICE_SIZE, NOTICE_SIZE * 1.5));
        let status = Buffer::new(&mut font_system, Metrics::new(LABEL_SIZE, LABEL_SIZE * 1.4));
        let waveform_label = Buffer::new(&mut font_system, Metrics::new(LABEL_SIZE, LABEL_SIZE * 1.4));
        let marks = MARKS.iter().map(|&mark| (mark, Buffer::new(&mut font_system, Metrics::new(SCALE_SIZE, SCALE_SIZE * 1.3)))).collect();

        let texts = Arc::new(Mutex::new(Texts::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let feed_thread = {
            let (texts, stop, picture_view, waveform) = (texts.clone(), stop.clone(), picture_view.clone(), waveform.clone());
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
                // Frames put on the GPU and measured by the waveform.
                let mut scope_frames = 0u64;
                // The waveform measures in the Colour Setup's Output Colour Space (the default Measured Space).
                let mut measurement = Measurement { luma, ..Measurement::default() };
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
                            // The Picture View and the waveform both see the picture after Output Conversion.
                            let picture = conversion.convert(encoder, gpu_frame);
                            waveform.measure(encoder, picture);
                            picture_view.show(picture);
                            scope_frames += 1;
                        });
                        shown = true;
                    });
                    let status = monitor.status();
                    let setup = status.colour_setup().map(|c| c.setup);
                    let now_measuring = output_conversion::measurement(status.colour_setup(), luma);
                    if now_measuring != measurement {
                        measurement = now_measuring;
                        waveform.set_measurement(measurement);
                    }
                    // A Colour Setup that arrives or changes while Resolve is stopped converts the frame on screen.
                    shown |= conversion.follow(setup.as_ref(), uploader.frame(), |encoder, picture| {
                        waveform.measure(encoder, picture);
                        picture_view.show(picture);
                    });
                    let now = Texts {
                        label: status.picture_label(),
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
            picture_view,
            waveform,
            texts,
            drawn: Texts { label: "\u{0}".into(), ..Default::default() },
            font_system,
            swash_cache: SwashCache::new(),
            viewport,
            atlas,
            text_renderer,
            label,
            status,
            notice,
            waveform_label,
            waveform_text: String::new(),
            marks,
            stop,
            feed_thread: Some(feed_thread),
            window,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.drawn.label = "\u{0}".into(); // lay the text out again for the new size
        self.waveform_text = "\u{0}".into();
        self.window.request_redraw();
    }

    fn draw(&mut self) {
        let scale = self.window.scale_factor() as f32;
        let (width, height) = (self.config.width as f32, self.config.height as f32);
        let margin = MARGIN * scale;
        let line = LABEL_SIZE * scale * 1.4;
        // A long Picture View label wraps: the strip grows by a line for each extra line, so the label never runs
        // into the status line. (Measured from the label as last laid out; a change of line count redraws.)
        let label_lines = self.label.layout_runs().count().max(1);
        let strip = LABEL_STRIP * scale + (label_lines - 1) as f32 * line;
        let texts = self.texts.lock().unwrap().clone();
        // Along the bottom, under the panel labels: the status line, and above it, while a picture is on screen, a
        // notice in the same quiet size (it never covers the picture).
        let notice_in_status = texts.has_picture && texts.notice.is_some();
        let row = STATUS_ROW * scale;
        let status_height = row * if notice_in_status { 2.0 } else { 1.0 };
        // Two panels side by side above the label strip: the Picture View on the left, the waveform on the right.
        let half = (width / 2.0).round();
        let panel_height = (height - strip - status_height - margin).max(1.0);
        let picture_area = [margin, margin, (half - 1.5 * margin).max(1.0), panel_height];
        let waveform_area = [half + 0.5 * margin, margin, (width - half - 1.5 * margin).max(1.0), panel_height];
        let plot = PlotLayout::new(waveform_area, scale);
        let gutter = plot.plot()[0] - waveform_area[0];
        let attrs = Attrs::new().family(Family::SansSerif);

        if texts != self.drawn {
            let line = LABEL_SIZE * scale * 1.4;
            self.label.set_metrics(Metrics::new(LABEL_SIZE * scale, line));
            self.label.set_size(Some(picture_area[2]), Some(strip));
            self.label.set_text(&texts.label, &attrs, Shaping::Advanced, None);
            self.label.shape_until_scroll(&mut self.font_system, false);
            self.status.set_metrics(Metrics::new(LABEL_SIZE * scale, line));
            self.status.set_size(Some(width - 2.0 * margin), Some(line));
            self.status.set_text(&texts.status, &attrs, Shaping::Advanced, None);
            self.status.shape_until_scroll(&mut self.font_system, false);
            let (size, notice_width, align) = if notice_in_status {
                (LABEL_SIZE, width - 2.0 * margin, None)
            } else {
                (NOTICE_SIZE, (picture_area[2] - 2.0 * margin).max(1.0), Some(glyphon::cosmic_text::Align::Center))
            };
            self.notice.set_metrics(Metrics::new(size * scale, size * scale * 1.5));
            self.notice.set_size(Some(notice_width), None);
            self.notice.set_text(texts.notice.as_deref().unwrap_or(""), &attrs, Shaping::Advanced, align);
            self.notice.shape_until_scroll(&mut self.font_system, false);
            self.drawn = texts.clone();
        }
        let waveform_text = self.waveform.label();
        if waveform_text != self.waveform_text {
            self.waveform_label.set_metrics(Metrics::new(LABEL_SIZE * scale, LABEL_SIZE * scale * 1.4));
            self.waveform_label.set_size(Some(waveform_area[2] - gutter), Some(strip));
            self.waveform_label.set_text(&waveform_text, &attrs, Shaping::Advanced, None);
            self.waveform_label.shape_until_scroll(&mut self.font_system, false);
            let mut codes = self.waveform.measurement().levels.scale_marks().into_iter();
            for (mark, buffer) in &mut self.marks {
                if let Mark::Code(code) = mark {
                    *code = codes.next().unwrap_or(*code);
                }
                let text = match mark {
                    Mark::Code(code) => code.to_string(),
                    Mark::Above => "> 1023".into(),
                    Mark::Below => "< 0".into(),
                };
                buffer.set_metrics(Metrics::new(SCALE_SIZE * scale, SCALE_SIZE * scale * 1.3));
                buffer.set_size(Some((gutter - 6.0 * scale).max(1.0)), None);
                buffer.set_wrap(glyphon::cosmic_text::Wrap::None);
                buffer.set_text(&text, &attrs, Shaping::Advanced, Some(glyphon::cosmic_text::Align::Right));
                buffer.shape_until_scroll(&mut self.font_system, false);
            }
            self.waveform_text = waveform_text;
        }
        let notice_height: f32 = self.notice.layout_runs().map(|run| run.line_height).sum();
        let bounds = TextBounds { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        if self.label.layout_runs().count().max(1) != label_lines {
            self.window.request_redraw(); // the label's line count changed: lay the panels out again
        }
        let label_top = height - status_height - strip + (LABEL_STRIP * scale - line) / 2.0;
        let status_top = height - row + (row - LABEL_SIZE * scale * 1.4) / 2.0 - 0.25 * margin;
        let (notice_left, notice_top) = if notice_in_status {
            (margin, status_top - row)
        } else {
            (picture_area[0] + margin, (margin + (panel_height - notice_height) / 2.0).max(margin))
        };
        let mark_line = SCALE_SIZE * scale * 1.3;
        let mark_y = |mark: Mark| match mark {
            Mark::Code(code) => plot.y_of_code(code),
            Mark::Above => plot.above()[1] + plot.above()[3] / 2.0,
            Mark::Below => plot.below()[1] + plot.below()[3] / 2.0,
        };
        let mut areas = vec![
            TextArea { buffer: &self.label, left: picture_area[0], top: label_top, scale: 1.0, bounds, default_color: LABEL_COLOUR, custom_glyphs: &[] },
            TextArea { buffer: &self.status, left: margin, top: status_top, scale: 1.0, bounds, default_color: LABEL_COLOUR, custom_glyphs: &[] },
            TextArea {
                buffer: &self.notice,
                left: notice_left,
                top: notice_top,
                scale: 1.0,
                bounds,
                default_color: NOTICE_COLOUR,
                custom_glyphs: &[],
            },
            TextArea { buffer: &self.waveform_label, left: plot.plot()[0], top: label_top, scale: 1.0, bounds, default_color: LABEL_COLOUR, custom_glyphs: &[] },
        ];
        if texts.has_picture {
            for (mark, buffer) in &self.marks {
                areas.push(TextArea {
                    buffer,
                    left: waveform_area[0],
                    top: mark_y(*mark) - mark_line / 2.0,
                    scale: 1.0,
                    bounds,
                    default_color: SCALE_COLOUR,
                    custom_glyphs: &[],
                });
            }
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
            self.picture_view.draw(&mut pass, picture_area, [width, height]);
            self.waveform.draw(&mut pass, waveform_area, [width, height], scale);
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
            WindowEvent::RedrawRequested => w.draw(),
            WindowEvent::CloseRequested => {
                w.close();
                event_loop.exit();
            }
            _ => {}
        }
    }
}
