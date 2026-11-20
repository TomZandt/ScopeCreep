//! Development tool (#82): each Panel's own GPU time per UHD frame, headless. Synthetic UHD (3840 × 2160) float
//! frames go through the real displays as the app runs them (upload, Output Conversion, each display's
//! `on_frame` and `on_timeline_frame`, each display's `draw`), and wgpu timestamp queries time each part on the
//! GPU. Every part runs in its own submission and the CPU waits for it before the next, so no two parts overlap on
//! the GPU and each time is that part's alone.
//!
//! Draws are timed into a Window-sized target at three sizes: the default 1280 × 760 Window at 1x (the default
//! layout's cells), a 3840 × 2160 Window at 2x (the same layout on a 4K monitor) and one Panel filling that 4K
//! Window (Ctrl+F). Displays not in the Window are drawn into a cell of the default layout of their kind (a
//! picture Panel into ARRI False Colour's, a Scope into the Luma waveform's).
//!
//! Without timestamp queries inside encoders, each part is timed on the CPU from submit to done instead (it says
//! so); that includes the submit and wake-up overhead.
//!
//!   cargo run --release -p scopecreep --example panel_gpu_time -- [--frames 300] [--warmup 30]
//!       [--picture graded|card|flat|all] [--app-ms 10]
//!
//! The app's part of the 20 ms UHD 50 frame on the GPU is taken as half (`--app-ms`), the rest left to Resolve's
//! own render. Each Panel's share is what is left of that after the shared work (upload and Output Conversion),
//! split evenly over the Window's Panels, and a Panel is over it when its median measure plus draw at the default
//! size is. The 99th percentile is printed too, but the GPU is shared with the desktop and Resolve, whose work can
//! land between a part's two timestamps, so the tail is not the Panel's alone.
use scopecreep::colour_setup::{ColourSetup, KnownColourSetup, Provenance};
use scopecreep::components::{BarKind, MenuBar};
use scopecreep::display::{Display, DisplayContext, DisplaySet};
use scopecreep::gpu_frame::{GpuFrame, new_texture_id};
use scopecreep::layout::MARGIN;
use scopecreep::layout_tree::LayoutTree;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::panel_grid::{PanelLayout, cells_filled};
use scopecreep::settings::PanelSettings;
use scopecreep::style;
use scopecreep::waveform::Waveform;
use scopecreep_feed::SentColourSetup;
use std::sync::Arc;
use std::time::Instant;

const WIDTH: u32 = 3840;
const HEIGHT: u32 = 2160;
/// What Resolve plays: a colour-managed DaVinci Wide Gamut timeline shown in Rec.709, so Output Conversion runs and
/// Stops False Colour decodes scene light.
const SETUP: &str =
    "science=rcm\ntimeline=DaVinci Wide Gamut / DaVinci Intermediate\noutput=Rec.709 Gamma 2.4\n";
/// The format the Window's surface usually has on Linux (not sRGB).
const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// The UHD 50 frame (ADR 0003).
const FRAME_MS: f64 = 20.0;
/// Timed parts per frame at most (two timestamps each).
const SLOTS: u32 = 96;

/// One Window size the draws are timed at.
struct Size {
    name: &'static str,
    surface: [f32; 2],
    scale: f32,
    /// One Panel fills the Window (Ctrl+F).
    fill: bool,
}

const SIZES: [Size; 3] = [
    Size {
        name: "1280x760 1x",
        surface: [1280.0, 760.0],
        scale: 1.0,
        fill: false,
    },
    Size {
        name: "3840x2160 2x",
        surface: [3840.0, 2160.0],
        scale: 2.0,
        fill: false,
    },
    Size {
        name: "fills 4K",
        surface: [3840.0, 2160.0],
        scale: 2.0,
        fill: true,
    },
];

struct Entry {
    label: String,
    in_window: bool,
    /// The same display as one above with a setting changed: not one of the Window's Panels by itself.
    variant: bool,
    display: Arc<dyn Display>,
    /// The Panel of the default layout whose cell it draws into.
    cell_of: usize,
}

/// Times parts of the GPU's work: with timestamp queries when the adapter has them inside encoders, else on the
/// CPU from submit to done.
struct Timer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    queries: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer)>,
    period_ns: f64,
    next: u32,
    cpu: Vec<f64>,
}

impl Timer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, timestamps: bool) -> Timer {
        let queries = timestamps.then(|| {
            let set = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("panel gpu time"),
                ty: wgpu::QueryType::Timestamp,
                count: SLOTS * 2,
            });
            let size = SLOTS as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            let resolve = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamps"),
                size,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamps read"),
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            (set, resolve, read)
        });
        Timer {
            device: device.clone(),
            queue: queue.clone(),
            queries,
            period_ns: queue.get_timestamp_period() as f64,
            next: 0,
            cpu: Vec::new(),
        }
    }

    fn wait(&self) {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }

    /// Time the compute `work` records; returns its slot.
    fn compute(&mut self, work: impl FnOnce(&mut wgpu::CommandEncoder)) -> usize {
        let slot = self.next;
        self.next += 1;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        if let Some((set, _, _)) = &self.queries {
            encoder.write_timestamp(set, slot * 2);
        }
        work(&mut encoder);
        if let Some((set, _, _)) = &self.queries {
            encoder.write_timestamp(set, slot * 2 + 1);
        }
        self.finish(encoder, slot);
        slot as usize
    }

    /// Time one render pass into `target` in which `draw` draws; returns its slot.
    fn draw(
        &mut self,
        target: &wgpu::TextureView,
        draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) -> usize {
        let slot = self.next;
        self.next += 1;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("panel"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: self.queries.as_ref().map(|(set, _, _)| {
                    wgpu::RenderPassTimestampWrites {
                        query_set: set,
                        beginning_of_pass_write_index: Some(slot * 2),
                        end_of_pass_write_index: Some(slot * 2 + 1),
                    }
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            draw(&mut pass);
        }
        self.finish(encoder, slot);
        slot as usize
    }

    fn finish(&mut self, mut encoder: wgpu::CommandEncoder, slot: u32) {
        let start = Instant::now();
        if let Some((set, resolve, _)) = &self.queries {
            encoder.resolve_query_set(
                set,
                slot * 2..slot * 2 + 2,
                resolve,
                slot as u64 * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT,
            );
        }
        self.queue.submit([encoder.finish()]);
        self.wait();
        self.cpu.push(start.elapsed().as_secs_f64() * 1e3);
    }

    /// Every part's time in milliseconds, by slot, and start over.
    fn read(&mut self) -> Vec<f64> {
        let slots = self.next as usize;
        self.next = 0;
        let cpu = std::mem::take(&mut self.cpu);
        let Some((_, resolve, read)) = &self.queries else {
            return cpu;
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(resolve, 0, read, 0, resolve.size());
        self.queue.submit([encoder.finish()]);
        read.map_async(wgpu::MapMode::Read, .., |_| {});
        self.wait();
        let times = {
            let mapped = read.get_mapped_range(..).unwrap();
            (0..slots)
                .map(|s| {
                    let at = s * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT as usize;
                    let t: &[u64] = bytemuck::cast_slice(&mapped[at..at + 16]);
                    t[1].saturating_sub(t[0]) as f64 * self.period_ns / 1e6
                })
                .collect()
        };
        read.unmap();
        times
    }
}

/// A picture with the spread of real footage: smooth gradients in the DaVinci Intermediate range with fine noise,
/// a warm patch of skin-like values, and some highlights over 1. Bottom row first, as the Sender writes it.
fn graded(width: u32, height: u32) -> Vec<f32> {
    let mut out = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let (u, v) = (x as f32 / width as f32, y as f32 / height as f32);
            let mut h = (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) as f32;
            h = (h * 0.000_123_4).fract() * 0.02 - 0.01;
            let base = 0.15 + 0.45 * v + 0.1 * (u * 9.0).sin() * (v * 5.0).cos();
            let mut rgb = [
                base + 0.12 * (u * 3.1).sin() + h,
                base + 0.08 * (v * 4.3).cos() + h,
                base + 0.15 * (u * 2.3 + v).cos() + h,
            ];
            let skin = ((u - 0.4) * (u - 0.4) + (v - 0.5) * (v - 0.5)) < 0.02;
            if skin {
                rgb = [base + 0.08 + h, base + 0.02 + h, base - 0.03 + h];
            }
            if u > 0.9 && v > 0.85 {
                rgb = rgb.map(|c| c + 0.5);
            }
            out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 1.0]);
        }
    }
    out
}

fn picture(kind: &str) -> Vec<f32> {
    match kind {
        "graded" => graded(WIDTH, HEIGHT),
        "card" => scopecreep_feed::synthetic::test_card(WIDTH, HEIGHT, 0),
        // Every pixel the same: the most contention any counting display's atomics can meet (a black frame between
        // shots, a flat slate).
        "flat" => scopecreep_feed::synthetic::solid(WIDTH, HEIGHT, [0.2, 0.2, 0.2, 1.0]),
        other => panic!("unknown picture {other} (graded, card, flat)"),
    }
}

/// Median, 99th percentile and maximum.
fn stats(samples: &[f64]) -> [f64; 3] {
    let mut s = samples.to_vec();
    s.sort_by(f64::total_cmp);
    let at = |q: f64| s[((s.len() - 1) as f64 * q).round() as usize];
    [at(0.5), at(0.99), s[s.len() - 1]]
}

/// Each Panel's content rectangle in the default layout's Window of `size`, as the app lays it out, or the one
/// Panel filling it.
fn contents(grid: &LayoutTree, size: &Size, panels: usize) -> Vec<[f32; 4]> {
    let [width, height] = size.surface;
    let scale = size.scale;
    let margin = MARGIN * scale;
    let status = style::SPACE_STATUS_ROW * scale;
    let top = MenuBar::height(scale) + BarKind::Top.height(scale) + margin;
    let area = [margin, top, width - 2.0 * margin, height - status - top];
    (0..panels)
        .map(|i| {
            let cell = cells_filled(grid, area, margin, size.fill.then_some(i))[i];
            PanelLayout::new(cell, scale).content
        })
        .collect()
}

fn main() {
    let (mut frames, mut warmup, mut pictures, mut app_ms) =
        (300usize, 30usize, vec!["graded", "card", "flat"], 10.0f64);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().expect("a value after each option");
        match arg.as_str() {
            "--frames" => frames = value().parse().expect("frames"),
            "--warmup" => warmup = value().parse().expect("warm-up frames"),
            "--app-ms" => app_ms = value().parse().expect("milliseconds"),
            "--picture" => {
                let p = value();
                pictures = match p.as_str() {
                    "all" => vec!["graded", "card", "flat"],
                    "graded" => vec!["graded"],
                    "card" => vec!["card"],
                    "flat" => vec!["flat"],
                    other => panic!("unknown picture {other}"),
                }
            }
            other => panic!("unknown argument {other}"),
        }
    }

    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a GPU");
    let info = adapter.get_info();
    let features = adapter.features();
    let filterable = features.contains(wgpu::Features::FLOAT32_FILTERABLE);
    let timestamps = features.contains(
        wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
    );
    println!(
        "GPU: {} ({:?}, driver {} {})",
        info.name, info.backend, info.driver, info.driver_info
    );
    println!(
        "TIMESTAMP_QUERY {}, TIMESTAMP_QUERY_INSIDE_ENCODERS {}, TIMESTAMP_QUERY_INSIDE_PASSES {}, FLOAT32_FILTERABLE {}",
        features.contains(wgpu::Features::TIMESTAMP_QUERY),
        features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS),
        features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES),
        filterable
    );
    println!(
        "timing: {}",
        if timestamps {
            "GPU timestamp queries, each part in its own submission"
        } else {
            "NO timestamp queries inside encoders: CPU time from submit to done, each part alone (includes overhead)"
        }
    );
    let mut required = wgpu::Features::empty();
    if filterable {
        required |= wgpu::Features::FLOAT32_FILTERABLE;
    }
    if timestamps {
        required |=
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    }
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("panel gpu time"),
        required_features: required,
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        ..Default::default()
    }))
    .expect("a GPU device");

    // Every display: the Window's first in Panel order, then a variant and the hidden ones, each with the Panel of
    // the default layout whose cell it draws into.
    let (d, q) = (&device, &queue);
    let luma = Waveform::new(d, q, TARGET);
    const ARRI: &str = "ARRI False Colour";
    const LUMA: &str = scopecreep::waveform::NAME;
    type Listed<'a> = (Option<&'a str>, bool, &'a str, Arc<dyn Display>);
    let window = |display: Arc<dyn Display>| -> Listed { (None, true, display.name(), display) };
    let hidden =
        |display: Arc<dyn Display>, like: &'static str| -> Listed { (None, false, like, display) };
    let list: Vec<Listed> = vec![
        window(Arc::new(scopecreep::picture_view::PictureView::new(
            d, q, TARGET, filterable,
        ))),
        window(Arc::new(luma.clone())),
        window(Arc::new(
            scopecreep::arri_false_colour::ArriFalseColour::new(d, q, TARGET),
        )),
        window(Arc::new(scopecreep::vectorscope::Vectorscope::new(
            d, q, TARGET,
        ))),
        window(Arc::new(
            scopecreep::saturation_heatmap::SaturationHeatmap::new(d, q, TARGET),
        )),
        window(Arc::new(scopecreep::skin_tone_view::SkinTone::new(
            d, q, TARGET, filterable,
        ))),
        window(Arc::new(
            scopecreep::stops_false_colour::StopsFalseColour::new(d, q, TARGET),
        )),
        (
            Some("Luma waveform, Colorise on"),
            true,
            LUMA,
            Arc::new(Waveform::new(d, q, TARGET)),
        ),
        hidden(
            Arc::new(scopecreep::gamut_warning::GamutWarning::new(d, q, TARGET)),
            ARRI,
        ),
        hidden(
            Arc::new(scopecreep::highlight_neutrals::HighlightNeutrals::new(
                d, q, TARGET, filterable,
            )),
            ARRI,
        ),
        hidden(Arc::new(scopecreep::cie::CiePlot::new(d, q, TARGET)), LUMA),
        hidden(
            Arc::new(scopecreep::black_and_white::BlackAndWhite::new(
                d, q, TARGET, filterable,
            )),
            ARRI,
        ),
        hidden(Arc::new(Waveform::new_colour(d, q, TARGET, &luma)), LUMA),
        hidden(
            Arc::new(scopecreep::channel_view::ChannelView::new(
                d, q, TARGET, filterable,
            )),
            ARRI,
        ),
    ];
    // The Window's seven, as the app has them (its display order).
    let window_names: Vec<&str> = list
        .iter()
        .filter(|(variant, in_window, _, _)| *in_window && variant.is_none())
        .map(|(_, _, _, display)| display.name())
        .collect();
    let grid = LayoutTree::default_for(&window_names);
    let entries: Vec<Entry> = list
        .into_iter()
        .map(|(variant, in_window, like, display)| Entry {
            label: variant.unwrap_or(display.name()).to_string(),
            in_window,
            variant: variant.is_some(),
            cell_of: window_names
                .iter()
                .position(|n| *n == like)
                .expect("a Panel of the Window"),
            display,
        })
        .collect();

    // The Colour Setup and default settings, as the app gives them; the second luma waveform with Colorise on.
    let known = KnownColourSetup {
        setup: ColourSetup::from_feed(&SentColourSetup::new(Provenance::Manual, SETUP))
            .expect("a Colour Setup"),
        provenance: Provenance::Manual,
    };
    let context = DisplayContext {
        colour_label: known.label(),
        colour_setup: Some(known.clone()),
        ..Default::default()
    };
    println!("Colour Setup: {}", known.label());
    let mut set = DisplaySet::default();
    let mut colorised_set = DisplaySet::default();
    for e in &entries {
        if e.variant {
            colorised_set.register(e.display.clone());
        } else {
            set.register(e.display.clone());
        }
    }
    set.configure(&context, &PanelSettings::default());
    let mut colorise = PanelSettings::default();
    colorise.set(
        scopecreep::waveform::NAME,
        scopecreep::waveform::COLORISE_SETTING,
        "on",
    );
    colorised_set.configure(&context, &colorise);
    let mut conversion = OutputConversion::new(d, q);
    conversion.follow(Some(&known.setup), None, |_, _| {});

    // The Feed frame on the GPU, and the staging buffer the app copies it from.
    let frame = GpuFrame {
        texture: device.create_texture(&wgpu::TextureDescriptor {
            label: Some("feed frame"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        }),
        width: WIDTH,
        height: HEIGHT,
        bottom_up: true,
        texture_id: new_texture_id(),
    };
    let row = WIDTH * 16;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("feed frame staging"),
        size: row as u64 * HEIGHT as u64,
        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: true,
    });
    staging.unmap();
    let targets: Vec<wgpu::TextureView> = SIZES
        .iter()
        .map(|s| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("window"),
                    size: wgpu::Extent3d {
                        width: s.surface[0] as u32,
                        height: s.surface[1] as u32,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TARGET,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        })
        .collect();
    let layouts: Vec<Vec<[f32; 4]>> = SIZES
        .iter()
        .map(|s| contents(&grid, s, window_names.len()))
        .collect();
    let mut timer = Timer::new(d, q, timestamps);

    for kind in &pictures {
        // The picture into the staging buffer (mapped for writing once more).
        staging.map_async(wgpu::MapMode::Write, .., |_| {});
        timer.wait();
        {
            let pixels = picture(kind);
            let mut mapped = staging.get_mapped_range_mut(..).unwrap();
            mapped.copy_from_slice(bytemuck::cast_slice(&pixels));
        }
        staging.unmap();

        // Per frame: [upload, conversion, the Window all at once (compute, draw at 1x)], then per display
        // [compute, draw at each size].
        let mut samples: Vec<Vec<f64>> = Vec::new();
        for n in 0..warmup + frames {
            let mut slots = Vec::new();
            slots.push(timer.compute(|encoder| {
                encoder.copy_buffer_to_texture(
                    wgpu::TexelCopyBufferInfo {
                        buffer: &staging,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(row),
                            rows_per_image: Some(HEIGHT),
                        },
                    },
                    frame.texture.as_image_copy(),
                    wgpu::Extent3d {
                        width: WIDTH,
                        height: HEIGHT,
                        depth_or_array_layers: 1,
                    },
                )
            }));
            let mut shown = None;
            slots.push(timer.compute(|encoder| {
                shown = conversion.convert(encoder, &frame).cloned();
            }));
            let shown = shown.expect("the GPU has room for the converted frame");
            // The Window as the app runs it: one submission measuring every Panel's display, one pass drawing them.
            slots.push(timer.compute(|encoder| {
                for e in entries.iter().filter(|e| e.in_window && !e.variant) {
                    e.display.on_frame(encoder, &shown);
                    e.display.on_timeline_frame(encoder, &frame);
                }
            }));
            slots.push(timer.draw(&targets[0], |pass| {
                for e in entries.iter().filter(|e| e.in_window && !e.variant) {
                    e.display.draw(
                        pass,
                        layouts[0][e.cell_of],
                        SIZES[0].surface,
                        SIZES[0].scale,
                    );
                }
            }));
            for e in &entries {
                slots.push(timer.compute(|encoder| {
                    e.display.on_frame(encoder, &shown);
                    e.display.on_timeline_frame(encoder, &frame);
                }));
                for (s, size) in SIZES.iter().enumerate() {
                    slots.push(timer.draw(&targets[s], |pass| {
                        e.display
                            .draw(pass, layouts[s][e.cell_of], size.surface, size.scale)
                    }));
                }
            }
            let times = timer.read();
            if n >= warmup {
                if samples.is_empty() {
                    samples = vec![Vec::new(); slots.len()];
                }
                for (i, slot) in slots.iter().enumerate() {
                    samples[i].push(times[*slot]);
                }
            }
        }

        let ms = |s: &[f64]| {
            let [median, p99, _] = stats(s);
            format!("{median:.3} / {p99:.3}")
        };
        let sum = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x + y).collect::<Vec<_>>();
        let in_window = entries.iter().filter(|e| e.in_window && !e.variant).count();
        // Each Panel's share: what is left of the app's part of the frame after the shared work, split evenly.
        let shared = stats(&samples[0])[0] + stats(&samples[1])[0];
        let share = (app_ms - shared) / in_window as f64;
        println!();
        println!(
            "## Picture: {kind}, {frames} frames (after {warmup} warm-up), UHD {WIDTH}x{HEIGHT}; median / 99th percentile, ms"
        );
        println!();
        println!("| Shared work | GPU time |");
        println!("|---|---|");
        println!(
            "| Upload (staging buffer to texture) | {} |",
            ms(&samples[0])
        );
        println!("| Output Conversion | {} |", ms(&samples[1]));
        println!(
            "| The Window's 7 displays measuring, one submission | {} |",
            ms(&samples[2])
        );
        println!(
            "| The Window's 7 displays drawing at 1280x760 1x, one pass | {} |",
            ms(&samples[3])
        );
        let whole: Vec<f64> = (0..frames)
            .map(|i| samples[0][i] + samples[1][i] + samples[2][i] + samples[3][i])
            .collect();
        let [wm, wp, wx] = stats(&whole);
        println!(
            "| **Whole Window per frame** (sum of the four above) | **{wm:.3} / {wp:.3}** (max {wx:.3}; {:.1} % of the {FRAME_MS} ms frame at the median) |",
            wm / FRAME_MS * 100.0
        );
        println!();
        println!(
            "Each Panel's share: ({app_ms} ms for the app - {shared:.2} ms shared work) / {in_window} Panels = {share:.2} ms, against its median."
        );
        println!();
        println!(
            "| Panel | In Window | Measure | Draw {} | Draw {} | Draw {} | Measure + draw 1x | Over its share? |",
            SIZES[0].name, SIZES[1].name, SIZES[2].name
        );
        println!("|---|---|---|---|---|---|---|---|");
        for (i, e) in entries.iter().enumerate() {
            let base = 4 + i * (1 + SIZES.len());
            let compute = &samples[base];
            let total = sum(compute, &samples[base + 1]);
            let verdict = if stats(&total)[0] > share {
                "**over**"
            } else {
                "within"
            };
            println!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                e.label,
                if e.in_window { "yes" } else { "no" },
                ms(compute),
                ms(&samples[base + 1]),
                ms(&samples[base + 2]),
                ms(&samples[base + 3]),
                ms(&total),
                verdict
            );
        }
    }
}
