//! The CIE chromaticity plot (#62), proven at the Feed seam: a ColorChecker chart goes in through the synthetic
//! writer with a Colour Setup, the app's Feed monitor hands it to the GPU and through the Output Conversion exactly
//! as the app does, and the tests read back what the plot counted, what it drew and its label. Expected xy come
//! from the chart's reference data and the f64 maths of the accuracy crate, never from the shader. Needs a GPU.
use scopecreep::colour_setup::{ColourSetup, KnownColourSetup};
use scopecreep::cie::{CieCounts, CiePlot, Plot, PlotLayout};
use scopecreep::display::{Display, DisplayContext};
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::monitor::Monitor;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::settings::Settings;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_accuracy::colour::{REC709, rgb_to_xyz, vmul};
use scopecreep_feed::{FeedWriter, FrameMeta, Provenance, SentColourSetup, now_ns};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

const MS: u64 = 1_000_000;
const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";
const DWG_P3: &str = "timeline=DaVinci WG/Intermediate\noutput=P3-D65\n";

fn gpu() -> (MutexGuard<'static, ()>, wgpu::Device, wgpu::Queue) {
    static GPU: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (device, queue) = GPU.get_or_init(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() }))
            .expect("these tests need a GPU");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("cie tests"), ..Default::default() })).unwrap()
    });
    (turn, device.clone(), queue.clone())
}

/// A stand-in Resolve and Sender, the app's Feed monitor, upload, Output Conversion and the plot, as `main.rs` wires them.
struct Bench {
    path: PathBuf,
    writer: FeedWriter,
    monitor: Monitor,
    uploader: FrameUploader,
    conversion: OutputConversion,
    plot: Arc<CiePlot>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    now: u64,
    frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Bench {
    fn new(name: &str, colour_setup: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-cie-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let mut writer = FeedWriter::create(&path, 4, 1920 * 1080 * 16).unwrap();
        writer.set_colour_setup(Some(SentColourSetup::new(Provenance::Manual, colour_setup)));
        let mut b = Bench {
            monitor: Monitor::new(&path),
            uploader: FrameUploader::new(&device, &queue),
            conversion: OutputConversion::new(&device, &queue),
            plot: Arc::new(CiePlot::new(&device, &queue, TARGET)),
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

    fn configure(&self, settings: &[(&str, &str)]) {
        let settings: Settings = settings.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        self.plot.configure(&DisplayContext { colour_setup: self.monitor.status().colour_setup(), settings, ..DisplayContext::default() });
    }

    fn poll(&mut self) {
        let (uploader, conversion, plot) = (&mut self.uploader, &mut self.conversion, &self.plot);
        self.monitor.poll(self.now, &mut |frame| {
            uploader.upload(frame, |encoder, gpu_frame| {
                let shown = conversion.convert(encoder, gpu_frame);
                plot.on_frame(encoder, shown);
            })
        });
        // As the app does after every poll: configure from the Colour Setup, then follow it.
        self.configure(&[]);
        let setup = self.monitor.status().colour_setup().map(|c| c.setup);
        let plot = &self.plot;
        self.conversion.follow(setup.as_ref(), self.uploader.frame(), |encoder, shown| plot.on_frame(encoder, shown));
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let meta = FrameMeta { width, height, frame_time: self.frame, frame_rate: 24.0, render_ns: self.now, sender_instance: 1 };
        self.writer.write_frame(&meta, pixels).unwrap();
        self.frame += 1.0;
        for _ in 0..50 {
            self.now += MS;
            self.poll();
        }
    }

    fn counts(&self) -> CieCounts {
        self.plot.counts()
    }

    /// Draw the plot into a `width` x `height` target and read it back (RGBA8, rows top first).
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
            self.plot.draw(&mut pass, [0.0, 0.0, width as f32, height as f32], [width as f32, height as f32], 1.0);
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

/// xy of a patch from the chart's reference data: its linear Rec.709 RGB (the light adapted to D65) to XYZ.
fn reference_xy(rec709_linear: [f64; 3]) -> [f64; 2] {
    let xyz = vmul(rec709_linear, &rgb_to_xyz(&REC709));
    let sum = xyz[0] + xyz[1] + xyz[2];
    [xyz[0] / sum, xyz[1] / sum]
}

#[test]
fn every_colorchecker_patch_lands_at_its_reference_chromaticity_within_0_002() {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    let frame = chart.frame(width, height);
    let mut b = Bench::new("patches", DWG_REC709);
    b.play(width, height, &frame.pixels);
    let counts = b.counts();
    assert_eq!(counts.total(), (width * height) as u64, "every pixel is counted somewhere");
    assert_eq!(counts.outside, 0);
    for patch in chart.patches() {
        let r = frame.patch_rect(patch.number);
        let area = ((r.x1 - r.x0) * (r.y1 - r.y0)) as u64;
        let want = reference_xy(patch.rec709_linear);
        let landed = counts.near(want, 0.002);
        assert!(landed >= area, "patch {} {} (xy {want:.4?}): only {landed} of its {area} pixels are within 0.002", patch.number, patch.name);
    }
}

#[test]
fn the_same_chart_through_a_p3_output_lands_at_the_same_chromaticities() {
    // The light is the same: only the picture's primaries and encoding differ, so xy must not move.
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    let frame = chart.frame(width, height);
    let mut b = Bench::new("p3", DWG_P3);
    b.play(width, height, &frame.pixels);
    let counts = b.counts();
    for patch in chart.patches() {
        let r = frame.patch_rect(patch.number);
        let area = ((r.x1 - r.x0) * (r.y1 - r.y0)) as u64;
        let want = reference_xy(patch.rec709_linear);
        assert!(counts.near(want, 0.002) >= area, "patch {} {}: want {want:.4?}, within 0.01: {}, of {area}", patch.number, patch.name, counts.near(want, 0.01));
    }
}

#[test]
fn black_pixels_have_no_chromaticity_and_are_counted_apart() {
    let mut b = Bench::new("black", DWG_REC709);
    let pixels: Vec<f32> = (0..64 * 36).flat_map(|_| [0.0, 0.0, 0.0, 1.0]).collect();
    b.play(64, 36, &pixels);
    let counts = b.counts();
    assert_eq!(counts.no_light, 64 * 36);
    assert_eq!(counts.total(), 64 * 36);
}

fn known(setup: &str) -> KnownColourSetup {
    KnownColourSetup { setup: ColourSetup::from_feed(&SentColourSetup::new(Provenance::Manual, setup)).unwrap(), provenance: Provenance::Manual }
}

#[test]
fn the_triangles_follow_the_colour_setup_and_the_timeline_one_can_be_turned_off() {
    let plot = Plot::new(Some(known(DWG_P3)), &Settings::new());
    assert_eq!(plot.output_triangle(), [[0.68, 0.32], [0.265, 0.69], [0.15, 0.06]]);
    let dwg = plot.timeline_triangle().expect("DaVinci Wide Gamut is drawn");
    assert_eq!(dwg, [[0.8, 0.313], [0.1682, 0.9877], [0.079, -0.1155]]);
    let off: Settings = [("timeline".to_string(), "off".to_string())].into();
    assert_eq!(Plot::new(Some(known(DWG_P3)), &off).timeline_triangle(), None);

    let aces = Plot::new(Some(known("timeline=ACEScct\noutput=Rec.2100 ST2084 (1000 nit)\naces_version=2.0\n")), &Settings::new());
    assert_eq!(aces.output_triangle(), [[0.708, 0.292], [0.17, 0.797], [0.131, 0.046]]);
    assert_eq!(aces.timeline_triangle(), Some([[0.713, 0.293], [0.165, 0.83], [0.128, 0.044]]));

    let rec709 = Plot::new(Some(known(DWG_REC709)), &Settings::new());
    assert_eq!(rec709.output_triangle(), [[0.64, 0.33], [0.3, 0.6], [0.15, 0.06]]);
}

#[test]
fn the_label_names_both_spaces_and_never_claims_more_than_is_known() {
    let label = Plot::new(Some(known(DWG_P3)), &Settings::new()).label();
    assert!(label.contains("Output P3-D65") && label.contains("gamma 2.6"), "{label}");
    assert!(label.contains("Timeline DaVinci WG/Intermediate"), "{label}");
    let off: Settings = [("timeline".to_string(), "off".to_string())].into();
    let label = Plot::new(Some(known(DWG_P3)), &off).label();
    assert!(label.contains("Timeline DaVinci WG/Intermediate") && label.contains("hidden"), "both spaces still named: {label}");
    let unknown = Plot::new(None, &Settings::new()).label();
    assert!(unknown.contains("Rec.709 assumed, measured as sent, unconverted") && unknown.contains("Timeline space not known"), "{unknown}");
    let other = Plot::new(Some(known("timeline=Arri LogC\noutput=Rec.709 Gamma 2.4\n")), &Settings::new()).label();
    assert!(other.contains("Timeline Arri LogC, gamut not known"), "{other}");
}

#[test]
fn the_plot_offers_trace_brightness_and_the_timeline_triangle_as_settings() {
    let (_turn, device, queue) = gpu();
    let plot = CiePlot::new(&device, &queue, TARGET);
    let defs = plot.setting_defs();
    assert_eq!(defs.iter().map(|d| d.key).collect::<Vec<_>>(), ["brightness", "timeline"]);
    assert_eq!(defs[1].choices.iter().map(|c| c.value).collect::<Vec<_>>(), ["on", "off"]);
    let gains = |v: &str| Plot::new(None, &[("brightness".to_string(), v.to_string())].into()).gain;
    assert!(gains("bright") > gains("normal") && gains("normal") > gains("dim"));
}

fn lit(image: &[u8], width: u32, [x, y]: [f32; 2]) -> u8 {
    // A point of trace is a pixel or two wide: the brightest of the pixel it falls in and its neighbours.
    let (x, y) = (x.floor() as usize, y.floor() as usize);
    (y - 1..=y + 1).flat_map(|y| (x - 1..=x + 1).map(move |x| image[(y * width as usize + x) * 4])).max().unwrap()
}

#[test]
fn the_drawn_plot_shows_the_trace_the_locus_and_both_triangles() {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    let frame = chart.frame(width, height);
    let mut b = Bench::new("draw", DWG_REC709);
    b.play(width, height, &frame.pixels);
    let layout = PlotLayout::new([0.0, 0.0, 600.0, 400.0], 1.0);
    let image = b.render(600, 400);
    let at = |xy: [f32; 2]| lit(&image, 600, layout.screen_of(xy));
    // The 18 % grey patches sit on the D65 white point: bright trace.
    assert!(at([0.3127, 0.3290]) > 200, "trace at the white point: {}", at([0.3127, 0.3290]));
    // Blank plot area is dark, the spectral locus (520 nm) is lit grey.
    assert!(at([0.45, 0.85]) < 20);
    assert!(at([0.0743, 0.8338]) > 60, "locus at 520 nm: {}", at([0.0743, 0.8338]));
    // The Rec.709 red corner is on the Output triangle; DaVinci Wide Gamut's green corner on the Timeline one.
    assert!(at([0.64, 0.33]) > 120, "Output triangle: {}", at([0.64, 0.33]));
    assert!(at([0.1682, 0.9877]) > 60, "Timeline triangle: {}", at([0.1682, 0.9877]));
    // Turned off, the Timeline triangle is gone.
    b.configure(&[("timeline", "off")]);
    let image = b.render(600, 400);
    assert!(lit(&image, 600, layout.screen_of([0.1682, 0.9877])) < 20);
}
