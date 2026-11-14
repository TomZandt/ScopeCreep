//! The scopecreep app: one window with a Picture View showing the latest Feed frame, and a quiet label.
//!
//!   scopecreep [--feed <path>]      (default: $SCOPECREEP_FEED, else /dev/shm/scopecreep-feed)
//!
//! Two threads. The Feed thread polls the Feed every millisecond (the ring is short and the Sender never waits),
//! uploads the frames to show, and wakes the window. The window thread only draws.
mod picture_view;

use glyphon::{Attrs, Buffer, Cache, Color, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
use picture_view::PictureView;
use scopecreep::monitor::Monitor;
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

/// What the labels say, as the Feed thread last saw it.
#[derive(Clone, Default, PartialEq)]
struct Texts {
    label: String,
    notice: Option<String>,
    has_picture: bool,
}

struct Wake;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut feed = scopecreep_feed::default_path();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--feed" => feed = args.next().ok_or("--feed needs a path")?.into(),
            "--version" => {
                println!("scopecreep {} (Feed protocol {}.{})", env!("CARGO_PKG_VERSION"), scopecreep_feed::VERSION_MAJOR, scopecreep_feed::VERSION_MINOR);
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}. Usage: scopecreep [--feed <path>]").into()),
        }
    }
    let event_loop = EventLoop::<Wake>::with_user_event().build()?;
    let mut app = App { feed, proxy: event_loop.create_proxy(), window: None };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    feed: PathBuf,
    proxy: EventLoopProxy<Wake>,
    window: Option<AppWindow>,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    picture_view: PictureView,
    texts: Arc<Mutex<Texts>>,
    drawn: Texts,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    label: Buffer,
    notice: Buffer,
    stop: Arc<AtomicBool>,
    feed_thread: Option<std::thread::JoinHandle<()>>,
    // Last, so it outlives the surface made from it.
    window: Arc<Window>,
}

impl AppWindow {
    fn new(window: Arc<Window>, event_loop: &ActiveEventLoop, feed: PathBuf, proxy: EventLoopProxy<Wake>) -> Result<AppWindow, Box<dyn std::error::Error>> {
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
        let mut font_system = FontSystem::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, if format.is_srgb() { ColorMode::Accurate } else { ColorMode::Web });
        let text_renderer = TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let label = Buffer::new(&mut font_system, Metrics::new(LABEL_SIZE, LABEL_SIZE * 1.4));
        let notice = Buffer::new(&mut font_system, Metrics::new(NOTICE_SIZE, NOTICE_SIZE * 1.5));

        let texts = Arc::new(Mutex::new(Texts::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let feed_thread = {
            let (texts, stop, picture_view) = (texts.clone(), stop.clone(), picture_view.clone());
            std::thread::Builder::new().name("feed".into()).spawn(move || {
                let mut monitor = Monitor::new(&feed);
                let mut last = Texts::default();
                while !stop.load(Ordering::Relaxed) {
                    let mut shown = false;
                    monitor.poll(scopecreep_feed::now_ns(), &mut |frame| {
                        picture_view.upload(frame);
                        shown = true;
                    });
                    let status = monitor.status();
                    let now = Texts { label: status.picture_label(), notice: status.notice(), has_picture: status.picture.is_some() };
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
            texts,
            drawn: Texts { label: "\u{0}".into(), ..Default::default() },
            font_system,
            swash_cache: SwashCache::new(),
            viewport,
            atlas,
            text_renderer,
            label,
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
        self.drawn.label = "\u{0}".into(); // lay the text out again for the new size
        self.window.request_redraw();
    }

    fn draw(&mut self) {
        let scale = self.window.scale_factor() as f32;
        let (width, height) = (self.config.width as f32, self.config.height as f32);
        let margin = MARGIN * scale;
        let strip = LABEL_STRIP * scale;

        let texts = self.texts.lock().unwrap().clone();
        if texts != self.drawn {
            let attrs = Attrs::new().family(Family::SansSerif);
            // With a picture on screen, a notice joins the label line instead of covering the picture.
            let (label, notice) = match (&texts.notice, texts.has_picture) {
                (Some(n), true) => (format!("{}  ·  {}", texts.label, n), String::new()),
                (n, _) => (texts.label.clone(), n.clone().unwrap_or_default()),
            };
            self.label.set_metrics(Metrics::new(LABEL_SIZE * scale, LABEL_SIZE * scale * 1.4));
            self.label.set_size(Some(width - 2.0 * margin), Some(strip));
            self.label.set_text(&label, &attrs, Shaping::Advanced, None);
            self.label.shape_until_scroll(&mut self.font_system, false);
            self.notice.set_metrics(Metrics::new(NOTICE_SIZE * scale, NOTICE_SIZE * scale * 1.5));
            self.notice.set_size(Some((width - 4.0 * margin).max(1.0)), None);
            self.notice.set_text(&notice, &attrs, Shaping::Advanced, Some(glyphon::cosmic_text::Align::Center));
            self.notice.shape_until_scroll(&mut self.font_system, false);
            self.drawn = texts;
        }
        let notice_height: f32 = self.notice.layout_runs().map(|run| run.line_height).sum();
        let bounds = TextBounds { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        self.viewport.update(&self.queue, Resolution { width: self.config.width, height: self.config.height });
        let prepared = self.text_renderer.prepare(
            &self.device,
            &self.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            [
                TextArea {
                    buffer: &self.label,
                    left: margin,
                    top: height - strip + (strip - LABEL_SIZE * scale * 1.4) / 2.0,
                    scale: 1.0,
                    bounds,
                    default_color: LABEL_COLOUR,
                    custom_glyphs: &[],
                },
                TextArea {
                    buffer: &self.notice,
                    left: 2.0 * margin,
                    top: ((height - strip - notice_height) / 2.0).max(margin),
                    scale: 1.0,
                    bounds,
                    default_color: NOTICE_COLOUR,
                    custom_glyphs: &[],
                },
            ],
            &mut self.swash_cache,
        );
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
            // The Picture View panel: everything above the label strip, inside the margin.
            self.picture_view.draw(&mut pass, [margin, margin, width - 2.0 * margin, height - strip - margin], [width, height]);
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
            .and_then(|window| AppWindow::new(Arc::new(window), event_loop, self.feed.clone(), self.proxy.clone()));
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
