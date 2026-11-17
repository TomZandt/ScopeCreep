//! The black-and-white Panel (#54, spec #49 stories 47 and 48), driven at the Feed seam: ColorChecker frames go in
//! through the synthetic writer, the app's Feed monitor and Output Conversion hand them to every display as the
//! app does, and the tests read what the waveform measured and what the black-and-white Panel drew, and its label.
//! A grey you see must line up with its trace: within one 8-bit step. Needs a GPU.
use scopecreep::display::DisplaySet;
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::monitor::Monitor;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::panel_grid::{PanelGrid, PanelLayout};
use scopecreep::settings::PanelSettings;
use scopecreep::waveform::WaveformCounts;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_feed::{FeedWriter, FrameMeta, Provenance, SentColourSetup, now_ns};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

const MS: u64 = 1_000_000;
const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const WAVEFORM: &str = "Luma waveform";
const BLACK_AND_WHITE: &str = "Black and white";

fn gpu() -> (MutexGuard<'static, ()>, wgpu::Device, wgpu::Queue) {
    static GPU: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (device, queue) = GPU.get_or_init(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() }))
            .expect("these tests need a GPU");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("black and white tests"), ..Default::default() })).unwrap()
    });
    (turn, device.clone(), queue.clone())
}

/// A stand-in Resolve and Sender, the app's Feed monitor, and the app's GPU side of the Feed, wired as `main.rs`
/// does: upload, Output Conversion, every display, and the Colour Setup and settings given to every display.
struct Bench {
    path: PathBuf,
    writer: FeedWriter,
    monitor: Monitor,
    uploader: FrameUploader,
    conversion: OutputConversion,
    set: DisplaySet,
    settings: PanelSettings,
    device: wgpu::Device,
    queue: wgpu::Queue,
    now: u64,
    frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Bench {
    fn new(name: &str, colour_setup: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-bw-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let mut writer = FeedWriter::create(&path, 4, 1920 * 1080 * 16).unwrap();
        writer.set_colour_setup(Some(SentColourSetup::new(Provenance::Manual, colour_setup)));
        let mut b = Bench {
            monitor: Monitor::new(&path),
            uploader: FrameUploader::new(&device, &queue),
            conversion: OutputConversion::new(&device, &queue),
            set: DisplaySet::standard(&device, &queue, TARGET, false),
            settings: PanelSettings::default(),
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

    fn index(&self, name: &str) -> usize {
        self.set.names().iter().position(|n| *n == name).unwrap_or_else(|| panic!("no display called {name}"))
    }

    fn poll(&mut self) {
        let (uploader, conversion, set) = (&mut self.uploader, &mut self.conversion, &self.set);
        self.monitor.poll(self.now, &mut |frame| {
            uploader.upload(frame, |encoder, gpu_frame| {
                let shown = conversion.convert(encoder, gpu_frame);
                set.on_frame(encoder, shown);
            })
        });
        let setup = self.monitor.status().colour_setup().map(|c| c.setup);
        self.set.configure(&self.monitor.status().display_context(), &self.settings);
        let set = &self.set;
        self.conversion.follow(setup.as_ref(), self.uploader.frame(), |encoder, shown| set.on_frame(encoder, shown));
    }

    /// The user picks a choice in a Panel's menu: the displays take it and measure the frame on screen again.
    fn choose(&mut self, panel: &str, key: &str, value: &str) {
        self.settings.set(panel, key, value);
        self.set.configure(&self.monitor.status().display_context(), &self.settings);
        let set = &self.set;
        self.conversion.again(self.uploader.frame(), |encoder, shown| set.on_frame(encoder, shown));
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

    fn counts(&self) -> WaveformCounts {
        *self.set.read_back(self.index(WAVEFORM)).and_then(|r| r.downcast::<WaveformCounts>().ok()).expect("the waveform reads back")
    }

    /// Draw every display into its Panel of a `width` × `height` target as the window does and read it back
    /// (RGBA8, rows top first), with the Panel of the black-and-white display.
    fn render(&self, width: u32, height: u32) -> (Vec<u8>, PanelLayout) {
        let cells = PanelGrid::new(self.set.len()).cells([0.0, 0.0, width as f32, height as f32], 8.0);
        let panels: Vec<PanelLayout> = cells.iter().map(|c| PanelLayout::new(*c, 1.0, 1)).collect();
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
            for (i, panel) in panels.iter().enumerate() {
                self.set.draw(&mut pass, i, panel.content, [width as f32, height as f32], 1.0);
            }
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
        let image = (0..height as usize).flat_map(|y| mapped[y * row as usize..y * row as usize + width as usize * 4].to_vec()).collect();
        (image, panels[self.index(BLACK_AND_WHITE)])
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Where picture pixel (`x`, `y`) of a `picture` sized frame is drawn when it is fitted into `area`.
fn on_screen(area: [f32; 4], picture: [u32; 2], x: f32, y: f32) -> (usize, usize) {
    let scale = (area[2] / picture[0] as f32).min(area[3] / picture[1] as f32);
    let (w, h) = (picture[0] as f32 * scale, picture[1] as f32 * scale);
    let (left, top) = ((area[0] + (area[2] - w) / 2.0).round(), (area[1] + (area[3] - h) / 2.0).round());
    ((left + x * scale) as usize, (top + y * scale) as usize)
}

fn grey_at(image: &[u8], width: u32, (x, y): (usize, usize)) -> [u8; 3] {
    let i = (y * width as usize + x) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";
const DWG_P3D65: &str = "timeline=DaVinci WG/Intermediate\noutput=P3-D65\n";
const PICTURE: [u32; 2] = [600, 400];

/// Plays the chart with only patch `number` painted (the rest black, so its columns hold nothing else), and
/// reports the code the waveform traced the patch at, and the grey the black-and-white Panel drew for it.
/// Video levels: code = 64 + 876 Luma.
fn trace_and_draw(b: &mut Bench, chart: &ColorChecker, number: usize) -> (u32, [u8; 3]) {
    let mut frame = chart.frame(PICTURE[0], PICTURE[1]);
    let black = [frame.pixels[0], frame.pixels[1], frame.pixels[2]];
    for other in (1..=24).filter(|n| *n != number) {
        let r = frame.patch_rect(other);
        for y in r.y0..r.y1 {
            let row = (PICTURE[1] - 1 - y) as usize; // rows are stored bottom first
            for x in r.x0..r.x1 {
                let i = (row * PICTURE[0] as usize + x as usize) * 4;
                frame.pixels[i..i + 3].copy_from_slice(&black);
            }
        }
    }
    b.play(PICTURE[0], PICTURE[1], &frame.pixels);
    let r = frame.patch_rect(number);
    let (cx, cy) = ((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2);
    let counts = b.counts();
    // Every pixel of the patch's middle column that is not the black around it sits on one code.
    let traced = (0..1024u32).find(|c| counts.at(cx, *c) == r.y1 - r.y0).unwrap_or_else(|| (0..1024u32).max_by_key(|c| counts.at(cx, *c)).unwrap());
    let (image, panel) = b.render(1500, 600);
    (traced, grey_at(&image, 1500, on_screen(panel.content, PICTURE, cx as f32 + 0.5, cy as f32 + 0.5)))
}

fn assert_drawn_is_traced(number: usize, name: &str, (code, drawn): (u32, [u8; 3])) {
    let want = ((code as f64 - 64.0) / 876.0 * 255.0).clamp(0.0, 255.0);
    assert!(drawn[0] == drawn[1] && drawn[1] == drawn[2], "patch {number} {name}: drawn grey, not colour: {drawn:?}");
    assert!((drawn[0] as f64 - want).abs() <= 1.0, "patch {number} {name}: drawn {} but its trace (code {code}) says {want:.2}", drawn[0]);
}

#[test]
fn every_colorchecker_patch_is_drawn_at_the_luma_the_waveform_traces_for_it() {
    let mut b = Bench::new("patches", DWG_REC709);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    for patch in chart.patches() {
        assert_drawn_is_traced(patch.number, patch.name, trace_and_draw(&mut b, &chart, patch.number));
    }
}

#[test]
fn the_waveforms_luma_weights_setting_keeps_the_two_in_agreement() {
    // Output P3-D65: from primaries and Resolve's weights differ, so the two modes draw different greys.
    let mut b = Bench::new("weights", DWG_P3D65);
    let chart = ColorChecker::new(TimelineSpace::DavinciIntermediate);
    let green = 14; // the chart's green: the weights matter most there
    let primaries = trace_and_draw(&mut b, &chart, green);
    assert_drawn_is_traced(green, "green", primaries);
    let label = b.set.labels()[b.index(BLACK_AND_WHITE)].clone();
    assert!(label.contains("from primaries"), "{label}");

    // Picking Resolve's weights on the waveform changes the black-and-white Panel too, with nothing else touched.
    b.choose(WAVEFORM, "luma", "match-resolve");
    let matched = trace_and_draw(&mut b, &chart, green);
    assert_drawn_is_traced(green, "green", matched);
    assert_ne!(primaries.1, matched.1, "the green patch is a different grey with the other weights");
    let label = b.set.labels()[b.index(BLACK_AND_WHITE)].clone();
    assert!(label.contains("to match Resolve") && label.contains("0.7152 G"), "the label names the weights: {label}");
    assert!(b.set.labels()[b.index(WAVEFORM)].contains("0.7152 G"), "the waveform names the same weights");
    for number in [1, 9, 20, 22] {
        assert_drawn_is_traced(number, "", trace_and_draw(&mut b, &chart, number));
    }
}
#[test]
fn the_panel_has_no_settings_of_its_own_and_follows_the_waveforms() {
    let b = Bench::new("menu", DWG_REC709);
    assert!(b.set.setting_defs(b.index(BLACK_AND_WHITE)).is_empty(), "no menu: the weights are the waveform's setting");
    assert!(!b.set.setting_defs(b.index(WAVEFORM)).is_empty());
}

#[test]
fn the_label_says_what_is_shown_and_which_weights_make_the_grey() {
    let mut b = Bench::new("label", DWG_REC709);
    let i = b.index(BLACK_AND_WHITE);
    assert_eq!(b.set.labels()[i], "Black and white");
    b.play(64, 36, &ColorChecker::new(TimelineSpace::DavinciIntermediate).frame(64, 36).pixels);
    let label = b.set.labels()[i].clone();
    assert!(label.starts_with("Black and white  ·  Luma  ·  Rec.709  ·  Y = 0.2126 R + 0.7152 G + 0.0722 B from primaries"), "{label}");
}
