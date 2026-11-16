//! The luma waveform: a Scope that plots, for every column of the picture, how many of its pixels have each luma
//! value, on a 10-bit scale (codes 0 to 1023).
//!
//! Measured on the GPU from every pixel of every Feed frame, at full resolution, in 32-bit float: one compute
//! pass over the frame [`crate::gpu_frame`] already put on the GPU. Each waveform column is one picture column,
//! and holds 1026 counts: one per code value, plus a row for everything below code 0 and a row for everything
//! above code 1023, so out-of-range pixels are counted and shown, never cut off.
//!
//! The Colour Setup's Levels decide where values land on the scale (#18): video levels put black (0.0) at code 64
//! and white (1.0) at 940, full (data) levels at 0 and 1023. Resolve hands the Sender the same values either way.
use crate::colour_setup::{KnownColourSetup, Levels, Provenance};
use crate::gpu_frame::GpuFrame;
use std::sync::{Arc, Mutex};

/// Code values on the 10-bit scale: 0 to 1023.
pub const CODES: u32 = 1024;
/// Counts per waveform column: below 0, codes 0 to 1023, above 1023.
const ROWS: u32 = CODES + 2;

/// The luma weights the product uses unless the user picks otherwise. The product owner may switch this to
/// [`LumaWeights::MatchResolve`] (Stage 0 report, open question): this line is the only change needed.
pub const DEFAULT_LUMA_WEIGHTS: LumaWeights = LumaWeights::FromPrimaries;

/// How the luma weights are chosen for the Measured Space (ADR 0004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LumaWeights {
    /// The Y row of the Measured Space's RGB-to-XYZ matrix, from its primaries and white point.
    FromPrimaries,
    /// The weights Resolve's own waveform uses: Rec.709 weights for Rec.709, P3-D65 and P3-DCI; Rec.2020 weights
    /// for Rec.2020 and Rec.2100.
    MatchResolve,
}

/// A colour space the waveform can measure in, by its primaries and white point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasuredSpace {
    Rec709,
    P3D65,
    P3Dci,
    Rec2020,
    Rec2100Pq,
    Rec2100Hlg,
}

/// CIE 1931 xy chromaticities: red, green, blue, white.
type Chromaticities = [[f64; 2]; 4];
const D65: [f64; 2] = [0.3127, 0.3290];
const REC709: Chromaticities = [[0.640, 0.330], [0.300, 0.600], [0.150, 0.060], D65];
const P3_D65: Chromaticities = [[0.680, 0.320], [0.265, 0.690], [0.150, 0.060], D65];
const P3_DCI: Chromaticities = [[0.680, 0.320], [0.265, 0.690], [0.150, 0.060], [0.314, 0.351]];
const REC2020: Chromaticities = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046], D65];

impl MeasuredSpace {
    pub fn name(self) -> &'static str {
        match self {
            MeasuredSpace::Rec709 => "Rec.709",
            MeasuredSpace::P3D65 => "P3-D65",
            MeasuredSpace::P3Dci => "P3-DCI",
            MeasuredSpace::Rec2020 => "Rec.2020",
            MeasuredSpace::Rec2100Pq => "Rec.2100 PQ",
            MeasuredSpace::Rec2100Hlg => "Rec.2100 HLG",
        }
    }

    fn chromaticities(self) -> Chromaticities {
        match self {
            MeasuredSpace::Rec709 => REC709,
            MeasuredSpace::P3D65 => P3_D65,
            MeasuredSpace::P3Dci => P3_DCI,
            MeasuredSpace::Rec2020 | MeasuredSpace::Rec2100Pq | MeasuredSpace::Rec2100Hlg => REC2020,
        }
    }

    /// The space whose primaries Resolve's own waveform takes its weights from (Stage 0 report, section 6).
    fn resolve_weights_space(self) -> MeasuredSpace {
        match self {
            MeasuredSpace::Rec709 | MeasuredSpace::P3D65 | MeasuredSpace::P3Dci => MeasuredSpace::Rec709,
            MeasuredSpace::Rec2020 | MeasuredSpace::Rec2100Pq | MeasuredSpace::Rec2100Hlg => MeasuredSpace::Rec2020,
        }
    }
}

/// The Y row of the RGB-to-XYZ matrix for these primaries and white point: how much luminance each primary adds
/// at full strength when the three together make the white point at Y = 1.
fn y_row([r, g, b, w]: Chromaticities) -> [f64; 3] {
    let xyz = |[x, y]: [f64; 2]| [x / y, 1.0, (1.0 - x - y) / y];
    let (r, g, b, w) = (xyz(r), xyz(g), xyz(b), xyz(w));
    // Solve [r g b] · s = w for each primary's scale s (Cramer's rule); each primary's Y is then its s.
    let det = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| {
        a[0] * (b[1] * c[2] - b[2] * c[1]) - b[0] * (a[1] * c[2] - a[2] * c[1]) + c[0] * (a[1] * b[2] - a[2] * b[1])
    };
    let d = det(r, g, b);
    [det(w, g, b) / d, det(r, w, b) / d, det(r, g, w) / d]
}

impl Levels {
    /// Where values land on the 10-bit scale: code = value × scale + offset. Inside Resolve every value is full
    /// range (0.0 black, 1.0 white) whatever the project's Levels; the Levels decide the codes those values are
    /// delivered as. Video levels: black 64, white 940. Full (data) levels: black 0, white 1023.
    pub fn scale_and_offset(self) -> (f64, f64) {
        match self {
            Levels::Video => (876.0, 64.0),
            Levels::Full => (1023.0, 0.0),
        }
    }

    /// The codes of black and white.
    pub fn black_and_white(self) -> (u32, u32) {
        match self {
            Levels::Video => (64, 940),
            Levels::Full => (0, 1023),
        }
    }

    /// The codes the scale names, top to bottom, besides its edge rows: black, middle and white for video
    /// levels; for full levels black and white are the plot's own edges, so the quarter lines instead.
    pub fn scale_marks(self) -> [u32; 3] {
        match self {
            Levels::Video => [940, 512, 64],
            Levels::Full => [768, 512, 256],
        }
    }

    /// The active range as the label shows it.
    pub fn range_label(self) -> &'static str {
        match self {
            Levels::Video => "Legal 64–940",
            Levels::Full => "Full 0–1023",
        }
    }
}

/// What the waveform measures in, and how: everything its label states. Until the Colour Setup is known, the
/// Measured Space (Rec.709) and the Levels (video, Resolve's default) are assumed, and the label says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Measurement {
    pub space: MeasuredSpace,
    /// The space is an assumption, not known from the Colour Setup.
    pub assumed: bool,
    pub luma: LumaWeights,
    /// How values map to the 10-bit scale.
    pub levels: Levels,
    /// Where the Levels came from; `None` when they are assumed.
    pub levels_from: Option<Provenance>,
}

impl Default for Measurement {
    fn default() -> Measurement {
        Measurement { space: MeasuredSpace::Rec709, assumed: true, luma: DEFAULT_LUMA_WEIGHTS, levels: Levels::Video, levels_from: None }
    }
}

impl Measurement {
    /// What to measure in for this Colour Setup (`None` when not known): its Output Colour Space and its Levels,
    /// each assumed when the Colour Setup does not say.
    pub fn for_colour_setup(known: Option<KnownColourSetup>, luma: LumaWeights) -> Measurement {
        let known = known.as_ref();
        let space = known.and_then(|c| c.setup.measured_space());
        let levels = known.and_then(|c| c.setup.levels.map(|levels| (levels, c.provenance)));
        Measurement {
            space: space.unwrap_or(MeasuredSpace::Rec709),
            assumed: space.is_none(),
            luma,
            levels: levels.map_or(Levels::Video, |(levels, _)| levels),
            levels_from: levels.map(|(_, from)| from),
        }
    }

    /// Luma weights for red, green and blue.
    pub fn weights(&self) -> [f64; 3] {
        match self.luma {
            LumaWeights::FromPrimaries => y_row(self.space.chromaticities()),
            LumaWeights::MatchResolve => y_row(self.space.resolve_weights_space().chromaticities()),
        }
    }

    /// The waveform's quiet label: the scale, the Measured Space and the luma weights in use.
    pub fn label(&self) -> String {
        let [r, g, b] = self.weights();
        let name = self.space.name();
        let space = if self.assumed { format!("{name} assumed") } else { name.to_string() };
        let how = match self.luma {
            LumaWeights::FromPrimaries => "from primaries",
            LumaWeights::MatchResolve => "to match Resolve",
        };
        let range = self.levels.range_label();
        let scale = match self.levels_from {
            None => format!("{range} assumed"),
            Some(Provenance::Project) => range.to_string(),
            Some(Provenance::Manual) => format!("{range} set manually"),
        };
        format!("Luma  ·  10-bit {scale}  ·  {space}  ·  Y = {r:.4} R + {g:.4} G + {b:.4} B {how}")
    }
}

const PARAMS_BYTES: u64 = 48;

const MEASURE_SHADER: &str = r#"
struct Params {
    weights: vec4<f32>,   // luma weights for R, G, B
    size: vec4<u32>,      // picture width, height
    levels: vec4<f32>,    // the Levels: code = luma * levels.x + levels.y
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var frame: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> counts: array<u32>;

const ROWS: u32 = 1026u;
const COLUMNS: u32 = 2u;    // picture columns per workgroup
const LANES: u32 = 128u;    // threads per picture column
var<workgroup> local_counts: array<atomic<u32>, 2052>;   // COLUMNS * ROWS

// Each workgroup owns two whole picture columns: it counts them in workgroup memory, then writes every count of
// its columns (zeros too), so the counts buffer needs no clearing between frames.
@compute @workgroup_size(2, 128)
fn measure(@builtin(local_invocation_id) local: vec3<u32>, @builtin(local_invocation_index) index: u32, @builtin(workgroup_id) group: vec3<u32>) {
    for (var i = index; i < COLUMNS * ROWS; i += COLUMNS * LANES) {
        atomicStore(&local_counts[i], 0u);
    }
    workgroupBarrier();
    let x = group.x * COLUMNS + local.x;
    if (x < params.size.x) {
        for (var y = local.y; y < params.size.y; y += LANES) {
            let rgb = textureLoad(frame, vec2<u32>(x, y), 0).rgb;
            let code = dot(rgb, params.weights.xyz) * params.levels.x + params.levels.y;
            var row = 0u;                                   // below code 0 (and NaN: not a value at all)
            if (code >= 1023.5) {
                row = ROWS - 1u;                            // above code 1023
            } else if (code >= -0.5) {
                row = u32(code + 0.5) + 1u;                 // nearest code
            }
            atomicAdd(&local_counts[local.x * ROWS + row], 1u);
        }
    }
    workgroupBarrier();
    for (var i = index; i < COLUMNS * ROWS; i += COLUMNS * LANES) {
        let column = group.x * COLUMNS + i / ROWS;
        if (column < params.size.x) {
            counts[column * ROWS + i % ROWS] = atomicLoad(&local_counts[i]);
        }
    }
}
"#;

/// How many pixels of each picture column landed at each code value: the waveform's contents.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveformCounts {
    columns: u32,
    counts: Vec<u32>,
}

impl WaveformCounts {
    /// Waveform columns: one per picture column.
    pub fn columns(&self) -> u32 {
        self.columns
    }
    /// Pixels of picture column `column` (from the left) at 10-bit code `code` (0 to 1023).
    pub fn at(&self, column: u32, code: u32) -> u32 {
        assert!(code < CODES);
        self.counts[(column * ROWS + code + 1) as usize]
    }
    /// Pixels of `column` below code 0.
    pub fn below(&self, column: u32) -> u32 {
        self.counts[(column * ROWS) as usize]
    }
    /// Pixels of `column` above code 1023.
    pub fn above(&self, column: u32) -> u32 {
        self.counts[(column * ROWS + ROWS - 1) as usize]
    }
    /// Every pixel counted.
    pub fn total(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }
}

/// Where the waveform's parts go inside its panel, in pixels: a gutter on the left for the scale's numbers, the
/// plot of codes 0 to 1023, and an edge row above and below it for pixels over and under the scale.
/// Rectangles are left, top, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotLayout {
    plot: [f32; 4],
    above: [f32; 4],
    below: [f32; 4],
}

/// Logical pixels.
const EDGE_ROW: f32 = 5.0;
const EDGE_GAP: f32 = 6.0;

impl PlotLayout {
    /// The layout inside `area` (left, top, width, height) at `scale` device pixels per logical pixel. The gutter
    /// keeps its full width (the scale's numbers stay readable however small the panel), so the plot is exactly
    /// the waveform panel [`crate::layout::WindowLayout`] sets beside it.
    pub fn new([left, top, width, height]: [f32; 4], scale: f32) -> PlotLayout {
        let gutter = (crate::layout::GUTTER * scale).round().min((width - 1.0).max(0.0));
        let (row, gap) = ((EDGE_ROW * scale).round(), (EDGE_GAP * scale).round());
        let (l, w) = (left + gutter, (width - gutter).max(1.0));
        let plot_top = top + row + gap;
        let plot_height = (height - 2.0 * (row + gap)).max(1.0);
        PlotLayout { plot: [l, plot_top, w, plot_height], above: [l, top, w, row], below: [l, plot_top + plot_height + gap, w, row] }
    }
    /// The plot of codes 0 (bottom) to 1023 (top).
    pub fn plot(&self) -> [f32; 4] {
        self.plot
    }
    /// The edge row for pixels above code 1023.
    pub fn above(&self) -> [f32; 4] {
        self.above
    }
    /// The edge row for pixels below code 0.
    pub fn below(&self) -> [f32; 4] {
        self.below
    }
    /// The middle of the band where `code` is plotted.
    pub fn y_of_code(&self, code: u32) -> f32 {
        let [_, top, _, height] = self.plot;
        top + height - (code as f32 + 0.5) / CODES as f32 * height
    }
    /// The middle of where picture column `column` of `columns` is plotted.
    pub fn x_of_column(&self, column: u32, columns: u32) -> f32 {
        let [left, _, width, _] = self.plot;
        left + (column as f32 + 0.5) / columns as f32 * width
    }
}

/// Which of the scale's numbers to show so none overlaps another: `marks` are each number's height on screen
/// and its rank (lower matters more), and a number is kept when it is at least `line` away from every number of
/// the same or a higher rank kept before it. Ties go to the earlier mark.
pub fn marks_that_fit(marks: &[(f32, u32)], line: f32) -> Vec<bool> {
    let mut order: Vec<usize> = (0..marks.len()).collect();
    order.sort_by_key(|&i| marks[i].1);
    let mut shown = vec![false; marks.len()];
    for i in order {
        if (0..marks.len()).all(|j| !shown[j] || (marks[i].0 - marks[j].0).abs() >= line) {
            shown[i] = true;
        }
    }
    shown
}

const DRAW_UNIFORM_BYTES: u64 = 96;

const DRAW_SHADER: &str = r#"
struct Placement {
    clip: vec4<f32>,      // the panel in clip space: left, top, right, bottom
    plot: vec4<f32>,      // rectangles in pixels: left, top, width, height
    above: vec4<f32>,
    below: vec4<f32>,
    info: vec4<f32>,      // waveform columns, picture height, device pixels per logical pixel
    legal: vec4<f32>,     // the codes of black and white for the Levels in use
};
@group(0) @binding(0) var<uniform> placement: Placement;
@group(0) @binding(1) var<storage, read> counts: array<u32>;

const ROWS: u32 = 1026u;
// Neutral greys only: nothing on screen may tint the eye.
const PLOT: vec3<f32> = vec3<f32>(0.035, 0.035, 0.035);
const EDGE: vec3<f32> = vec3<f32>(0.085, 0.085, 0.085);
const GRID: vec3<f32> = vec3<f32>(0.14, 0.14, 0.14);
const LEGAL: vec3<f32> = vec3<f32>(0.30, 0.30, 0.30);
const TRACE: vec3<f32> = vec3<f32>(0.86, 0.86, 0.86);

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    return vec4<f32>(mix(placement.clip.x, placement.clip.z, corner.x), mix(placement.clip.y, placement.clip.w, corner.y), 0.0, 1.0);
}

fn inside(p: vec2<f32>, r: vec4<f32>) -> bool {
    return p.x >= r.x && p.x < r.x + r.z && p.y >= r.y && p.y < r.y + r.w;
}

// The picture columns under screen column `x`: [first, end).
fn columns_at(x: f32) -> vec2<u32> {
    let n = placement.info.x;
    let first = clamp(floor((x - placement.plot.x) / placement.plot.z * n), 0.0, n - 1.0);
    let end = clamp(max(ceil((x + 1.0 - placement.plot.x) / placement.plot.z * n), first + 1.0), 1.0, n);
    return vec2<u32>(u32(first), u32(end));
}

// Mean pixels per picture column in rows [first_row, end_row) of the counts, over the picture columns under x.
fn density(x: f32, first_row: u32, end_row: u32) -> f32 {
    let columns = columns_at(x);
    let step = max(1u, (columns.y - columns.x) / 64u);   // a very narrow panel samples the columns
    var sum = 0.0;
    var sampled = 0.0;
    for (var c = columns.x; c < columns.y; c += step) {
        for (var r = first_row; r < end_row; r++) {
            sum += f32(counts[c * ROWS + r]);
        }
        sampled += 1.0;
    }
    return sum / sampled;
}

// How bright the trace is for a density: logarithmic, and never invisible while even one pixel is there.
fn brightness(v: f32, floor_level: f32) -> f32 {
    if (v <= 0.0) {
        return 0.0;
    }
    return clamp(max(floor_level, log2(1.0 + v) / log2(1.0 + placement.info.y / 8.0)), 0.0, 1.0);
}

fn y_of_code(code: f32) -> f32 {
    return placement.plot.y + placement.plot.w - (code + 0.5) / 1024.0 * placement.plot.w;
}

fn on_line(y: f32, code: f32) -> bool {
    return floor(y_of_code(code)) == y;
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let p = floor(position.xy);
    if (inside(p, placement.plot)) {
        var base = PLOT;
        if (on_line(p.y, 0.0) || on_line(p.y, 256.0) || on_line(p.y, 512.0) || on_line(p.y, 768.0) || on_line(p.y, 1023.0)) {
            base = GRID;
        }
        if (on_line(p.y, placement.legal.x) || on_line(p.y, placement.legal.y)) {
            base = LEGAL;
        }
        // The codes under this screen row (bottom of the plot is code 0).
        let bottom = placement.plot.y + placement.plot.w;
        let first = u32(clamp(floor((bottom - p.y - 1.0) / placement.plot.w * 1024.0), 0.0, 1023.0));
        let end = u32(clamp(max(ceil((bottom - p.y) / placement.plot.w * 1024.0), f32(first) + 1.0), 1.0, 1024.0));
        return vec4<f32>(mix(base, TRACE, brightness(density(p.x, first + 1u, end + 1u), 0.25)), 1.0);
    }
    if (inside(p, placement.above)) {
        return vec4<f32>(mix(EDGE, TRACE, brightness(density(p.x, ROWS - 1u, ROWS), 0.8)), 1.0);
    }
    if (inside(p, placement.below)) {
        return vec4<f32>(mix(EDGE, TRACE, brightness(density(p.x, 0u, 1u), 0.8)), 1.0);
    }
    discard;
}
"#;

struct Counts {
    buffer: wgpu::Buffer,
    columns: u32,
    /// Picture rows: the most pixels one column can hold.
    picture_height: u32,
}

struct State {
    counts: Option<Counts>,
    bound: Option<(u64, wgpu::BindGroup)>,
    draw_bound: Option<wgpu::BindGroup>,
}

/// The waveform's GPU state. Cheap to clone: the Feed thread measures through one clone while the window thread
/// draws through another.
#[derive(Clone)]
pub struct Waveform {
    device: wgpu::Device,
    queue: wgpu::Queue,
    measure: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    draw: wgpu::RenderPipeline,
    placement: wgpu::Buffer,
    measurement: Arc<Mutex<Measurement>>,
    state: Arc<Mutex<State>>,
}

impl Waveform {
    /// `target`: the format of what [`Waveform::draw`] draws into.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> Waveform {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("waveform measure"), source: wgpu::ShaderSource::Wgsl(MEASURE_SHADER.into()) });
        let measure = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("waveform measure"),
            layout: None,
            module: &shader,
            entry_point: Some("measure"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform params"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("waveform draw"), source: wgpu::ShaderSource::Wgsl(DRAW_SHADER.into()) });
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("waveform draw"),
            layout: None,
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let placement = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform placement"),
            size: DRAW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Waveform {
            device: device.clone(),
            queue: queue.clone(),
            measure,
            params,
            draw,
            placement,
            measurement: Arc::new(Mutex::new(Measurement::default())),
            state: Arc::new(Mutex::new(State { counts: None, bound: None, draw_bound: None })),
        }
    }

    /// What to measure in from the next frame on (from the Colour Setup, or the user's choice of luma weights).
    pub fn set_measurement(&self, measurement: Measurement) {
        *self.measurement.lock().unwrap() = measurement;
    }

    pub fn measurement(&self) -> Measurement {
        *self.measurement.lock().unwrap()
    }

    /// The quiet label: the scale, the Measured Space and the luma weights in use.
    pub fn label(&self) -> String {
        self.measurement().label()
    }

    /// Record the measurement of `frame` into `encoder` (see [`crate::gpu_frame::FrameUploader::upload`]).
    pub fn measure(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let measurement = self.measurement();
        let [r, g, b] = measurement.weights();
        let (scale, offset) = measurement.levels.scale_and_offset();
        let mut params = [0u8; PARAMS_BYTES as usize];
        params[..16].copy_from_slice(bytemuck::cast_slice(&[r as f32, g as f32, b as f32, 0.0]));
        params[16..24].copy_from_slice(bytemuck::cast_slice(&[frame.width, frame.height]));
        params[32..40].copy_from_slice(bytemuck::cast_slice(&[scale as f32, offset as f32]));
        self.queue.write_buffer(&self.params, 0, &params);

        let mut state = self.state.lock().unwrap();
        if state.counts.as_ref().is_none_or(|c| c.columns != frame.width) {
            state.counts = Some(Counts {
                buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("waveform counts"),
                    size: frame.width as u64 * ROWS as u64 * 4,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                columns: frame.width,
                picture_height: frame.height,
            });
            state.bound = None;
            state.draw_bound = None;
        }
        state.counts.as_mut().unwrap().picture_height = frame.height;
        if state.bound.as_ref().is_none_or(|(id, _)| *id != frame.texture_id) {
            let view = frame.texture.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("waveform measure"),
                layout: &self.measure.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: state.counts.as_ref().unwrap().buffer.as_entire_binding() },
                ],
            });
            state.bound = Some((frame.texture_id, bind_group));
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("waveform measure"), timestamp_writes: None });
        pass.set_pipeline(&self.measure);
        pass.set_bind_group(0, &state.bound.as_ref().unwrap().1, &[]);
        pass.dispatch_workgroups(frame.width.div_ceil(2), 1, 1);
    }

    /// Draw the waveform into `area` (left, top, width, height in pixels of a `surface`-sized target), laid out as
    /// [`PlotLayout::new`] says. Draws nothing until a frame has been measured.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let mut state = self.state.lock().unwrap();
        let Some(counts) = &state.counts else { return };
        let [left, top, width, height] = area;
        if width < 1.0 || height < 1.0 {
            return;
        }
        let (columns, picture_height) = (counts.columns, counts.picture_height);
        if state.draw_bound.is_none() {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("waveform draw"),
                layout: &self.draw.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.placement.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: counts.buffer.as_entire_binding() },
                ],
            });
            state.draw_bound = Some(bind_group);
        }
        let layout = PlotLayout::new(area, scale);
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let mut placement = [0f32; (DRAW_UNIFORM_BYTES / 4) as usize];
        placement[..4].copy_from_slice(&[clip(left, surface[0]), -clip(top, surface[1]), clip(left + width, surface[0]), -clip(top + height, surface[1])]);
        placement[4..8].copy_from_slice(&layout.plot);
        placement[8..12].copy_from_slice(&layout.above);
        placement[12..16].copy_from_slice(&layout.below);
        placement[16..19].copy_from_slice(&[columns as f32, picture_height as f32, scale]);
        let (black, white) = self.measurement().levels.black_and_white();
        placement[20..22].copy_from_slice(&[black as f32, white as f32]);
        self.queue.write_buffer(&self.placement, 0, bytemuck::cast_slice(&placement));
        pass.set_pipeline(&self.draw);
        pass.set_bind_group(0, state.draw_bound.as_ref().unwrap(), &[]);
        pass.draw(0..4, 0..1);
    }

    /// Read the waveform's contents back from the GPU (waits for it): for tests and diagnostics, not per frame.
    pub fn counts(&self) -> WaveformCounts {
        let state = self.state.lock().unwrap();
        let Some(counts) = &state.counts else { return WaveformCounts { columns: 0, counts: Vec::new() } };
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform readback"),
            size: counts.buffer.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&counts.buffer, 0, &readback, 0, None);
        self.queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("the GPU finishes");
        let data = bytemuck::cast_slice(&readback.get_mapped_range(..).expect("mapped")).to_vec();
        WaveformCounts { columns: counts.columns, counts: data }
    }
}
