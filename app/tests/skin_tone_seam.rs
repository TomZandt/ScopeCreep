//! The Skin Tone Panel (#63), proven at the Feed seam: synthetic frames go in through the synthetic writer, the
//! app's Feed monitor and Output Conversion hand them to the display exactly as the app does, and the test reads
//! back what the Panel drew and its label. Expected hues come from the ColorChecker chart code (#52). Needs a GPU.
use scopecreep::display::{Display, DisplayContext};
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::skin_tone_view::SkinTone;
use scopecreep::monitor::Monitor;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::settings::PanelSettings;
use scopecreep::skin_tone::SKIN_TONE_LINE_DEGREES;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_feed::{FeedWriter, FrameMeta, Provenance, SentColourSetup, now_ns};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

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
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("skin tone tests"), ..Default::default() })).unwrap()
    });
    (turn, device.clone(), queue.clone())
}

struct Bench {
    path: PathBuf,
    writer: FeedWriter,
    monitor: Monitor,
    uploader: FrameUploader,
    conversion: OutputConversion,
    display: SkinTone,
    device: wgpu::Device,
    queue: wgpu::Queue,
    now: u64,
    frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Bench {
    /// `colour_setup`: the Sender's manual override (`key=value` lines), or none for a picture shown as sent.
    fn new(name: &str, colour_setup: Option<&str>) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-skintone-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let mut writer = FeedWriter::create(&path, 4, 1920 * 1080 * 16).unwrap();
        writer.set_colour_setup(colour_setup.map(|s| SentColourSetup::new(Provenance::Manual, s)));
        let mut b = Bench {
            monitor: Monitor::new(&path),
            uploader: FrameUploader::new(&device, &queue),
            conversion: OutputConversion::new(&device, &queue),
            display: SkinTone::new(&device, &queue, TARGET, false),
            path,
            writer,
            device,
            queue,
            now: now_ns(),
            frame: 86400.0,
            _gpu_turn: turn,
        };
        b.poll();
        b.configure(None);
        b
    }

    fn poll(&mut self) {
        let (uploader, conversion, display) = (&mut self.uploader, &mut self.conversion, &self.display);
        self.monitor.poll(self.now, &mut |frame| {
            uploader.upload(frame, |encoder, gpu_frame| {
                let shown = conversion.convert(encoder, gpu_frame);
                display.on_frame(encoder, shown);
            })
        });
        let setup = self.monitor.status().colour_setup().map(|c| c.setup);
        let display = &self.display;
        self.conversion.follow(setup.as_ref(), self.uploader.frame(), |encoder, shown| display.on_frame(encoder, shown));
    }

    /// The Colour Setup the Feed carries and the user's tolerance choice (degrees; none is the default).
    fn configure(&self, tolerance: Option<&str>) {
        let mut settings = PanelSettings::default();
        if let Some(tolerance) = tolerance {
            settings.set(self.display.name(), "tolerance", tolerance);
        }
        let known = self.monitor.status().colour_setup();
        let colour_label = known.as_ref().map_or("Colour Setup unknown".to_string(), |c| c.setup.timeline.clone());
        let context = DisplayContext { colour_setup: known, colour_label, settings: settings.resolve(self.display.name(), self.display.setting_defs()), ..DisplayContext::default() };
        self.display.configure(&context);
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let meta = FrameMeta { width, height, frame_time: self.frame, frame_rate: 24.0, render_ns: self.now, sender_instance: 1 };
        self.writer.write_frame(&meta, pixels).unwrap();
        self.frame += 1.0;
        for _ in 0..50 {
            self.now += MS;
            self.poll();
        }
        self.configure(None);
    }

    /// What the Panel drew into a `width` × `height` target that has the frame's own shape (RGBA8, rows top first).
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
            self.display.draw(&mut pass, [0.0, 0.0, width as f32, height as f32], [width as f32, height as f32], 1.0);
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

/// Within a rounding step of a solid colour.
fn is_solid(shown: [u8; 3], solid: [f32; 3]) -> bool {
    shown.iter().zip(solid).all(|(&c, s)| (c as f32 - s * 255.0).abs() <= 2.0)
}

const YELLOW: [f32; 3] = [1.0, 0.85, 0.0];
const GREEN: [f32; 3] = [0.1, 0.85, 0.2];
const MAGENTA: [f32; 3] = [0.95, 0.2, 0.85];

/// Painted by the Panel: one of its three solid colours.
fn is_painted(shown: [u8; 3]) -> bool {
    is_solid(shown, YELLOW) || is_solid(shown, GREEN) || is_solid(shown, MAGENTA)
}

/// Solid yellow, at full strength (not a tint mixed with grey).
fn is_yellow(shown: [u8; 3]) -> bool {
    is_solid(shown, YELLOW)
}

const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";

/// A flat frame at `offset` degrees from the shared skin-tone line (BT.709 Cb/Cr angle), luma `y`, saturation
/// `chroma`, shown as sent. The weights are the ones the Panel uses when no Colour Setup is known.
fn hue_frame(offset: f64, y: f64, chroma: f64) -> Vec<f32> {
    let [wr, wg, wb] = scopecreep::output_conversion::measurement(None, scopecreep::waveform::LumaWeights::FromPrimaries).weights();
    let angle = (SKIN_TONE_LINE_DEGREES + offset).to_radians();
    let (cb, cr) = (chroma * angle.cos(), chroma * angle.sin());
    let r = y + 2.0 * (1.0 - wr) * cr;
    let b = y + 2.0 * (1.0 - wb) * cb;
    let g = (y - wr * r - wb * b) / wg;
    (0..64 * 36).flat_map(|_| [r as f32, g as f32, b as f32, 1.0]).collect()
}

fn centre(b: &Bench) -> [u8; 3] {
    pixel(&b.render(64, 36), 64, 32, 18)
}

fn shown_at(b: &mut Bench, offset: f64) -> [u8; 3] {
    b.play(64, 36, &hue_frame(offset, 0.5, 0.15));
    centre(b)
}

/// Hue difference from the skin-tone line, wrapped to -180..180.
fn offset_of(hue: f64) -> f64 {
    (hue - SKIN_TONE_LINE_DEGREES + 540.0).rem_euclid(360.0) - 180.0
}

#[test]
fn the_skin_patches_are_painted_solid_and_the_other_patches_keep_their_own_colour() {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    let frame = chart.frame(width, height);
    let mut b = Bench::new("chart", Some(DWG_REC709));
    b.play(width, height, &frame.pixels);
    let image = b.render(width, height);
    for patch in chart.patches() {
        let offset = offset_of(patch.hue_degrees);
        let r = frame.patch_rect(patch.number);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let shown = pixel(&image, width, x, y);
                let what = format!("patch {} {} ({offset:+.1} deg from the line) shows {shown:?}", patch.number, patch.name);
                match patch.number {
                    // Dark skin sits 2 degrees below the line Resolve draws: solid yellow.
                    1 => assert!(is_yellow(shown), "{what}"),
                    // Light skin is 5 degrees below it, at the edge of the on-line band: solid yellow or the magenta side.
                    2 => assert!(is_yellow(shown) || is_solid(shown, MAGENTA), "{what}"),
                    // Orange is 9 degrees above the line: solid green.
                    7 => assert!(is_solid(shown, GREEN), "{what}"),
                    // The neutral patches have no hue to speak of: their own (grey) colour.
                    19..=24 => assert!(!is_painted(shown) && (shown[0] as i32 - shown[2] as i32).abs() <= 12, "{what}"),
                    // Blue sky and the others 30 degrees or more away keep their own colour.
                    _ if offset.abs() >= 30.0 => assert!(!is_painted(shown), "{what}"),
                    _ => {}
                }
            }
        }
    }
    // Blue sky keeps its own blue.
    let r = frame.patch_rect(3);
    let sky = pixel(&image, width, (r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2);
    assert!(sky[2] > sky[0] && sky[2] > sky[1], "blue sky stays blue: {sky:?}");
}

#[test]
fn hues_either_side_of_the_line_are_solid_green_and_solid_magenta_out_to_the_outer_band() {
    let mut b = Bench::new("sides", None);
    assert!(is_yellow(shown_at(&mut b, 0.0)), "on the line is yellow: {:?}", centre(&b));
    for offset in [8.0, 15.0, 28.0] {
        assert!(is_solid(shown_at(&mut b, offset), GREEN), "+{offset}: {:?}", centre(&b));
        assert!(is_solid(shown_at(&mut b, -offset), MAGENTA), "-{offset}: {:?}", centre(&b));
    }
}

#[test]
fn hues_far_from_the_line_neutral_pixels_and_any_brightness_behave() {
    let mut b = Bench::new("far", None);
    for offset in [-60.0, -35.0, 35.0, 60.0, 120.0, 180.0] {
        let shown = shown_at(&mut b, offset);
        assert!(!is_painted(shown), "{offset} deg from the line keeps its own colour: {shown:?}");
    }
    // A hue on the line but nearly neutral keeps its own (near-grey) colour.
    b.play(64, 36, &hue_frame(0.0, 0.5, 0.004));
    assert!(!is_painted(centre(&b)), "{:?}", centre(&b));
    // Only hue matters: the same hue dark and light is painted the same way.
    for y in [0.15, 0.85] {
        b.play(64, 36, &hue_frame(0.0, y, 0.1));
        assert!(is_yellow(centre(&b)), "luma {y}: {:?}", centre(&b));
    }
}

#[test]
fn the_tolerance_widens_and_narrows_the_on_line_band() {
    let mut b = Bench::new("tolerance", None);
    b.play(64, 36, &hue_frame(12.0, 0.5, 0.15));
    // 12 degrees out at the default 5 degrees: green.
    assert!(is_solid(centre(&b), GREEN), "{:?}", centre(&b));
    // Widen to 10 (band out to 60): 12 is just off the line, still green; widen the line itself with 15 below.
    b.configure(Some("10"));
    assert!(is_solid(centre(&b), GREEN), "{:?}", centre(&b));
    b.play(64, 36, &hue_frame(8.0, 0.5, 0.15));
    b.configure(Some("10"));
    assert!(is_yellow(centre(&b)), "{:?}", centre(&b));
    // Narrow to 2 (band out to 12): 15 degrees out is past it and keeps its own colour.
    b.play(64, 36, &hue_frame(15.0, 0.5, 0.15));
    b.configure(Some("2"));
    assert!(!is_painted(centre(&b)), "{:?}", centre(&b));
}

#[test]
fn the_tolerance_is_a_list_of_choices_with_five_degrees_first_and_the_label_names_the_bands_and_the_angle() {
    let mut b = Bench::new("label", Some(DWG_REC709));
    let defs = b.display.setting_defs();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].key, "tolerance");
    assert_eq!(defs[0].choices[0].value, "5");
    b.play(16, 9, &(0..16 * 9).flat_map(|_| [0.5, 0.5, 0.5, 1.0]).collect::<Vec<f32>>());
    let label = b.display.label();
    assert!(label.starts_with("Skin tone"), "{label}");
    assert!(label.contains("±5°") && label.contains("±30°"), "{label}");
    assert!(label.contains("123° (as Resolve 21.2 draws it)"), "{label}");
    assert!(label.contains("Output picture") && label.contains("DaVinci WG/Intermediate") && label.contains("Output Conversion"), "{label}");
    b.configure(Some("10"));
    let label = b.display.label();
    assert!(label.contains("±10°") && label.contains("±60°"), "{label}");
}

#[test]
fn nothing_is_drawn_before_a_frame_arrives() {
    let b = Bench::new("empty", None);
    assert_eq!(pixel(&b.render(64, 36), 64, 32, 18), [0, 0, 0]);
    assert_eq!(b.display.label(), "Skin tone");
}
