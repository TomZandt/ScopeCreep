//! The vectorscope: a Scope that plots every pixel of the Output picture by its colour, Cb across and Cr up, so
//! hue is the angle and saturation the distance from the middle (spec #49, stories 26 to 32).
//!
//! Measured on the GPU from every pixel of every Feed frame, in 32-bit float: one compute pass over the frame
//! [`crate::gpu_frame`] already put on the GPU counts the pixels into a square grid of bins. Cb and Cr use the
//! Measured Space's luma weights (ADR 0004), so the vectorscope agrees with the Luma waveform: Cb = (B - Y) /
//! (2 (1 - Wb)) and Cr = (R - Y) / (2 (1 - Wr)), each running from -0.5 to 0.5.
//!
//! Two Panels show it (ADR 0011): at 100 % the plot's edge is [`HALF_EXTENT`] on each axis; at 200 % it is half
//! that, so the same data sits at twice the radius.
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::settings::{Choice, SettingDef};
use crate::skin_tone::SKIN_TONE_LINE_DEGREES;
use crate::waveform::Measurement;
use std::sync::{Arc, Mutex};

/// Bins across (and down) the plot.
pub const BINS: u32 = 1024;
/// Cb and Cr at the plot's edge at 100 %: a little past 0.5, so that the 100 % bars (up to about 0.51 from the
/// middle) and slightly over-saturated pictures stay inside.
pub const HALF_EXTENT: f64 = 0.55;
/// The outer ring at 100 %, in Cb/Cr. At 200 % the ring is drawn in the same place, so it stands for half this.
pub const RING: f64 = 0.5;
/// How far a colour target box reaches from the target's middle, in Cb/Cr.
pub const TARGET_HALF: f64 = 0.02;

/// How much of the colour space a Panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zoom {
    X1,
    X2,
}

impl Zoom {
    pub fn factor(self) -> f64 {
        match self {
            Zoom::X1 => 1.0,
            Zoom::X2 => 2.0,
        }
    }
    /// Cb and Cr at the plot's edge.
    pub fn half_extent(self) -> f64 {
        HALF_EXTENT / self.factor()
    }
    fn name(self) -> &'static str {
        match self {
            Zoom::X1 => "Vectorscope 100 %",
            Zoom::X2 => "Vectorscope 200 %",
        }
    }
}

/// Cb and Cr of an encoded `rgb` for luma `weights` (Wr, Wg, Wb).
pub fn chroma(weights: [f64; 3], rgb: [f64; 3]) -> [f64; 2] {
    let y = weights[0] * rgb[0] + weights[1] * rgb[1] + weights[2] * rgb[2];
    [(rgb[2] - y) / (2.0 * (1.0 - weights[2])), (rgb[0] - y) / (2.0 * (1.0 - weights[0]))]
}

/// Where the six colour bars (red, yellow, green, cyan, magenta, blue) land when they are `level` (0.75 or 1.0)
/// of full strength: their Cb/Cr for these luma weights.
pub fn bar_targets(weights: [f64; 3], level: f64) -> [[f64; 2]; 6] {
    [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]].map(|c| chroma(weights, c.map(|v| v * level)))
}

/// The settings' keys.
pub const TARGETS_SETTING: &str = "targets";
pub const SKIN_LINE_SETTING: &str = "skin-line";
pub const BRIGHTNESS_SETTING: &str = "brightness";
pub const SPREAD_SETTING: &str = "spread";

const SETTINGS: [SettingDef; 4] = [
    SettingDef {
        key: BRIGHTNESS_SETTING,
        label: "Trace brightness",
        choices: &[Choice { value: "normal", text: "Normal" }, Choice { value: "bright", text: "Brighter" }, Choice { value: "dim", text: "Dimmer" }],
    },
    SettingDef { key: SPREAD_SETTING, label: "Soft spread", choices: &[Choice { value: "on", text: "On" }, Choice { value: "off", text: "Off" }] },
    SettingDef { key: TARGETS_SETTING, label: "Colour targets", choices: &[Choice { value: "75", text: "75 % bars" }, Choice { value: "100", text: "100 % bars" }] },
    SettingDef { key: SKIN_LINE_SETTING, label: "Skin-tone line", choices: &[Choice { value: "on", text: "On" }, Choice { value: "off", text: "Off" }] },
];

/// Everything the user and the Colour Setup decide about how the vectorscope measures and draws.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Config {
    measurement: Measurement,
    /// 0.75 or 1.0.
    bars: f64,
    skin_line: bool,
    gain: f32,
    spread: bool,
}

/// Where the plot goes inside a Panel's display area, in pixels: a square, centred. Left, top, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotSquare {
    pub plot: [f32; 4],
}

/// Logical pixels: clear space around the plot.
const MARGIN: f32 = 6.0;

impl PlotSquare {
    pub fn new([left, top, width, height]: [f32; 4], scale: f32) -> PlotSquare {
        let side = (width.min(height) - 2.0 * MARGIN * scale).max(1.0).floor();
        PlotSquare { plot: [(left + (width - side) / 2.0).floor(), (top + (height - side) / 2.0).floor(), side, side] }
    }
    pub fn centre(&self) -> [f32; 2] {
        [self.plot[0] + self.plot[2] / 2.0, self.plot[1] + self.plot[3] / 2.0]
    }
    /// Pixels from the middle to the plot's edge.
    pub fn radius(&self) -> f32 {
        self.plot[2] / 2.0
    }
    /// Where the colour `cb`, `cr` is plotted at `zoom` (Cr is up).
    pub fn position(&self, cb: f64, cr: f64, zoom: Zoom) -> [f32; 2] {
        let [x, y] = self.centre();
        let k = self.radius() as f64 / zoom.half_extent();
        [x + (cb * k) as f32, y - (cr * k) as f32]
    }
    /// Pixels from the middle to the outer ring, the same at either zoom.
    pub fn ring_radius(&self) -> f32 {
        (RING / HALF_EXTENT) as f32 * self.radius()
    }
}

/// How many pixels of the picture landed in each bin: the vectorscope's contents. Bin (0, 0) is at the top left
/// of the plot.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorscopeCounts {
    half_extent: f64,
    counts: Vec<u32>,
}

impl VectorscopeCounts {
    /// Pixels in the bin `x` from the left and `y` from the top.
    pub fn at(&self, x: u32, y: u32) -> u32 {
        self.counts[(y * BINS + x) as usize]
    }
    /// Pixels in the outermost bins: colours off the chart, counted where they leave it.
    pub fn on_edge(&self) -> u64 {
        let last = BINS - 1;
        (0..BINS).flat_map(|i| [(i, 0), (i, last), (0, i), (last, i)]).map(|(x, y)| self.at(x, y) as u64).sum()
    }
    /// Every pixel counted (only a colour that is not a number, or impossibly far out, is not).
    pub fn total(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }
    /// The bin with the most pixels, as (x, y); the first if tied.
    pub fn peak_bin(&self) -> (u32, u32) {
        let (mut best, mut at) = (0, 0);
        for (i, &c) in self.counts.iter().enumerate() {
            if c > best {
                (best, at) = (c, i);
            }
        }
        (at as u32 % BINS, at as u32 / BINS)
    }
    /// The Cb and Cr at the middle of bin (`x`, `y`).
    pub fn centre_of(&self, x: u32, y: u32) -> [f64; 2] {
        let to = |i: u32| (i as f64 + 0.5) / BINS as f64 * 2.0 - 1.0;
        [to(x) * self.half_extent, -to(y) * self.half_extent]
    }
    /// The bin that holds `cb`, `cr`, if it is inside the plot.
    pub fn bin_of(&self, cb: f64, cr: f64) -> Option<(u32, u32)> {
        let (u, v) = (cb / self.half_extent * 0.5 + 0.5, 0.5 - cr / self.half_extent * 0.5);
        ((0.0..1.0).contains(&u) && (0.0..1.0).contains(&v)).then(|| ((u * BINS as f64) as u32, (v * BINS as f64) as u32))
    }
    /// Pixels in the 3 x 3 bins around the bin that holds `cb`, `cr`.
    pub fn near(&self, cb: f64, cr: f64) -> u32 {
        let Some((x, y)) = self.bin_of(cb, cr) else { return 0 };
        let mut sum = 0;
        for yy in y.saturating_sub(1)..=(y + 1).min(BINS - 1) {
            for xx in x.saturating_sub(1)..=(x + 1).min(BINS - 1) {
                sum += self.at(xx, yy);
            }
        }
        sum
    }
}

const MEASURE_SHADER: &str = r#"
struct Params {
    weights: vec4<f32>,   // luma weights for R, G, B
    size: vec4<u32>,      // picture width, height
    scale: vec4<f32>,     // 1 / half extent, Cb per (B - Y), Cr per (R - Y)
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var frame: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> bins: array<atomic<u32>>;

const BINS: f32 = 1024.0;

@compute @workgroup_size(16, 16)
fn measure(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.size.x || id.y >= params.size.y) {
        return;
    }
    let rgb = textureLoad(frame, vec2<u32>(id.x, id.y), 0).rgb;
    let y = dot(rgb, params.weights.xyz);
    let u = (rgb.b - y) * params.scale.y * params.scale.x * 0.5 + 0.5;
    let v = 0.5 - (rgb.r - y) * params.scale.z * params.scale.x * 0.5;
    if (u >= 0.0 && u < 1.0 && v >= 0.0 && v < 1.0) {
        atomicAdd(&bins[u32(v * BINS) * 1024u + u32(u * BINS)], 1u);
        return;
    }
    // Off the chart: counted on the plot's edge, where the line from the middle to the colour leaves it, so that
    // it is never hidden. (Not a number, or too far to place: not counted.)
    let du = u - 0.5;
    let dv = v - 0.5;
    let m = max(abs(du), abs(dv));
    if (du == du && dv == dv && m > 0.5 && m < 1.0e6) {
        let k = 0.4995 / m;
        atomicAdd(&bins[u32((0.5 + dv * k) * BINS) * 1024u + u32((0.5 + du * k) * BINS)], 1u);
    }
}
"#;

const PARAMS_BYTES: u64 = 48;
const DRAW_UNIFORM_BYTES: u64 = 192;

const DRAW_SHADER: &str = r#"
struct Placement {
    clip: vec4<f32>,      // the panel in clip space: left, top, right, bottom
    plot: vec4<f32>,      // the square plot in pixels: left, top, width, height
    info: vec4<f32>,      // half extent (Cb/Cr), trace gain, pixels counted, device pixels per logical pixel
    opts: vec4<f32>,      // skin line on, its cos and sin, target box half size (Cb/Cr)
    rings: vec4<f32>,     // the outer ring as a fraction of the plot's radius
    targets: array<vec4<f32>, 6>,   // Cb, Cr of each colour target
    weights: vec4<f32>,   // luma weights for R, G, B, to turn a position back into a colour
};
@group(0) @binding(0) var<uniform> placement: Placement;
@group(0) @binding(1) var<storage, read> bins: array<u32>;

const BINS: u32 = 1024u;
// The plot, grid and skin-tone line are neutral greys; only the trace and the colour targets carry colour.
const PLOT: vec3<f32> = vec3<f32>(0.035, 0.035, 0.035);
const GRID: vec3<f32> = vec3<f32>(0.14, 0.14, 0.14);
const MARK: vec3<f32> = vec3<f32>(0.30, 0.30, 0.30);
const TRACE: vec3<f32> = vec3<f32>(0.86, 0.86, 0.86);
// Targets and trace are tinted by the colour they stand for: the six bars, red, yellow, green, cyan, magenta, blue.
const BARS: array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(0.0, 1.0, 0.0),
    vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(1.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, 1.0));
// How strongly the trace is tinted: its chroma is boosted so that a small saturation still reads as its hue.
const TINT_BOOST: f32 = 2.0;
const TINT_LUMA: f32 = 0.7;

// The colour of a spot on the plot (Cb, Cr): the hue and saturation that put pixels there, at a fixed luma and
// scaled so its strongest channel is full. The middle of the plot is white.
fn tint(cb: f32, cr: f32) -> vec3<f32> {
    let w = placement.weights.xyz;
    let r = TINT_LUMA + cr * TINT_BOOST * 2.0 * (1.0 - w.x);
    let b = TINT_LUMA + cb * TINT_BOOST * 2.0 * (1.0 - w.z);
    let g = (TINT_LUMA - w.x * r - w.z * b) / w.y;
    let c = clamp(vec3<f32>(r, g, b), vec3<f32>(0.0), vec3<f32>(1.0));
    return c / max(max(c.r, c.g), max(c.b, 0.001)) * 0.95;
}

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    return vec4<f32>(mix(placement.clip.x, placement.clip.z, corner.x), mix(placement.clip.y, placement.clip.w, corner.y), 0.0, 1.0);
}

// Pixels counted in the bins under the plot pixel `p` (from the plot's top left).
fn density(p: vec2<f32>) -> f32 {
    let side = placement.plot.z;
    let first = vec2<f32>(floor(p.x / side * f32(BINS)), floor(p.y / side * f32(BINS)));
    let n = u32(max(1.0, ceil(f32(BINS) / side)));
    var sum = 0.0;
    for (var j = 0u; j < n; j++) {
        for (var i = 0u; i < n; i++) {
            let x = u32(first.x) + i;
            let y = u32(first.y) + j;
            if (x < BINS && y < BINS) {
                sum += f32(bins[y * BINS + x]);
            }
        }
    }
    return sum;
}

// How bright the trace is for a count: a phosphor-style response, 1 - exp(-gain * density), where density is
// relative to the picture spread evenly over a twentieth of the plot, so the look does not change with the Panel's
// size or the picture's. Stray pixels stay visible but faint (never below 8 %).
fn brightness(v: f32) -> f32 {
    if (v <= 0.0) {
        return 0.0;
    }
    let even = max(placement.info.z / (placement.plot.z * placement.plot.w * 0.05), 1.0e-3);
    return clamp(max(0.08, 1.0 - exp(-4.0 * placement.info.y * v / even)), 0.0, 1.0);
}

// The density under a plot pixel, spread a little over its neighbours (about 1 device pixel) when the soft spread
// is on, so the trace reads as smooth tone instead of speckle.
fn soft_density(p: vec2<f32>) -> f32 {
    if (placement.rings.y < 0.5) {
        return density(p);
    }
    var sum = 0.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let w = (2.0 - abs(f32(i))) * (2.0 - abs(f32(j))) / 16.0;
            let q = p + vec2<f32>(f32(i), f32(j)) * max(1.0, placement.info.w);
            if (q.x >= 0.0 && q.y >= 0.0 && q.x < placement.plot.z && q.y < placement.plot.w) {
                sum += w * density(q);
            }
        }
    }
    return sum;
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let p = floor(position.xy);
    let plot = placement.plot;
    if (p.x < plot.x || p.y < plot.y || p.x >= plot.x + plot.z || p.y >= plot.y + plot.w) {
        discard;
    }
    let radius = plot.z / 2.0;
    let q = p + vec2<f32>(0.5, 0.5) - vec2<f32>(plot.x + radius, plot.y + radius);   // from the middle, y down
    let half_line = 0.5 * max(1.0, placement.info.w);
    let r = length(q);
    var base = PLOT;
    let outer = placement.rings.x * radius;
    if (abs(q.x) <= half_line || abs(q.y) <= half_line || abs(r - outer / 2.0) <= half_line) {
        base = GRID;
    }
    // The outer ring is a hue wheel: each point is the colour that lands there, muted so it never competes with the
    // trace.
    if (abs(r - outer) <= half_line * 2.0) {
        let d = q / max(r, 0.001);
        base = tint(d.x * 0.5, -d.y * 0.5) * 0.7;
    }
    // The skin-tone line, from the middle to the outer ring.
    if (placement.opts.x > 0.5) {
        let dir = vec2<f32>(placement.opts.y, -placement.opts.z);
        let along = dot(q, dir);
        let across = abs(q.x * dir.y - q.y * dir.x);
        if (along > 0.0 && along <= outer && across <= half_line * 1.5) {
            base = MARK;
        }
    }
    // The colour targets: a box around each.
    let k = radius / placement.info.x;
    let half_box = placement.opts.w * k;
    for (var i = 0u; i < 6u; i++) {
        let c = vec2<f32>(placement.targets[i].x * k, -placement.targets[i].y * k);
        let d = max(abs(q.x - c.x), abs(q.y - c.y));
        if (d <= half_box + half_line && d > half_box - half_line) {
            base = BARS[i] * 0.7;
        }
    }
    let rel = p - plot.xy;
    var shown = brightness(soft_density(rel));
    // Colours off the chart sit on the plot's edge: drawn as a solid bar along it, as the waveform does.
    let band = max(3.0, 3.0 * placement.info.w);
    let to_edge = min(min(rel.x, plot.z - 1.0 - rel.x), min(rel.y, plot.w - 1.0 - rel.y));
    if (to_edge < band) {
        let snapped = vec2<f32>(
            select(select(rel.x, 0.0, rel.x < band), plot.z - 1.0, rel.x > plot.z - 1.0 - band),
            select(select(rel.y, 0.0, rel.y < band), plot.w - 1.0, rel.y > plot.w - 1.0 - band));
        if (density(snapped) > 0.0) {
            shown = 1.0;
        }
    }
    var trace = TRACE;
    if (shown > 0.0) {
        let spot = (p - plot.xy + vec2<f32>(0.5, 0.5)) / plot.z;
        trace = tint((spot.x - 0.5) * 2.0 * placement.info.x, (0.5 - spot.y) * 2.0 * placement.info.x);
    }
    return vec4<f32>(mix(base, trace, shown), 1.0);
}
"#;

struct State {
    bins: wgpu::Buffer,
    /// Pixels in the last frame measured, for the brightness scale.
    pixels: u32,
    bound: Option<(u64, wgpu::BindGroup)>,
    draw_bound: Option<wgpu::BindGroup>,
    measured: bool,
}

/// One vectorscope Panel's GPU state. Cheap to clone: the Feed thread measures through one clone while the
/// window thread draws through another.
#[derive(Clone)]
pub struct Vectorscope {
    zoom: Zoom,
    device: wgpu::Device,
    queue: wgpu::Queue,
    measure: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    draw: wgpu::RenderPipeline,
    placement: wgpu::Buffer,
    config: Arc<Mutex<Config>>,
    state: Arc<Mutex<State>>,
}

impl Vectorscope {
    /// `target`: the format of what [`Display::draw`] draws into.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, zoom: Zoom) -> Vectorscope {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("vectorscope measure"), source: wgpu::ShaderSource::Wgsl(MEASURE_SHADER.into()) });
        let measure = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("vectorscope measure"),
            layout: None,
            module: &shader,
            entry_point: Some("measure"),
            compilation_options: Default::default(),
            cache: None,
        });
        let uniform = |label, size| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("vectorscope draw"), source: wgpu::ShaderSource::Wgsl(DRAW_SHADER.into()) });
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vectorscope draw"),
            layout: None,
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let bins = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vectorscope bins"),
            size: BINS as u64 * BINS as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Vectorscope {
            zoom,
            device: device.clone(),
            queue: queue.clone(),
            measure,
            params: uniform("vectorscope params", PARAMS_BYTES),
            draw,
            placement: uniform("vectorscope placement", DRAW_UNIFORM_BYTES),
            config: Arc::new(Mutex::new(Config { measurement: Measurement::default(), bars: 0.75, skin_line: true, gain: 1.0, spread: true })),
            state: Arc::new(Mutex::new(State { bins, pixels: 0, bound: None, draw_bound: None, measured: false })),
        }
    }

    fn config(&self) -> Config {
        *self.config.lock().unwrap()
    }

    /// Read what was measured back from the GPU (waits for it): for tests and diagnostics, not per frame.
    pub fn counts(&self) -> VectorscopeCounts {
        let state = self.state.lock().unwrap();
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vectorscope readback"),
            size: state.bins.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&state.bins, 0, &readback, 0, None);
        self.queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("the GPU finishes");
        let counts = bytemuck::cast_slice(&readback.get_mapped_range(..).expect("mapped")).to_vec();
        VectorscopeCounts { half_extent: self.zoom.half_extent(), counts }
    }

    /// The quiet label (ADR 0007): the Measured Space and weights in use, the targets, and the skin-tone
    /// angle, which was measured in Resolve 21.2 (#60).
    fn make_label(&self) -> String {
        let config = self.config();
        let m = config.measurement;
        let [r, g, b] = m.weights();
        let name = m.space.name();
        let space = if m.assumed { format!("{name} assumed, measured as sent, unconverted") } else { name.to_string() };
        let mut label = format!(
            "{}  ·  Cb/Cr  ·  {space}  ·  Y = {r:.4} R + {g:.4} G + {b:.4} B {}  ·  {} % targets  ·  colours off the chart sit on the edge",
            self.zoom.name(),
            m.weights_how(),
            (config.bars * 100.0).round()
        );
        if config.skin_line {
            label += &format!("  ·  skin-tone line {SKIN_TONE_LINE_DEGREES}° (as Resolve 21.2 draws it)");
        }
        label
    }
}

impl Display for Vectorscope {
    fn name(&self) -> &'static str {
        self.zoom.name()
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let w = self.config().measurement.weights();
        let mut params = [0u8; PARAMS_BYTES as usize];
        params[..16].copy_from_slice(bytemuck::cast_slice(&[w[0] as f32, w[1] as f32, w[2] as f32, 0.0]));
        params[16..24].copy_from_slice(bytemuck::cast_slice(&[frame.width, frame.height]));
        params[32..44].copy_from_slice(bytemuck::cast_slice(&[
            (1.0 / self.zoom.half_extent()) as f32,
            (1.0 / (2.0 * (1.0 - w[2]))) as f32,
            (1.0 / (2.0 * (1.0 - w[0]))) as f32,
        ]));
        self.queue.write_buffer(&self.params, 0, &params);

        let mut state = self.state.lock().unwrap();
        state.pixels = frame.width * frame.height;
        state.measured = true;
        if state.bound.as_ref().is_none_or(|(id, _)| *id != frame.texture_id) {
            let view = frame.texture.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vectorscope measure"),
                layout: &self.measure.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: state.bins.as_entire_binding() },
                ],
            });
            state.bound = Some((frame.texture_id, bind_group));
        }
        encoder.clear_buffer(&state.bins, 0, None);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("vectorscope measure"), timestamp_writes: None });
        pass.set_pipeline(&self.measure);
        pass.set_bind_group(0, &state.bound.as_ref().unwrap().1, &[]);
        pass.dispatch_workgroups(frame.width.div_ceil(16), frame.height.div_ceil(16), 1);
    }

    fn configure(&self, context: &DisplayContext) {
        let get = |key: &str| context.settings.get(key).map(String::as_str);
        *self.config.lock().unwrap() = Config {
            measurement: crate::output_conversion::measurement(context.colour_setup.clone(), context.luma),
            bars: if get(TARGETS_SETTING) == Some("100") { 1.0 } else { 0.75 },
            skin_line: get(SKIN_LINE_SETTING) != Some("off"),
            spread: get(SPREAD_SETTING) != Some("off"),
            gain: match get(BRIGHTNESS_SETTING) {
                Some("bright") => 4.0,
                Some("dim") => 0.25,
                _ => 1.0,
            },
        };
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn label(&self) -> String {
        self.make_label()
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let mut state = self.state.lock().unwrap();
        let [left, top, width, height] = area;
        if !state.measured || width < 1.0 || height < 1.0 {
            return;
        }
        if state.draw_bound.is_none() {
            state.draw_bound = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vectorscope draw"),
                layout: &self.draw.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.placement.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: state.bins.as_entire_binding() },
                ],
            }));
        }
        let config = self.config();
        let square = PlotSquare::new(area, scale);
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let mut placement = [0f32; (DRAW_UNIFORM_BYTES / 4) as usize];
        placement[..4].copy_from_slice(&[clip(left, surface[0]), -clip(top, surface[1]), clip(left + width, surface[0]), -clip(top + height, surface[1])]);
        placement[4..8].copy_from_slice(&square.plot);
        placement[8..12].copy_from_slice(&[self.zoom.half_extent() as f32, config.gain, state.pixels as f32, scale]);
        let angle = SKIN_TONE_LINE_DEGREES.to_radians();
        placement[12..16].copy_from_slice(&[if config.skin_line { 1.0 } else { 0.0 }, angle.cos() as f32, angle.sin() as f32, TARGET_HALF as f32]);
        placement[16] = (RING / HALF_EXTENT) as f32;
        placement[17] = if config.spread { 1.0 } else { 0.0 };
        for (i, [cb, cr]) in bar_targets(config.measurement.weights(), config.bars).iter().enumerate() {
            placement[20 + i * 4] = *cb as f32;
            placement[21 + i * 4] = *cr as f32;
        }
        let w = config.measurement.weights();
        placement[44..48].copy_from_slice(&[w[0] as f32, w[1] as f32, w[2] as f32, 0.0]);
        self.queue.write_buffer(&self.placement, 0, bytemuck::cast_slice(&placement));
        pass.set_pipeline(&self.draw);
        pass.set_bind_group(0, state.draw_bound.as_ref().unwrap(), &[]);
        pass.draw(0..4, 0..1);
    }

    fn read_back(&self) -> Option<Box<dyn std::any::Any>> {
        Some(Box::new(self.counts()))
    }
}
