//! The one GPU test bench the display tests share (#107, spec #106): the device, a stand-in Resolve playing
//! synthetic and ColorChecker frames through the Feed, the app's Feed monitor and uploader, and a way to draw
//! Panels and read the pixels back. A test file wraps a [`Rig`] with the displays it looks at and its own
//! assertions; it never sets up the GPU itself. Needs a GPU.
#![allow(dead_code)]
use scopecreep::feed_loop::{Changed, FeedLoop};
use scopecreep::gpu_frame::{FrameUploader, GpuFrame};
use scopecreep::monitor::Monitor;
use scopecreep::panel_grid::{PanelGrid, PanelLayout};
use scopecreep::project::ProjectReader;
use scopecreep_feed::{FeedWriter, FrameMeta, Provenance, SentColourSetup, now_ns};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

pub const MS: u64 = 1_000_000;
/// The format every test draws into: what the readback sees is RGBA8.
pub const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const FEED_CAPACITY: usize = 1920 * 1080 * 16;

/// One GPU for the whole test binary; tests take turns on it.
fn gpu() -> (MutexGuard<'static, ()>, wgpu::Device, wgpu::Queue) {
    static GPU: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (device, queue) = GPU.get_or_init(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() }))
            .expect("these tests need a GPU");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("display tests"), ..Default::default() })).unwrap()
    });
    (turn, device.clone(), queue.clone())
}

/// A stand-in Resolve and Sender (the Feed writer), the app's Feed monitor and frame uploader, and the GPU.
/// The wrapping bench decides what is done with each uploaded frame.
pub struct Rig {
    pub path: PathBuf,
    pub writer: FeedWriter,
    pub monitor: Monitor,
    pub uploader: FrameUploader,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub now: u64,
    pub frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Rig {
    /// No Colour Setup sent.
    pub fn new(name: &str) -> Rig {
        Rig::build(name, FEED_CAPACITY, None, None)
    }

    /// The Sender gives `colour_setup` (the text of a manual Colour Setup), or none.
    pub fn with_colour_setup(name: &str, colour_setup: Option<&str>) -> Rig {
        Rig::build(name, FEED_CAPACITY, colour_setup, None)
    }

    /// Everything a bench can vary: the Feed's frame capacity and a stand-in project reader for the Monitor.
    pub fn build(name: &str, capacity: usize, colour_setup: Option<&str>, project: Option<Box<dyn ProjectReader>>) -> Rig {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let mut writer = FeedWriter::create(&path, 4, capacity).unwrap();
        if let Some(text) = colour_setup {
            writer.set_colour_setup(Some(SentColourSetup::new(Provenance::Manual, text)));
        }
        let monitor = match project {
            Some(reader) => Monitor::with_project_reader(&path, reader),
            None => Monitor::new(&path),
        };
        Rig { monitor, uploader: FrameUploader::new(&device, &queue), path, writer, device, queue, now: now_ns(), frame: 86400.0, _gpu_turn: turn }
    }

    /// The app polls the Feed once: a frame that arrived is uploaded and handed to `on_frame`.
    pub fn poll(&mut self, mut on_frame: impl FnMut(&mut wgpu::CommandEncoder, &GpuFrame)) {
        let uploader = &mut self.uploader;
        self.monitor.poll(self.now, &mut |frame| uploader.upload(frame, |encoder, gpu_frame| on_frame(encoder, gpu_frame)));
    }

    /// The app's Feed loop takes one step at the rig's clock: the same code the app's Feed thread runs.
    pub fn step(&mut self, feed_loop: &mut FeedLoop) -> Changed {
        feed_loop.step(&mut self.monitor, &mut self.uploader, self.now)
    }

    /// Resolve plays one frame (RGBA, bottom row first); nothing has taken it yet.
    pub fn write(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let meta = FrameMeta { width, height, frame_time: self.frame, frame_rate: 24.0, render_ns: self.now, sender_instance: 1 };
        self.writer.write_frame(&meta, pixels).unwrap();
        self.frame += 1.0;
    }

    /// Resolve plays one frame and the app has a moment to take it: `step` is the app's work, run once per
    /// millisecond for 50 ms.
    pub fn play(&mut self, width: u32, height: u32, pixels: &[f32], step: impl FnMut(&mut Rig)) {
        self.write(width, height, pixels);
        self.wait(50, step);
    }

    /// Time passes: `step` runs once per millisecond.
    pub fn wait(&mut self, ms: u64, mut step: impl FnMut(&mut Rig)) {
        for _ in 0..ms {
            self.now += MS;
            step(self);
        }
    }

    /// Draw into a cleared `width` × `height` target with `draw` and read it back (RGBA8, rows top first).
    pub fn render(&self, width: u32, height: u32, draw: impl FnOnce(&mut wgpu::RenderPass<'_>)) -> Vec<u8> {
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            draw(&mut pass);
        }
        let row = (width * 4).next_multiple_of(256);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * height) as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mapped = readback.get_mapped_range(..).unwrap();
        (0..height as usize).flat_map(|y| mapped[y * row as usize..y * row as usize + width as usize * 4].to_vec()).collect()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The Panels of `count` displays in a `width` × `height` window at `scale`, as the window lays them out.
pub fn layouts(count: usize, width: u32, height: u32, scale: f32) -> Vec<PanelLayout> {
    let cells = PanelGrid::automatic(count).cells([0.0, 0.0, width as f32, height as f32], 8.0 * scale);
    cells.iter().map(|c| PanelLayout::new(*c, scale)).collect()
}

/// A frame of one colour (RGBA, as the Feed carries it).
pub fn solid(width: u32, height: u32, rgb: [f32; 3]) -> Vec<f32> {
    (0..width * height).flat_map(|_| [rgb[0], rgb[1], rgb[2], 1.0]).collect()
}
