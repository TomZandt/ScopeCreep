//! Highlight Neutrals (#65), proven at the Feed seam: synthetic frames go in through the synthetic writer, the
//! app's Feed monitor and Output Conversion hand them to the display exactly as the app does, and the test reads
//! back what the Panel drew and its label. Expected neutrals come from the ColorChecker chart code (#52). Needs a GPU.
use scopecreep::display::{Display, DisplayContext};
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::highlight_neutrals::HighlightNeutrals;
use scopecreep::monitor::Monitor;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::settings::PanelSettings;
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
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("highlight neutrals tests"), ..Default::default() })).unwrap()
    });
    (turn, device.clone(), queue.clone())
}

struct Bench {
    path: PathBuf,
    writer: FeedWriter,
    monitor: Monitor,
    uploader: FrameUploader,
    conversion: OutputConversion,
    display: HighlightNeutrals,
    device: wgpu::Device,
    queue: wgpu::Queue,
    now: u64,
    frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Bench {
    /// `colour_setup`: the Sender's manual override (`key=value` lines), or none for a picture shown as sent.
    fn new(name: &str, colour_setup: Option<&str>) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-neutrals-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let mut writer = FeedWriter::create(&path, 4, 1920 * 1080 * 16).unwrap();
        writer.set_colour_setup(colour_setup.map(|s| SentColourSetup::new(Provenance::Manual, s)));
        let mut b = Bench {
            monitor: Monitor::new(&path),
            uploader: FrameUploader::new(&device, &queue),
            conversion: OutputConversion::new(&device, &queue),
            display: HighlightNeutrals::new(&device, &queue, TARGET, false),
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

    /// The Colour Setup the Feed carries and the user's tolerance choice (a percentage; none is the default).
    fn configure(&self, tolerance: Option<&str>) {
        let mut settings = PanelSettings::default();
        if let Some(tolerance) = tolerance {
            settings.set(self.display.name(), "tolerance", tolerance);
        }
        let known = self.monitor.status().colour_setup();
        let colour_label = known.as_ref().map_or("Colour Setup unknown".to_string(), |c| c.setup.timeline.clone());
        let context = DisplayContext { colour_setup: known, colour_label, settings: settings.resolve(self.display.name(), self.display.setting_defs()) };
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

/// Shown green: the green channel clearly above both others.
fn is_green([r, g, b]: [u8; 3]) -> bool {
    g as i32 > r as i32 + 60 && g as i32 > b as i32 + 60
}

/// Shown grey: all three channels equal.
fn is_grey([r, g, b]: [u8; 3]) -> bool {
    r == g && g == b
}

const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";
const ACESCCT_REC709: &str = "timeline=ACEScct\noutput=Rec.709 BT.1886\naces_version=2.0\n";

fn assert_chart_greys_green_and_colours_not(name: &str, space: TimelineSpace, setup: &str) {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(space);
    let frame = chart.frame(width, height);
    let mut b = Bench::new(name, Some(setup));
    b.play(width, height, &frame.pixels);
    let image = b.render(width, height);
    for patch in chart.patches() {
        let r = frame.patch_rect(patch.number);
        let (mut green, mut total) = (0, 0);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                total += 1;
                green += is_green(pixel(&image, width, x, y)) as u32;
            }
        }
        if patch.number >= 19 {
            assert_eq!(green, total, "{space:?} grey patch {} {} is green all over", patch.number, patch.name);
        } else {
            assert_eq!(green, 0, "{space:?} coloured patch {} {} has no green", patch.number, patch.name);
        }
    }
}

#[test]
fn every_grey_colorchecker_patch_is_green_in_a_davinci_intermediate_chart_and_no_coloured_patch_is() {
    assert_chart_greys_green_and_colours_not("di", TimelineSpace::DavinciIntermediate, DWG_REC709);
}

#[test]
fn every_grey_colorchecker_patch_is_green_in_an_acescct_chart_and_no_coloured_patch_is() {
    assert_chart_greys_green_and_colours_not("acescct", TimelineSpace::AcesCct, ACESCCT_REC709);
}

/// A flat frame whose red and green are `base` and whose blue is `base + offset`, shown as sent (the values are
/// exactly what the display sees).
fn offset_grey(base: f32, offset: f32) -> Vec<f32> {
    (0..64 * 36).flat_map(|_| [base, base, base + offset, 1.0]).collect()
}

fn centre(b: &Bench) -> [u8; 3] {
    pixel(&b.render(64, 36), 64, 32, 18)
}

#[test]
fn a_grey_with_a_small_channel_offset_turns_green_only_once_the_tolerance_covers_it() {
    let mut b = Bench::new("offset", None);
    b.play(64, 36, &offset_grey(0.5, 0.04));
    // The offset is 4 % of the range: not neutral at the default 3 %, so it stays grey; neutral from 5 % on.
    assert!(is_grey(centre(&b)), "4 % off at 3 % tolerance stays grey: {:?}", centre(&b));
    b.configure(Some("5"));
    assert!(is_green(centre(&b)), "4 % off at 5 % tolerance is green: {:?}", centre(&b));
    b.configure(Some("1"));
    assert!(is_grey(centre(&b)), "4 % off at 1 % tolerance is grey again: {:?}", centre(&b));
    // 2 % off is within the default 3 %.
    b.play(64, 36, &offset_grey(0.5, 0.02));
    assert!(is_green(centre(&b)), "2 % off at the default 3 % is green: {:?}", centre(&b));
}

#[test]
fn a_coloured_pixel_is_shown_in_grey_that_is_its_luma() {
    let mut b = Bench::new("colour", None);
    b.play(64, 36, &(0..64 * 36).flat_map(|_| [1.0, 0.0, 0.0, 1.0]).collect::<Vec<f32>>());
    let [r, g, bl] = centre(&b);
    assert!(is_grey([r, g, bl]), "{r} {g} {bl}");
    // Rec.709's red weight, 0.2126, as the waveform weighs it when the Colour Setup is unknown (target not sRGB).
    assert!((r as i32 - (0.2126_f32 * 255.0).round() as i32).abs() <= 1, "{r}");
}

#[test]
fn the_tolerance_is_a_list_of_choices_with_three_percent_first_and_the_label_names_it() {
    let mut b = Bench::new("label", Some(DWG_REC709));
    let defs = b.display.setting_defs();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].key, "tolerance");
    assert_eq!(defs[0].choices[0].value, "3");
    b.play(16, 9, &(0..16 * 9).flat_map(|_| [0.5, 0.5, 0.5, 1.0]).collect::<Vec<f32>>());
    let label = b.display.label();
    assert!(label.starts_with("Highlight neutrals"), "{label}");
    assert!(label.contains("Output picture"), "{label}");
    assert!(label.contains("within 3 %"), "{label}");
    assert!(label.contains("DaVinci WG/Intermediate"), "{label}");
    assert!(label.contains("Output Conversion"), "{label}");
    b.configure(Some("5"));
    assert!(b.display.label().contains("within 5 %"), "{}", b.display.label());
}

#[test]
fn nothing_is_drawn_before_a_frame_arrives() {
    let b = Bench::new("empty", None);
    assert_eq!(pixel(&b.render(64, 36), 64, 32, 18), [0, 0, 0]);
    assert_eq!(b.display.label(), "Highlight neutrals");
}
