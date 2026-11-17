//! The vectorscope Panels (#61, spec #49 stories 26 to 32), driven at the Feed seam: synthetic frames go in through
//! the synthetic writer, the app's Feed monitor hands them to the GPU exactly as the app does, and the tests look
//! at what each vectorscope measured, what it drew and its label. Expected positions come from the standard
//! BT.709 Cb/Cr constants (1.8556, 1.5748) and the research table's hue angles, never from the shader. Needs a GPU.
use scopecreep::colour_setup::{ColourSetup, KnownColourSetup, Levels, Provenance, Science};
use scopecreep::display::{DisplayContext, DisplaySet};
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::monitor::Monitor;
use scopecreep::panel_grid::{PanelGrid, PanelLayout};
use scopecreep::settings::PanelSettings;
use scopecreep::skin_tone::SKIN_TONE_LINE_DEGREES;
use scopecreep::vectorscope::{PlotSquare, VectorscopeCounts, Zoom};
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_accuracy::transfer::bt709_oetf;
use scopecreep_feed::{FeedWriter, FrameMeta, now_ns};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

const MS: u64 = 1_000_000;
const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const V100: &str = "Vectorscope 100 %";
const V200: &str = "Vectorscope 200 %";

fn gpu() -> (MutexGuard<'static, ()>, wgpu::Device, wgpu::Queue) {
    static GPU: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (device, queue) = GPU.get_or_init(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() }))
            .expect("these tests need a GPU");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("vectorscope tests"), ..Default::default() })).unwrap()
    });
    (turn, device.clone(), queue.clone())
}

struct Bench {
    path: PathBuf,
    writer: FeedWriter,
    monitor: Monitor,
    uploader: FrameUploader,
    set: DisplaySet,
    settings: PanelSettings,
    context: DisplayContext,
    device: wgpu::Device,
    queue: wgpu::Queue,
    now: u64,
    frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-vectorscope-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let writer = FeedWriter::create(&path, 4, 1920 * 1080 * 16).unwrap();
        let b = Bench {
            monitor: Monitor::new(&path),
            uploader: FrameUploader::new(&device, &queue),
            set: DisplaySet::standard(&device, &queue, TARGET, false),
            settings: PanelSettings::default(),
            context: DisplayContext::default(),
            path,
            writer,
            device,
            queue,
            now: now_ns(),
            frame: 86400.0,
            _gpu_turn: turn,
        };
        b.reconfigure();
        b
    }

    fn reconfigure(&self) {
        self.set.configure(&self.context, &self.settings);
    }

    fn index(&self, name: &str) -> usize {
        self.set.names().iter().position(|n| *n == name).unwrap_or_else(|| panic!("no display called {name}"))
    }

    fn choose(&mut self, panel: &str, key: &str, value: &str) {
        self.settings.set(panel, key, value);
        self.reconfigure();
    }

    fn label(&self, name: &str) -> String {
        self.set.labels()[self.index(name)].clone()
    }

    fn counts(&self, name: &str) -> VectorscopeCounts {
        *self.set.read_back(self.index(name)).and_then(|r| r.downcast::<VectorscopeCounts>().ok()).expect("a vectorscope reads back")
    }

    /// Resolve plays one frame (RGBA, bottom row first) and the app has a moment to take it.
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

    /// Draw every display into its Panel of a `width` x `height` target and read it back (RGBA8, rows top first).
    fn render(&self, width: u32, height: u32, scale: f32) -> (Vec<u8>, Vec<PanelLayout>) {
        let cells = PanelGrid::new(self.set.len()).cells([0.0, 0.0, width as f32, height as f32], 8.0 * scale);
        let panels: Vec<PanelLayout> = cells.iter().map(|c| PanelLayout::new(*c, scale, 1)).collect();
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
                self.set.draw(&mut pass, i, panel.content, [width as f32, height as f32], scale);
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
        (image, panels)
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn solid(width: u32, height: u32, rgb: [f32; 3]) -> Vec<f32> {
    (0..width * height).flat_map(|_| [rgb[0], rgb[1], rgb[2], 1.0]).collect()
}

/// BT.709 Cb and Cr from the standard's own constants.
fn cb_cr(rgb: [f64; 3]) -> [f64; 2] {
    let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    [(rgb[2] - y) / 1.8556, (rgb[0] - y) / 1.5748]
}

/// Colour bars: red, yellow, green, cyan, magenta, blue at `level`, as ten-pixel stripes.
fn bars(level: f32) -> (u32, u32, Vec<f32>, [[f64; 3]; 6]) {
    let colours = [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]].map(|c: [f64; 3]| c.map(|v| v * level as f64));
    let (w, h) = (60u32, 36u32);
    let pixels = (0..w * h).flat_map(|i| colours[(i % w / 10) as usize].map(|v| v as f32).into_iter().chain([1.0])).collect();
    (w, h, pixels, colours)
}

/// Where `render` put the pixel under `at`, as three bytes.
fn pixel(image: &[u8], width: u32, at: [f32; 2]) -> [u8; 3] {
    let i = (at[1] as usize * width as usize + at[0] as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// The brightest red value in the 3 x 3 pixels around `at`.
fn brightest_near(image: &[u8], width: u32, at: [f32; 2]) -> u8 {
    let mut best = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            best = best.max(pixel(image, width, [at[0] + dx as f32, at[1] + dy as f32])[0]);
        }
    }
    best
}

fn angle_gap(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn there_are_two_vectorscope_panels_the_100_percent_one_first() {
    let b = Bench::new("order");
    assert_eq!(b.index(V200), b.index(V100) + 1);
}

#[test]
fn rec709_bars_at_75_and_100_percent_land_on_their_targets() {
    let mut b = Bench::new("bars");
    for level in [0.75f32, 1.0] {
        let (w, h, pixels, colours) = bars(level);
        b.play(w, h, &pixels);
        let counts = b.counts(V100);
        for colour in &colours {
            let [cb, cr] = cb_cr(*colour);
            assert_eq!(counts.near(cb, cr), 360, "{}% bars: the stripe {colour:?} lands at Cb {cb:.4}, Cr {cr:.4}", level * 100.0);
        }
        assert_eq!(counts.total(), 6 * 360, "every pixel is counted once");
    }
}

#[test]
fn the_100_percent_vectorscope_shows_the_whole_picture_and_the_200_percent_only_what_is_within_its_radius() {
    let mut b = Bench::new("range");
    let (w, h, pixels, _) = bars(1.0);
    b.play(w, h, &pixels);
    assert_eq!(b.counts(V100).total(), 6 * 360, "at 100 % every full-strength bar is on the plot");
    // At 200 % a full-strength bar is further out than the plot reaches: those pixels are off it, not wrapped.
    assert!(b.counts(V200).total() < 6 * 360);
}

#[test]
fn each_colorchecker_patch_lands_at_the_research_tables_hue_angle_within_one_degree() {
    let mut b = Bench::new("hue");
    // Rec.709 BT.709-encoded patches, as the research table's hue column defines them, shown as sent.
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    for patch in &chart.patches()[..18] {
        let rgb = patch.rec709_linear.map(|l| (l.signum() * bt709_oetf(l.abs())) as f32);
        b.play(64, 36, &solid(64, 36, rgb));
        let counts = b.counts(V100);
        assert_eq!(counts.total(), 64 * 36, "{} is on the plot", patch.name);
        let (x, y) = counts.peak_bin();
        assert_eq!(counts.at(x, y), 64 * 36, "{} is one colour", patch.name);
        let [cb, cr] = counts.centre_of(x, y);
        let hue = cr.atan2(cb).to_degrees().rem_euclid(360.0);
        assert!(angle_gap(hue, patch.hue_degrees) <= 1.0, "{} lands at {hue:.2} degrees, the table says {:.2}", patch.name, patch.hue_degrees);
    }
}

#[test]
fn the_200_percent_panel_shows_the_same_data_at_twice_the_radius() {
    let mut b = Bench::new("twice");
    b.play(64, 36, &solid(64, 36, [0.5, 0.25, 0.375]));
    let from_middle = |counts: &VectorscopeCounts| {
        let (x, y) = counts.peak_bin();
        assert_eq!(counts.at(x, y), 64 * 36);
        [x as f64 + 0.5 - 512.0, y as f64 + 0.5 - 512.0]
    };
    let (one, two) = (from_middle(&b.counts(V100)), from_middle(&b.counts(V200)));
    for axis in 0..2 {
        assert!((two[axis] - 2.0 * one[axis]).abs() <= 1.0, "axis {axis}: {} bins from the middle at 100 %, {} at 200 %", one[axis], two[axis]);
    }
    assert!(one[0].abs() > 10.0 && one[1].abs() > 10.0, "a colour well off the middle: {one:?}");
}

fn black_frame_and_render(b: &mut Bench) -> (Vec<u8>, PlotSquare, PanelLayout) {
    b.play(64, 36, &solid(64, 36, [0.0; 3]));
    let (image, panels) = b.render(2000, 900, 1.0);
    let panel = panels[b.index(V100)];
    (image, PlotSquare::new(panel.content, 1.0), panel)
}

#[test]
fn the_skin_tone_line_is_drawn_at_the_recorded_angle_and_can_be_turned_off() {
    assert_eq!(SKIN_TONE_LINE_DEGREES, 123.0, "the angle measured in Resolve 21.2 (#60), held in one shared constant");
    let mut b = Bench::new("skin");
    let (on, square, _) = black_frame_and_render(&mut b);
    let width = 2000;
    let [mx, my] = square.centre();
    let along = |degrees: f64, fraction: f32| {
        let r = square.ring_radius() * fraction;
        [mx + r * degrees.to_radians().cos() as f32, my - r * degrees.to_radians().sin() as f32]
    };
    let lit = |image: &[u8], degrees: f64| brightest_near(image, width, along(degrees, 0.7));
    let background = 9; // 0.035 of 255
    assert!(lit(&on, SKIN_TONE_LINE_DEGREES) > 60, "the skin-tone line is drawn: {}", lit(&on, SKIN_TONE_LINE_DEGREES));
    assert!(lit(&on, SKIN_TONE_LINE_DEGREES + 25.0) <= 40, "and only at its angle: {}", lit(&on, SKIN_TONE_LINE_DEGREES + 25.0));

    let panel = b.set.name(b.index(V100));
    b.choose(panel, "skin-line", "off");
    let (off, _, _) = black_frame_and_render(&mut b);
    assert!(lit(&off, SKIN_TONE_LINE_DEGREES) <= 40, "turned off, nothing is drawn there: {}", lit(&off, SKIN_TONE_LINE_DEGREES));
    assert!(lit(&off, SKIN_TONE_LINE_DEGREES) >= background);
}

#[test]
fn the_colour_targets_are_drawn_where_the_chosen_bars_land() {
    let mut b = Bench::new("targets");
    let width = 2000;
    let (image, square, _) = black_frame_and_render(&mut b);
    // The top edge of the red target box (red at 75 %: Cb -0.0859, Cr 0.375; the box reaches 0.02 in Cb/Cr).
    let on_box = |image: &[u8], level: f64| {
        let [cb, cr] = cb_cr([level, 0.0, 0.0]);
        let [x, y] = square.position(cb, cr + 0.02, Zoom::X1);
        brightest_near(image, width, [x, y])
    };
    assert!(on_box(&image, 0.75) > 60, "a target box at the 75 % red position: {}", on_box(&image, 0.75));
    assert!(on_box(&image, 1.0) <= 40, "not at the 100 % red position: {}", on_box(&image, 1.0));

    let panel = b.set.name(b.index(V100));
    b.choose(panel, "targets", "100");
    let (image, _, _) = black_frame_and_render(&mut b);
    assert!(on_box(&image, 1.0) > 60, "100 % targets: a box at the 100 % red position: {}", on_box(&image, 1.0));
    assert!(on_box(&image, 0.75) <= 40, "and no longer at the 75 % position: {}", on_box(&image, 0.75));
}

#[test]
fn the_trace_is_tinted_by_the_colour_that_put_it_there_and_the_middle_is_white() {
    let mut b = Bench::new("tint");
    let width = 2000;
    let cases: [([f32; 3], usize); 4] = [([0.75, 0.0, 0.0], 0), ([0.0, 0.75, 0.0], 1), ([0.0, 0.0, 0.75], 2), ([0.75, 0.0, 0.75], 3)];
    for (rgb, which) in cases {
        b.play(64, 36, &solid(64, 36, rgb));
        let (image, panels) = b.render(width, 900, 1.0);
        let square = PlotSquare::new(panels[b.index(V100)].content, 1.0);
        let [cb, cr] = cb_cr([rgb[0] as f64, rgb[1] as f64, rgb[2] as f64]);
        let [x, y] = square.position(cb, cr, Zoom::X1);
        let p = pixel(&image, width, [x, y]);
        let (hi, lo) = (p.iter().copied().max().unwrap(), p.iter().copied().min().unwrap());
        assert!(hi > 120, "the {rgb:?} trace is lit: {p:?}");
        if which < 3 {
            assert_eq!(p[which], hi, "the {rgb:?} trace is tinted towards its own colour: {p:?}");
        } else {
            assert!(p[0] > p[1] && p[2] > p[1], "magenta is red and blue, not green: {p:?}");
        }
        assert!(hi as i32 - lo as i32 > 60, "and clearly tinted, not grey: {p:?}");
    }
    // Grey pixels land in the middle: white, not tinted.
    b.play(64, 36, &solid(64, 36, [0.5, 0.5, 0.5]));
    let (image, panels) = b.render(width, 900, 1.0);
    let square = PlotSquare::new(panels[b.index(V100)].content, 1.0);
    let [x, y] = square.position(0.0, 0.0, Zoom::X1);
    let mut lit = [0u8; 3];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let p = pixel(&image, width, [x + dx as f32, y + dy as f32]);
            if p[0] as u32 + p[1] as u32 + p[2] as u32 > lit.iter().map(|v| *v as u32).sum::<u32>() { lit = p; }
        }
    }
    assert!(lit[0] > 150 && (lit[0] as i32 - lit[2] as i32).abs() < 40, "the middle is white: {lit:?}");
}

#[test]
fn the_colour_target_boxes_carry_the_colour_they_stand_for() {
    let mut b = Bench::new("target colours");
    let width = 2000;
    let (image, square, _) = black_frame_and_render(&mut b);
    // The top edge of each box, in the order red, yellow, green, cyan, magenta, blue at 75 %.
    let colours: [[f64; 3]; 6] = [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]];
    for bar in colours {
        let [cb, cr] = cb_cr(bar.map(|v| v * 0.75));
        let [x, y] = square.position(cb, cr + 0.02, Zoom::X1);
        let mut best = [0u8; 3];
        for dy in -1..=1 {
            for dx in -1..=1 {
                let p = pixel(&image, width, [x + dx as f32, y + dy as f32]);
                if p.iter().map(|v| *v as u32).sum::<u32>() > best.iter().map(|v| *v as u32).sum::<u32>() { best = p; }
            }
        }
        for c in 0..3 {
            if bar[c] > 0.5 { assert!(best[c] > 100, "the {bar:?} box has {c} lit: {best:?}"); } else { assert!(best[c] < 40, "the {bar:?} box has {c} dark: {best:?}"); }
        }
    }
}

#[test]
fn the_trace_brightness_setting_makes_a_sparse_colour_brighter_or_dimmer() {
    let mut b = Bench::new("brightness");
    let width = 2000;
    // One red pixel in an otherwise black frame.
    let mut pixels = solid(64, 36, [0.0; 3]);
    pixels[0] = 0.75;
    let [cb, cr] = cb_cr([0.75, 0.0, 0.0]);
    let mut seen = Vec::new();
    for choice in ["dim", "normal", "bright"] {
        let panel = b.set.name(b.index(V100));
        b.choose(panel, "brightness", choice);
        b.play(64, 36, &pixels);
        let (image, panels) = b.render(width, 900, 1.0);
        let square = PlotSquare::new(panels[b.index(V100)].content, 1.0);
        seen.push(brightest_near(&image, width, square.position(cb, cr, Zoom::X1)));
    }
    assert!(seen[0] < seen[1] && seen[1] <= seen[2], "dimmer, normal, brighter: {seen:?}");
}

#[test]
fn the_label_names_the_measured_space_the_weights_the_targets_and_the_assumed_angle() {
    let mut b = Bench::new("label");
    let assumed = b.label(V100);
    assert!(assumed.starts_with("Vectorscope 100 %  ·  Cb/Cr"), "{assumed}");
    assert!(assumed.contains("Rec.709 assumed, measured as sent, unconverted"), "{assumed}");
    assert!(assumed.contains("Y = 0.2126 R + 0.7152 G + 0.0722 B"), "{assumed}");
    assert!(assumed.contains("75 % targets"), "{assumed}");
    assert!(assumed.contains("123° (as Resolve 21.2 draws it)"), "{assumed}");
    assert!(!assumed.contains("Resolve"), "the angle is not claimed to match Resolve: {assumed}");
    assert!(b.label(V200).starts_with("Vectorscope 200 %"));

    let setup = ColourSetup::new(Some(Science::Yrgb), "P3-D65".into(), "P3-D65".into(), None, None, None, Some(Levels::Video));
    b.context = DisplayContext { colour_setup: Some(KnownColourSetup { setup, provenance: Provenance::Project }), colour_label: "P3-D65".into(), ..DisplayContext::default() };
    let panel = b.set.name(b.index(V100));
    b.choose(panel, "targets", "100");
    b.choose(panel, "skin-line", "off");
    let p3 = b.label(V100);
    assert!(p3.contains("P3-D65") && !p3.contains("assumed"), "{p3}");
    assert!(p3.contains("Y = 0.2290 R + 0.6917 G + 0.0793 B"), "the same weights as the Luma waveform for this space: {p3}");
    assert!(p3.contains("100 % targets"), "{p3}");
    assert!(!p3.contains("skin-tone"), "no angle is stated when the line is off: {p3}");
}

#[test]
fn the_menu_offers_trace_brightness_targets_and_the_skin_tone_line_with_sensible_defaults() {
    let b = Bench::new("menu");
    let defs = b.set.setting_defs(b.index(V100));
    let keys: Vec<_> = defs.iter().map(|d| (d.key, d.choices.iter().map(|c| c.value).collect::<Vec<_>>())).collect();
    assert_eq!(keys, [("brightness", vec!["normal", "bright", "dim"]), ("targets", vec!["75", "100"]), ("skin-line", vec!["on", "off"])]);
}
