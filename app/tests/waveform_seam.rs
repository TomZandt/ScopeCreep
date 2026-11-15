//! The luma waveform, driven at the Feed (spec #2, "Testing Decisions"): synthetic frames go in through the
//! synthetic writer, the app's Feed monitor hands them to the GPU exactly as the app does, and the tests look at
//! what a colourist would see: how many pixels landed at each code value of each column, the label, and the
//! waveform as drawn. Needs a GPU.
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::monitor::Monitor;
use scopecreep::waveform::{LumaWeights, MeasuredSpace, Measurement, PlotLayout, Waveform, WaveformCounts};
use scopecreep_feed::{FeedWriter, FrameMeta, now_ns};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

const MS: u64 = 1_000_000;

/// One GPU device for all the tests, used by one test at a time: the graphics memory is shared with Resolve.
fn gpu() -> (MutexGuard<'static, ()>, wgpu::Device, wgpu::Queue) {
    static GPU: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (device, queue) = GPU.get_or_init(make_gpu);
    (turn, device.clone(), queue.clone())
}

fn make_gpu() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("these tests need a GPU");
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("waveform tests"), ..Default::default() })).unwrap()
}

/// A stand-in Resolve (the synthetic writer), the app's Feed monitor, and the app's GPU side of the Feed: the
/// frame upload and the waveform.
struct Bench {
    path: PathBuf,
    writer: FeedWriter,
    monitor: Monitor,
    uploader: FrameUploader,
    waveform: Waveform,
    device: wgpu::Device,
    queue: wgpu::Queue,
    now: u64,
    frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-waveform-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let writer = FeedWriter::create(&path, 4, 1920 * 1080 * 16).unwrap();
        let mut b = Bench {
            monitor: Monitor::new(&path),
            uploader: FrameUploader::new(&device, &queue),
            waveform: Waveform::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm),
            path,
            writer,
            device,
            queue,
            now: now_ns(),
            frame: 86400.0,
            _gpu_turn: turn,
        };
        b.poll();
        b
    }

    fn poll(&mut self) {
        let (uploader, waveform) = (&mut self.uploader, &self.waveform);
        self.monitor.poll(self.now, &mut |frame| uploader.upload(frame, |encoder, gpu_frame| waveform.measure(encoder, gpu_frame)));
    }

    /// Resolve plays one frame (RGBA, bottom row first, as the Sender writes it) and the app has a moment to
    /// take it.
    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) -> WaveformCounts {
        let meta = FrameMeta { width, height, frame_time: self.frame, frame_rate: 24.0, render_ns: self.now, sender_instance: 1 };
        self.writer.write_frame(&meta, pixels).unwrap();
        self.frame += 1.0;
        for _ in 0..50 {
            self.now += MS;
            self.poll();
        }
        self.waveform.counts()
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A picture `width` × `height` where each pixel's red, green and blue are given by `rgb(x, row from the top)`.
fn picture(width: u32, height: u32, rgb: impl Fn(u32, u32) -> [f32; 3]) -> Vec<f32> {
    let mut out = Vec::with_capacity(width as usize * height as usize * 4);
    for row in 0..height {
        let from_top = height - 1 - row; // the Feed stores rows bottom first
        for x in 0..width {
            let [r, g, b] = rgb(x, from_top);
            out.extend_from_slice(&[r, g, b, 1.0]);
        }
    }
    out
}

/// The grey whose luma is exactly 10-bit code `code`.
fn grey(code: u32) -> [f32; 3] {
    let v = code as f32 / 1023.0;
    [v, v, v]
}

// ------------------------------------------------------------------ ramps

#[test]
fn a_horizontal_ramp_puts_every_pixel_of_column_x_at_code_x() {
    let mut b = Bench::new("hramp");
    let counts = b.play(1024, 9, &picture(1024, 9, |x, _| grey(x)));
    assert_eq!(counts.columns(), 1024, "one waveform column per picture column");
    for x in 0..1024 {
        for code in 0..1024 {
            assert_eq!(counts.at(x, code), if code == x { 9 } else { 0 }, "column {x}, code {code}");
        }
        assert_eq!((counts.below(x), counts.above(x)), (0, 0), "column {x}: nothing out of range");
    }
}

#[test]
fn a_vertical_ramp_puts_one_pixel_of_every_column_at_every_code() {
    let mut b = Bench::new("vramp");
    // Three columns: an odd width, so the rows of the frame are not a convenient size for the GPU.
    let counts = b.play(3, 1024, &picture(3, 1024, |_, from_top| grey(1023 - from_top)));
    assert_eq!(counts.columns(), 3);
    for x in 0..3 {
        for code in 0..1024 {
            assert_eq!(counts.at(x, code), 1, "column {x}, code {code}");
        }
    }
    assert_eq!(counts.total(), 3 * 1024, "every pixel counted once");
}

// ------------------------------------------------------------------ luma weights

/// Plays three columns of half-strength red, green and blue, and returns the code each one lands on.
fn half_primaries_codes(b: &mut Bench) -> [u32; 3] {
    let counts = b.play(3, 4, &picture(3, 4, |x, _| {
        let mut rgb = [0.0; 3];
        rgb[x as usize] = 0.5;
        rgb
    }));
    [0, 1, 2].map(|x| (0..1024).find(|&code| counts.at(x, code) == 4).unwrap_or_else(|| panic!("column {x} is not on one code")))
}

fn measuring(space: MeasuredSpace, luma: LumaWeights) -> Measurement {
    Measurement { space, assumed: false, luma }
}

// Expected codes: 0.5 × weight × 1023, rounded, with the weights ADR 0004 quotes for each space.
#[test]
fn rec709_luma_uses_the_rec709_weights() {
    let mut b = Bench::new("w709");
    b.waveform.set_measurement(measuring(MeasuredSpace::Rec709, LumaWeights::FromPrimaries));
    assert_eq!(half_primaries_codes(&mut b), [109, 366, 37]); // 0.2126, 0.7152, 0.0722
    assert!(b.waveform.label().ends_with("Rec.709  ·  Y = 0.2126 R + 0.7152 G + 0.0722 B from primaries"), "{}", b.waveform.label());
}

#[test]
fn p3_d65_luma_uses_weights_derived_from_its_primaries() {
    let mut b = Bench::new("wp3");
    b.waveform.set_measurement(measuring(MeasuredSpace::P3D65, LumaWeights::FromPrimaries));
    assert_eq!(half_primaries_codes(&mut b), [117, 354, 41]); // 0.2290, 0.6917, 0.0793
    assert!(b.waveform.label().ends_with("P3-D65  ·  Y = 0.2290 R + 0.6917 G + 0.0793 B from primaries"), "{}", b.waveform.label());
}

#[test]
fn rec2020_luma_uses_the_rec2020_weights() {
    let mut b = Bench::new("w2020");
    b.waveform.set_measurement(measuring(MeasuredSpace::Rec2020, LumaWeights::FromPrimaries));
    assert_eq!(half_primaries_codes(&mut b), [134, 347, 30]); // 0.2627, 0.6780, 0.0593
    assert!(b.waveform.label().ends_with("Rec.2020  ·  Y = 0.2627 R + 0.6780 G + 0.0593 B from primaries"), "{}", b.waveform.label());
}

#[test]
fn match_resolve_uses_rec709_weights_for_p3() {
    let mut b = Bench::new("wmatchp3");
    b.waveform.set_measurement(measuring(MeasuredSpace::P3D65, LumaWeights::MatchResolve));
    assert_eq!(half_primaries_codes(&mut b), [109, 366, 37]);
    assert!(b.waveform.label().ends_with("P3-D65  ·  Y = 0.2126 R + 0.7152 G + 0.0722 B to match Resolve"), "{}", b.waveform.label());
    b.waveform.set_measurement(measuring(MeasuredSpace::P3Dci, LumaWeights::MatchResolve));
    assert_eq!(half_primaries_codes(&mut b), [109, 366, 37]);
}

#[test]
fn match_resolve_uses_rec2020_weights_for_rec2100() {
    let mut b = Bench::new("wmatch2100");
    b.waveform.set_measurement(measuring(MeasuredSpace::Rec2100Pq, LumaWeights::MatchResolve));
    assert_eq!(half_primaries_codes(&mut b), [134, 347, 30]);
    assert!(b.waveform.label().ends_with("Rec.2100 PQ  ·  Y = 0.2627 R + 0.6780 G + 0.0593 B to match Resolve"), "{}", b.waveform.label());
}

#[test]
fn until_the_colour_setup_is_known_the_label_says_rec709_is_assumed() {
    let mut b = Bench::new("assumed");
    assert_eq!(half_primaries_codes(&mut b), [109, 366, 37]);
    assert_eq!(b.waveform.label(), "Luma  ·  10-bit 0–1023  ·  Rec.709 assumed  ·  Y = 0.2126 R + 0.7152 G + 0.0722 B from primaries");
}

// ------------------------------------------------------------------ out of range

/// An HD frame of mid grey with one pixel far above the scale at column 1000 and one below it at column 7.
fn grey_with_two_stray_pixels() -> Vec<f32> {
    picture(1920, 1080, |x, from_top| match (x, from_top) {
        (1000, 300) => [1.5, 1.5, 1.5],
        (7, 900) => [-0.2, -0.2, -0.2],
        _ => grey(512),
    })
}

#[test]
fn a_single_out_of_range_pixel_is_counted_in_its_column() {
    let mut b = Bench::new("stray");
    let counts = b.play(1920, 1080, &grey_with_two_stray_pixels());
    assert_eq!(counts.total(), 1920 * 1080, "no pixel is cut off");
    for x in 0..1920 {
        assert_eq!(counts.above(x), if x == 1000 { 1 } else { 0 }, "above 1023, column {x}");
        assert_eq!(counts.below(x), if x == 7 { 1 } else { 0 }, "below 0, column {x}");
    }
    assert_eq!(counts.at(1000, 1023), 0, "not piled onto code 1023");
    assert_eq!(counts.at(7, 0), 0, "not piled onto code 0");
}

/// Draws the waveform into a `width` × `height` picture and returns it as RGBA bytes, top row first.
fn draw(b: &Bench, width: u32, height: u32) -> Vec<u8> {
    let (device, queue) = (&b.device, &b.queue);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
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
        b.waveform.draw(&mut pass, [0.0, 0.0, width as f32, height as f32], [width as f32, height as f32], 1.0);
    }
    let row = (width * 4).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * height) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None } },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    readback.map_async(wgpu::MapMode::Read, .., |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = readback.get_mapped_range(..).unwrap();
    (0..height as usize).flat_map(|y| mapped[y * row as usize..y * row as usize + width as usize * 4].to_vec()).collect()
}

#[test]
fn a_single_out_of_range_pixel_is_visible_in_the_edge_rows() {
    let mut b = Bench::new("strayvisible");
    b.play(1920, 1080, &grey_with_two_stray_pixels());
    let (width, height) = (800, 500);
    let image = draw(&b, width, height);
    let layout = PlotLayout::new([0.0, 0.0, width as f32, height as f32], 1.0);
    let brightness = |x: f32, y: f32| image[(y as usize * width as usize + x as usize) * 4] as u32;
    let brightest_in = |rect: [f32; 4], column: u32| {
        let x = layout.x_of_column(column, 1920);
        (rect[1] as u32..(rect[1] + rect[3]) as u32).map(|y| brightness(x, y as f32)).max().unwrap()
    };
    let (above, below) = (layout.above(), layout.below());
    assert!(brightest_in(above, 1000) >= 128, "the pixel above 1023 lights the top edge row: {}", brightest_in(above, 1000));
    assert!(brightest_in(above, 1400) < 64, "elsewhere the top edge row stays dark: {}", brightest_in(above, 1400));
    assert!(brightest_in(below, 7) >= 128, "the pixel below 0 lights the bottom edge row: {}", brightest_in(below, 7));
    assert!(brightest_in(below, 1400) < 64, "elsewhere the bottom edge row stays dark: {}", brightest_in(below, 1400));
    let mid_grey = layout.y_of_code(512);
    assert!(brightness(layout.x_of_column(1400, 1920), mid_grey) >= 128, "the mid-grey trace is drawn");
}
