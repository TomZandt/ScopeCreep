//! The saturation heatmap (#64), driven at the Feed seam: synthetic frames go in through the synthetic writer, the app's
//! Feed monitor hands them to the GPU as the app does, and the tests read back what the Panel drew, its label and
//! its legend. Expected values come from the BT.709 constants and the research table
//! through the ColorChecker frames of #52. Needs a GPU.
use scopecreep::muted_palette::{STOPS, muted};
use scopecreep::saturation_heatmap::SaturationHeatmap;
use scopecreep::display::{DisplayContext, DisplaySet};
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::monitor::Monitor;
use scopecreep::settings::PanelSettings;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_accuracy::transfer::bt709_oetf;
use scopecreep_feed::{FeedWriter, FrameMeta, now_ns};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

const MS: u64 = 1_000_000;
const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn gpu() -> (MutexGuard<'static, ()>, wgpu::Device, wgpu::Queue) {
    static GPU: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (device, queue) = GPU.get_or_init(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() }))
            .expect("these tests need a GPU");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("saturation heatmap tests"), ..Default::default() })).unwrap()
    });
    (turn, device.clone(), queue.clone())
}

struct Bench {
    path: PathBuf,
    writer: FeedWriter,
    monitor: Monitor,
    uploader: FrameUploader,
    set: DisplaySet,
    device: wgpu::Device,
    queue: wgpu::Queue,
    now: u64,
    frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-heatmap-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let writer = FeedWriter::create(&path, 4, 1920 * 1080 * 16).unwrap();
        let mut set = DisplaySet::default();
        set.register(Arc::new(SaturationHeatmap::new(&device, &queue, TARGET)));
        Bench { monitor: Monitor::new(&path), uploader: FrameUploader::new(&device, &queue), set, path, writer, device, queue, now: now_ns(), frame: 86400.0, _gpu_turn: turn }
    }

    fn configure(&self, legend: &str, scale: &str) {
        let mut settings = PanelSettings::default();
        settings.set(self.set.name(0), "legend", legend);
        settings.set(self.set.name(0), "scale", scale);
        self.set.configure(&DisplayContext::default(), &settings);
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let meta = FrameMeta { width, height, frame_time: self.frame, frame_rate: 24.0, render_ns: self.now, sender_instance: 1 };
        self.writer.write_frame(&meta, pixels).unwrap();
        self.frame += 1.0;
        for _ in 0..50 {
            self.now += MS;
            let (uploader, set) = (&mut self.uploader, &self.set);
            self.monitor.poll(self.now, &mut |frame| uploader.upload(frame, |encoder, gpu_frame| set.on_frame(encoder, gpu_frame)));
        }
    }

    /// Draw the Panel into a `width` × `height` target at scale 1 and read it back (RGBA8, rows top first).
    fn render(&self, width: u32, height: u32) -> Vec<u8> {
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
            self.set.draw(&mut pass, 0, [0.0, 0.0, width as f32, height as f32], [width as f32, height as f32], 1.0);
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

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn pixel(image: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * width as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// A frame (RGBA, bottom row first) of one solid colour.
fn solid(width: u32, height: u32, rgb: [f32; 3]) -> Vec<f32> {
    (0..width * height).flat_map(|_| [rgb[0], rgb[1], rgb[2], 1.0]).collect()
}

/// BT.709 Cb and Cr from the standard's own constants (1.8556, 1.5748), and the distance from neutral as a share
/// of the vectorscope's 100 % ring (Cb/Cr 0.5).
fn percent(rgb: [f64; 3]) -> f64 {
    let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let (cb, cr) = ((rgb[2] - y) / 1.8556, (rgb[0] - y) / 1.5748);
    cb.hypot(cr) / 0.5 * 100.0
}

/// Where on the scale a drawn colour sits (0 cool to 1 warm), by finding the palette colour nearest to it.
fn position_of(got: [u8; 3]) -> f32 {
    let distance = |t: f32| muted(t).iter().zip(got).map(|(m, g)| (m * 255.0 - g as f32).abs()).fold(0.0, f32::max);
    (0..=2000).map(|i| i as f32 / 2000.0).min_by(|a, b| distance(*a).partial_cmp(&distance(*b)).unwrap()).unwrap()
}

fn assert_on_palette(got: [u8; 3], t: f32, what: &str) {
    let want = muted(t);
    assert!((0..3).all(|c| (got[c] as f32 - want[c] * 255.0).abs() <= 1.5), "{what}: {got:?} is not the palette at {t:.3} ({:?})", want.map(|v| v * 255.0));
}

/// The encoded Output picture of a patch of the research table: linear Rec.709 through the BT.709 OETF, mirrored
/// about zero for a channel below 0 (cyan's red).
fn encoded(linear: [f64; 3]) -> [f64; 3] {
    linear.map(|l| l.signum() * bt709_oetf(l.abs()))
}

/// Patch number, name, encoded colour and drawn colour (the middle of the patch) for the ColorChecker as the
/// Output picture.
fn chart_patches(b: &mut Bench) -> Vec<(usize, &'static str, [f64; 3], [u8; 3])> {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    let layout = chart.frame(width, height);
    let mut pixels = vec![0.0f32; layout.pixels.len()];
    for patch in chart.patches() {
        let r = layout.patch_rect(patch.number);
        let rgb = encoded(patch.rec709_linear);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let i = (((height - 1 - y) * width + x) * 4) as usize; // the frame's bottom row is first
                pixels[i..i + 4].copy_from_slice(&[rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0]);
            }
        }
    }
    b.play(width, height, &pixels);
    let image = b.render(width, height);
    chart
        .patches()
        .iter()
        .map(|patch| {
            let r = layout.patch_rect(patch.number);
            (patch.number, patch.name, encoded(patch.rec709_linear), pixel(&image, width, (r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2))
        })
        .collect()
}

#[test]
fn the_palette_is_cool_to_warm_at_an_even_brightness() {
    assert_eq!(muted(0.0), STOPS[0]);
    assert_eq!(muted(1.0), STOPS[STOPS.len() - 1]);
    assert_eq!(muted(-1.0), muted(0.0));
    assert_eq!(muted(9.0), muted(1.0));
    // Cool is blue and warm is red.
    assert!(STOPS[0][2] > STOPS[0][0] && STOPS[STOPS.len() - 1][0] > STOPS[STOPS.len() - 1][2]);
    // No part of the scale is brighter than another.
    let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    for i in 0..=100 {
        let l = luma(muted(i as f32 / 100.0));
        assert!((l - 0.56).abs() < 0.02, "luma {l} at {i} %");
    }
}

#[test]
fn the_display_is_registered_in_the_standard_set() {
    let (_turn, device, queue) = gpu();
    let set = DisplaySet::standard(&device, &queue, TARGET, false);
    assert!(set.names().contains(&"Saturation heatmap"), "{:?}", set.names());
}

#[test]
fn the_grey_patches_are_at_the_cool_end() {
    let mut b = Bench::new("greys");
    b.configure("off", "100");
    let patches = chart_patches(&mut b);
    for (number, name, _, got) in patches.iter().filter(|p| p.0 >= 19) {
        // The chart's greys are neutral to within the research table's rounding: under 3 % of the scale.
        let at = position_of(*got);
        assert!(at <= 0.03, "patch {number} {name} is drawn at {at:.3} of the scale");
    }
}

#[test]
fn colour_bars_at_100_percent_sit_at_the_100_percent_point_and_75_percent_bars_below_it() {
    let mut b = Bench::new("bars");
    b.configure("off", "100");
    for level in [1.0f64, 0.75] {
        let colours = [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]].map(|c: [f64; 3]| c.map(|v| v * level));
        let (w, h) = (60u32, 36u32);
        let pixels: Vec<f32> = (0..w * h).flat_map(|i| colours[(i % w / 10) as usize].map(|v| v as f32).into_iter().chain([1.0])).collect();
        b.play(w, h, &pixels);
        let image = b.render(w, h);
        for (i, c) in colours.iter().enumerate() {
            let got = pixel(&image, w, i as u32 * 10 + 5, 18);
            let at = percent(*c);
            // Bars scale with their level, and 100 % bars are within a fifth of the 100 % point (green and
            // magenta are the furthest out, at 119 %).
            assert!((at / level - 100.0).abs() <= 20.0, "bar {i} at {at:.1} %");
            assert_on_palette(got, (at / 100.0).min(1.0) as f32, &format!("{level} bar {i}"));
        }
    }
}

#[test]
fn each_colorchecker_patch_shows_its_cb_cr_distance_on_the_scale() {
    let mut b = Bench::new("patches");
    b.configure("off", "100");
    for (number, name, rgb, got) in chart_patches(&mut b) {
        let want = (percent(rgb) / 100.0).min(1.0) as f32;
        let at = position_of(got);
        assert!((at - want).abs() <= 0.01, "patch {number} {name}: drawn at {at:.3} of the scale, Cb/Cr distance says {want:.3}");
    }
}

#[test]
fn changing_the_scale_remaps_the_colours() {
    let mut b = Bench::new("scale");
    // A muted orange, about 36 % saturated.
    let rgb = [0.75f64, 0.5, 0.25];
    let at = percent(rgb);
    assert!(at > 20.0 && at < 50.0, "{at}");
    b.play(40, 40, &solid(40, 40, rgb.map(|v| v as f32)));
    for scale in ["100", "50", "25", "200"] {
        b.configure("off", scale);
        let image = b.render(40, 40);
        let want = (at / scale.parse::<f64>().unwrap()).min(1.0) as f32;
        assert_on_palette(pixel(&image, 40, 20, 20), want, &format!("scale {scale}"));
    }
    // The offered scales, the first the default.
    let defs = b.set.setting_defs(0);
    assert_eq!(defs[0].key, "scale");
    assert_eq!(defs[0].choices[0].value, "100");
    // The label says the scale.
    b.configure("off", "50");
    assert!(b.set.labels()[0].contains("warm 50 %"), "{}", b.set.labels()[0]);
    b.configure("off", "200");
    assert!(b.set.labels()[0].contains("warm 200 %"), "{}", b.set.labels()[0]);
}

#[test]
fn the_legend_toggles_and_shows_the_palette_with_its_ends() {
    let mut b = Bench::new("legend");
    let (width, height) = (600u32, 400u32);
    b.play(width, height, &solid(width, height, [0.5, 0.5, 0.5]));

    b.configure("on", "100");
    let defs = b.set.setting_defs(0);
    assert_eq!(defs[1].key, "legend");
    assert_eq!(defs[1].choices.iter().map(|c| c.value).collect::<Vec<_>>(), ["on", "off"]);
    let image = b.render(width, height);
    // The bar under the picture runs cool (left) to warm (right).
    assert_on_palette(pixel(&image, width, 2, height - 26 + 4), 0.0, "legend left");
    assert_on_palette(pixel(&image, width, width - 3, height - 26 + 4), 1.0, "legend right");
    let middle = position_of(pixel(&image, width, width / 2, height - 26 + 4));
    assert!((middle - 0.5).abs() < 0.02, "{middle}");
    let texts = b.set.scale_texts(0, [0.0, 0.0, width as f32, height as f32], 1.0);
    assert_eq!(texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["0 %", "100 %"]);
    // The picture still shows above it (a grey: the cool end).
    assert_on_palette(pixel(&image, width, 300, 100), 0.0, "the picture");
    b.configure("on", "50");
    assert_eq!(b.set.scale_texts(0, [0.0, 0.0, width as f32, height as f32], 1.0)[1].text, "50 %");

    // Off: no bar, no text, the picture takes the whole Panel.
    b.configure("off", "100");
    let image = b.render(width, height);
    assert_on_palette(pixel(&image, width, width - 3, height - 26 + 4), 0.0, "picture runs to the bottom");
    assert!(b.set.scale_texts(0, [0.0, 0.0, width as f32, height as f32], 1.0).is_empty());
}

#[test]
fn the_label_says_what_is_measured_and_that_the_picture_is_assumed_until_the_colour_setup_is_known() {
    let b = Bench::new("label");
    b.configure("on", "100");
    let label = b.set.labels()[0].clone();
    assert!(label.starts_with("Saturation heatmap"), "{label}");
    assert!(label.contains("Output picture"), "{label}");
    assert!(label.contains("Rec.709 assumed, unconverted"), "{label}");
    assert!(label.contains("100 % = the vectorscope's 100 % ring"), "{label}");
}

#[test]
fn nothing_is_drawn_until_a_frame_has_arrived() {
    let b = Bench::new("empty");
    b.configure("on", "100");
    let image = b.render(200, 100);
    assert!(image.chunks(4).all(|p| p[..3] == [0, 0, 0]));
    assert!(b.set.scale_texts(0, [0.0, 0.0, 200.0, 100.0], 1.0).is_empty());
}
