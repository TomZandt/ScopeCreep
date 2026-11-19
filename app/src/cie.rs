//! The CIE chromaticity plot (ticket #62, spec #49 stories 33 to 37): every pixel of the picture is placed on the
//! CIE 1931 xy diagram, with the spectral locus and the gamut triangles of the Output Colour Space and of the
//! Timeline Colour Space.
//!
//! Think of a dartboard hung on the wall: the picture's pixels are darts, the board is the xy diagram, and each
//! pixel's colour (not its brightness) decides where its dart lands. A GPU pass throws every pixel of every frame
//! at a fine grid of bins (0.001 in x and y) and counts the hits; the draw pass turns the counts into
//! phosphor-style brightness (many hits glow, one hit is never invisible).
//!
//! The picture the displays see is in the Output Colour Space's encoding (after Output Conversion), so the pass
//! first undoes that encoding to linear light (the encoding of each Measured Space is [`Encoding`]), then goes
//! RGB to XYZ with the space's own primaries, then to xy. A space the app cannot name is assumed to be Rec.709
//! and the label says so (the waveform's rule, ADR 0007).
use crate::colour_setup::{KnownColourSetup, TimelineSpace};
use crate::gpu_frame::GpuFrame;
use crate::settings::{Choice, SettingDef};
use crate::measurement::{LumaWeights, MeasuredSpace};
use scopecreep_accuracy::colour::{AP1, Chromaticities, DWG, P3_D65, REC2020, REC709, rgb_to_xyz};
use scopecreep_accuracy::transfer;
use std::sync::{Arc, Mutex};

/// The part of the xy plane the plot shows: wide enough for every gamut drawn, including DaVinci Wide Gamut's
/// blue (y below 0) and green (y near 1).
pub const DOMAIN: [f32; 4] = [-0.05, -0.15, 0.85, 1.0];
/// Width and height of one counting bin, in xy.
pub const BIN: f32 = 0.001;
pub const BINS_X: u32 = 900;
pub const BINS_Y: u32 = 1150;
const BIN_COUNT: usize = (BINS_X * BINS_Y) as usize;

/// CIE 1931 2 degree standard observer, chromaticity (x, y) of the spectral colours, 380 to 700 nm every 10 nm.
pub const SPECTRAL_LOCUS: [[f32; 2]; 33] = [
    [0.1741, 0.0050], [0.1738, 0.0049], [0.1733, 0.0048], [0.1726, 0.0048], [0.1714, 0.0051], [0.1689, 0.0069],
    [0.1644, 0.0109], [0.1566, 0.0177], [0.1440, 0.0297], [0.1241, 0.0578], [0.0913, 0.1327], [0.0454, 0.2950],
    [0.0082, 0.5384], [0.0139, 0.7502], [0.0743, 0.8338], [0.1547, 0.8059], [0.2296, 0.7543], [0.3016, 0.6923],
    [0.3731, 0.6245], [0.4441, 0.5547], [0.5125, 0.4866], [0.5752, 0.4242], [0.6270, 0.3725], [0.6658, 0.3340],
    [0.6915, 0.3083], [0.7079, 0.2920], [0.7190, 0.2809], [0.7260, 0.2740], [0.7300, 0.2700], [0.7320, 0.2680],
    [0.7334, 0.2666], [0.7344, 0.2656], [0.7347, 0.2653],
];

const P3_DCI: Chromaticities = Chromaticities { white: [0.314, 0.351], ..P3_D65 };

/// How a Measured Space's code values are turned back into linear light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Encoding {
    /// A pure power, applied to the size of the value and keeping its sign (out-of-gamut channels go negative).
    Gamma(f32),
    /// SMPTE ST 2084.
    Pq,
    /// The BT.2100 HLG inverse OETF (scene light; the system gamma is not applied).
    Hlg,
}

impl Encoding {
    fn code(self) -> (u32, f32) {
        match self {
            Encoding::Gamma(g) => (0, g),
            Encoding::Pq => (1, 0.0),
            Encoding::Hlg => (2, 0.0),
        }
    }
    fn text(self) -> String {
        match self {
            Encoding::Gamma(g) => format!("gamma {g}"),
            Encoding::Pq => "ST 2084".into(),
            Encoding::Hlg => "HLG".into(),
        }
    }
}

fn output_chromaticities(space: MeasuredSpace) -> Chromaticities {
    match space {
        MeasuredSpace::Rec709 => REC709,
        MeasuredSpace::P3D65 => P3_D65,
        MeasuredSpace::P3Dci => P3_DCI,
        MeasuredSpace::Rec2020 | MeasuredSpace::Rec2100Pq | MeasuredSpace::Rec2100Hlg => REC2020,
    }
}

fn output_encoding(space: MeasuredSpace) -> Encoding {
    match space {
        MeasuredSpace::Rec709 | MeasuredSpace::Rec2020 => Encoding::Gamma(2.4),
        MeasuredSpace::P3D65 | MeasuredSpace::P3Dci => Encoding::Gamma(2.6),
        MeasuredSpace::Rec2100Pq => Encoding::Pq,
        MeasuredSpace::Rec2100Hlg => Encoding::Hlg,
    }
}

/// The gamut of a Timeline Colour Space by its primaries, `None` when the space is not one the plot knows.
pub fn timeline_chromaticities(space: &TimelineSpace) -> Option<Chromaticities> {
    match space {
        TimelineSpace::DaVinciWideGamut => Some(DWG),
        TimelineSpace::Acescct | TimelineSpace::Acescc => Some(AP1),
        TimelineSpace::Rec709Gamma24 => Some(REC709),
        TimelineSpace::Other(name) if name == "ACEScg" => Some(AP1),
        TimelineSpace::Other(_) => None,
    }
}

fn corners(c: &Chromaticities) -> [[f32; 2]; 3] {
    [c.red, c.green, c.blue].map(|[x, y]| [x as f32, y as f32])
}

/// What the plot shows for a Colour Setup and the user's settings: everything its label states.
#[derive(Clone, Debug, PartialEq)]
pub struct Plot {
    pub output: MeasuredSpace,
    /// The Output Colour Space is an assumption (the picture is measured as sent), not known.
    pub assumed: bool,
    pub encoding: Encoding,
    /// Timeline Colour Space as Resolve names it, `None` when the Colour Setup is not known.
    pub timeline_name: Option<String>,
    /// The Timeline gamut, `None` when the space is not one the plot knows.
    pub timeline: Option<[[f32; 2]; 3]>,
    pub show_timeline: bool,
    /// Trace brightness: the gain on the counts.
    pub gain: f32,
}

/// The key of the trace brightness setting, and of the Timeline triangle setting.
pub const BRIGHTNESS_SETTING: &str = "brightness";
pub const TIMELINE_SETTING: &str = "timeline";

const SETTINGS: [SettingDef; 2] = [
    SettingDef {
        key: BRIGHTNESS_SETTING,
        label: "Trace brightness",
        choices: &[
            Choice { value: "normal", text: "Normal" },
            Choice { value: "bright", text: "Bright (sparse colours)" },
            Choice { value: "dim", text: "Dim (dense colours)" },
        ],
    },
    SettingDef {
        key: TIMELINE_SETTING,
        label: "Timeline triangle",
        choices: &[Choice { value: "on", text: "Shown" }, Choice { value: "off", text: "Hidden" }],
    },
];

impl Plot {
    pub fn new(known: Option<KnownColourSetup>, settings: &crate::settings::Settings) -> Plot {
        let m = crate::output_conversion::measurement(known.clone(), LumaWeights::FromPrimaries);
        let timeline_space = known.as_ref().map(|k| k.setup.timeline_space.clone());
        Plot {
            output: m.space,
            assumed: m.assumed,
            encoding: output_encoding(m.space),
            timeline_name: known.as_ref().map(|k| k.setup.timeline.clone()),
            timeline: timeline_space.as_ref().and_then(timeline_chromaticities).map(|c| corners(&c)),
            show_timeline: settings.get(TIMELINE_SETTING).map(String::as_str) != Some("off"),
            gain: match settings.get(BRIGHTNESS_SETTING).map(String::as_str) {
                Some("bright") => 4.0,
                Some("dim") => 0.25,
                _ => 1.0,
            },
        }
    }

    pub fn output_triangle(&self) -> [[f32; 2]; 3] {
        corners(&output_chromaticities(self.output))
    }

    /// The Timeline triangle as drawn: `None` when turned off or not known.
    pub fn timeline_triangle(&self) -> Option<[[f32; 2]; 3]> {
        if self.show_timeline { self.timeline } else { None }
    }

    /// The quiet label: both spaces by name, and how the pixels were read (ADR 0007).
    pub fn label(&self) -> String {
        let name = self.output.name();
        let output = if self.assumed {
            format!("Output {name} assumed, measured as sent, unconverted")
        } else {
            format!("Output {name}, {} decoded", self.encoding.text())
        };
        let timeline = match (&self.timeline_name, self.timeline, self.show_timeline) {
            (None, _, _) => "Timeline space not known".to_string(),
            (Some(n), None, _) => format!("Timeline {n}, gamut not known"),
            (Some(n), Some(_), true) => format!("Timeline {n}"),
            (Some(n), Some(_), false) => format!("Timeline {n}, triangle hidden"),
        };
        format!("CIE 1931 xy  ·  {output}  ·  {timeline}")
    }
}

/// Where the plot goes inside its Panel: the xy domain scaled to fit, centred, with a small margin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotLayout {
    /// left, top, width, height in pixels.
    pub plot: [f32; 4],
}

impl PlotLayout {
    pub fn new([left, top, width, height]: [f32; 4], scale: f32) -> PlotLayout {
        let margin = (6.0 * scale).round();
        let (aw, ah) = ((width - 2.0 * margin).max(1.0), (height - 2.0 * margin).max(1.0));
        let (dw, dh) = (DOMAIN[2] - DOMAIN[0], DOMAIN[3] - DOMAIN[1]);
        let per_unit = (aw / dw).min(ah / dh);
        let (w, h) = ((dw * per_unit).floor().max(1.0), (dh * per_unit).floor().max(1.0));
        PlotLayout { plot: [(left + (width - w) / 2.0).round(), (top + (height - h) / 2.0).round(), w, h] }
    }

    /// Where xy is on screen.
    pub fn screen_of(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        let [l, t, w, h] = self.plot;
        [l + (x - DOMAIN[0]) / (DOMAIN[2] - DOMAIN[0]) * w, t + h - (y - DOMAIN[1]) / (DOMAIN[3] - DOMAIN[1]) * h]
    }
}

/// How many pixels of the picture landed in each xy bin: the plot's contents.
#[derive(Clone, Debug, PartialEq)]
pub struct CieCounts {
    bins: Vec<u32>,
    /// Pixels with no chromaticity (no light, or not a number).
    pub no_light: u32,
    /// Pixels whose chromaticity is outside [`DOMAIN`].
    pub outside: u32,
}

impl CieCounts {
    /// Pixels in the bin at (`bx`, `by`), `by` counted up from `DOMAIN[1]`.
    pub fn bin(&self, bx: u32, by: u32) -> u32 {
        self.bins[(by * BINS_X + bx) as usize]
    }
    /// Pixels within `tolerance` of `xy` in both x and y.
    pub fn near(&self, [x, y]: [f64; 2], tolerance: f64) -> u64 {
        let range = |v: f64, lo: f32, n: u32| {
            let first = ((v - tolerance - lo as f64) / BIN as f64).floor().max(0.0) as u32;
            let last = (((v + tolerance - lo as f64) / BIN as f64).floor() as i64).clamp(0, n as i64 - 1) as u32;
            first..=last
        };
        let mut sum = 0u64;
        for by in range(y, DOMAIN[1], BINS_Y) {
            for bx in range(x, DOMAIN[0], BINS_X) {
                // Only bins whose centre is within the tolerance.
                let (cx, cy) = (DOMAIN[0] as f64 + (bx as f64 + 0.5) * BIN as f64, DOMAIN[1] as f64 + (by as f64 + 0.5) * BIN as f64);
                if (cx - x).abs() <= tolerance && (cy - y).abs() <= tolerance {
                    sum += self.bin(bx, by) as u64;
                }
            }
        }
        sum
    }
    /// Every pixel counted.
    pub fn total(&self) -> u64 {
        self.bins.iter().map(|&c| c as u64).sum::<u64>() + self.no_light as u64 + self.outside as u64
    }
}

const PARAMS_BYTES: u64 = 80;

fn measure_shader() -> String {
    format!(
        r#"
struct Params {{
    x_col: vec4<f32>,     // X = dot(rgb, x_col.xyz), likewise Y and Z
    y_col: vec4<f32>,
    z_col: vec4<f32>,
    info: vec4<u32>,      // picture width, height, encoding (0 gamma, 1 ST 2084, 2 HLG)
    gamma: vec4<f32>,
}};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var frame: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> bins: array<atomic<u32>>;

const BINS_X: u32 = {BINS_X}u;
const BINS_Y: u32 = {BINS_Y}u;
const NO_LIGHT: u32 = {BIN_COUNT}u;
const OUTSIDE: u32 = {outside}u;
const DOMAIN: vec4<f32> = vec4<f32>({d0:?}, {d1:?}, {d2:?}, {d3:?});
const BIN: f32 = {BIN:?};
const PQ_M1: f32 = {pq_m1:?};
const PQ_M2: f32 = {pq_m2:?};
const PQ_C1: f32 = {pq_c1:?};
const PQ_C2: f32 = {pq_c2:?};
const PQ_C3: f32 = {pq_c3:?};

fn decode(v: f32) -> f32 {{
    switch params.info.z {{
        case 0u: {{
            return sign(v) * pow(abs(v), params.gamma.x);
        }}
        case 1u: {{
            let p = pow(max(v, 0.0), 1.0 / PQ_M2);
            return pow(max(p - PQ_C1, 0.0) / (PQ_C2 - PQ_C3 * p), 1.0 / PQ_M1);
        }}
        default: {{
            if (v <= 0.5) {{
                return v * v / 3.0;
            }}
            return (exp((v - 0.55991073) / 0.17883277) + 0.28466892) / 12.0;
        }}
    }}
}}

@compute @workgroup_size(8, 8)
fn measure(@builtin(global_invocation_id) id: vec3<u32>) {{
    if (id.x >= params.info.x || id.y >= params.info.y) {{
        return;
    }}
    let rgb = textureLoad(frame, id.xy, 0).rgb;
    let lin = vec3<f32>(decode(rgb.r), decode(rgb.g), decode(rgb.b));
    let xyz = vec3<f32>(dot(lin, params.x_col.xyz), dot(lin, params.y_col.xyz), dot(lin, params.z_col.xyz));
    let sum = xyz.x + xyz.y + xyz.z;
    if (!(xyz.y > 0.0 && sum > 0.0)) {{
        atomicAdd(&bins[NO_LIGHT], 1u);
        return;
    }}
    let bx = floor((xyz.x / sum - DOMAIN.x) / BIN);
    let by = floor((xyz.y / sum - DOMAIN.y) / BIN);
    if (!(bx >= 0.0 && bx < f32(BINS_X) && by >= 0.0 && by < f32(BINS_Y))) {{
        atomicAdd(&bins[OUTSIDE], 1u);
        return;
    }}
    atomicAdd(&bins[u32(by) * BINS_X + u32(bx)], 1u);
}}
"#,
        outside = BIN_COUNT + 1,
        d0 = DOMAIN[0],
        d1 = DOMAIN[1],
        d2 = DOMAIN[2],
        d3 = DOMAIN[3],
        pq_m1 = transfer::PQ_M1 as f32,
        pq_m2 = transfer::PQ_M2 as f32,
        pq_c1 = transfer::PQ_C1 as f32,
        pq_c2 = transfer::PQ_C2 as f32,
        pq_c3 = transfer::PQ_C3 as f32,
    )
}

const DRAW_UNIFORM_FLOATS: usize = 4 * 8 + 4 * 17;

fn draw_shader() -> String {
    format!(
        r#"
struct Placement {{
    clip: vec4<f32>,       // the panel in clip space: left, top, right, bottom
    plot: vec4<f32>,       // the plot in pixels: left, top, width, height
    info: vec4<f32>,       // gain, total pixels, show timeline (0 or 1), timeline known (0 or 1)
    output_a: vec4<f32>,   // Output triangle: red, green
    output_b: vec4<f32>,   // blue
    timeline_a: vec4<f32>,
    timeline_b: vec4<f32>,
    pad: vec4<f32>,
    locus: array<vec4<f32>, 17>,   // two points each
}};
@group(0) @binding(0) var<uniform> placement: Placement;
@group(0) @binding(1) var<storage, read> bins: array<u32>;

const BINS_X: u32 = {BINS_X}u;
const BINS_Y: u32 = {BINS_Y}u;
const DOMAIN: vec4<f32> = vec4<f32>({d0:?}, {d1:?}, {d2:?}, {d3:?});
// Neutral greys only: nothing on screen may tint the eye.
const PLOT: vec3<f32> = vec3<f32>(0.035, 0.035, 0.035);
const GRID: vec3<f32> = vec3<f32>(0.075, 0.075, 0.075);
const LOCUS: vec3<f32> = vec3<f32>(0.42, 0.42, 0.42);
const OUTPUT: vec3<f32> = vec3<f32>(0.62, 0.62, 0.62);
const TIMELINE: vec3<f32> = vec3<f32>(0.36, 0.36, 0.36);
const TRACE: vec3<f32> = vec3<f32>(0.95, 0.95, 0.95);

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {{
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    return vec4<f32>(mix(placement.clip.x, placement.clip.z, corner.x), mix(placement.clip.y, placement.clip.w, corner.y), 0.0, 1.0);
}}

fn screen_of(xy: vec2<f32>) -> vec2<f32> {{
    let p = placement.plot;
    return vec2<f32>(p.x + (xy.x - DOMAIN.x) / (DOMAIN.z - DOMAIN.x) * p.z, p.y + p.w - (xy.y - DOMAIN.y) / (DOMAIN.w - DOMAIN.y) * p.w);
}}

fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {{
    let sa = screen_of(a);
    let sb = screen_of(b);
    let ab = sb - sa;
    let t = clamp(dot(p - sa, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
    return length(p - (sa + ab * t));
}}

fn triangle(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {{
    return min(segment(p, a, b), min(segment(p, b, c), segment(p, c, a)));
}}

fn locus_point(i: u32) -> vec2<f32> {{
    let v = placement.locus[i / 2u];
    if (i % 2u == 0u) {{
        return v.xy;
    }}
    return v.zw;
}}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {{
    let p = floor(position.xy) + vec2<f32>(0.5, 0.5);
    let r = placement.plot;
    if (!(p.x >= r.x && p.x < r.x + r.z && p.y >= r.y && p.y < r.y + r.w)) {{
        discard;
    }}
    let xy = vec2<f32>(DOMAIN.x + (p.x - r.x) / r.z * (DOMAIN.z - DOMAIN.x), DOMAIN.y + (r.y + r.w - p.y) / r.w * (DOMAIN.w - DOMAIN.y));
    var colour = PLOT;
    // A faint grid every 0.1.
    let grid = abs(fract(xy / 0.1 + vec2<f32>(0.5, 0.5)) - vec2<f32>(0.5, 0.5)) * 0.1 * r.zw / (vec2<f32>(DOMAIN.z - DOMAIN.x, DOMAIN.w - DOMAIN.y));
    if (min(grid.x, grid.y) < 0.5) {{
        colour = GRID;
    }}
    // The spectral locus, and the line of purples closing it.
    var d = 1e9;
    for (var i = 0u; i < 32u; i++) {{
        d = min(d, segment(p, locus_point(i), locus_point(i + 1u)));
    }}
    d = min(d, segment(p, locus_point(32u), locus_point(0u)));
    if (d < 0.75) {{
        colour = LOCUS;
    }}
    if (placement.info.z > 0.5 && placement.info.w > 0.5 && triangle(p, placement.timeline_a.xy, placement.timeline_a.zw, placement.timeline_b.xy) < 0.75) {{
        colour = TIMELINE;
    }}
    if (triangle(p, placement.output_a.xy, placement.output_a.zw, placement.output_b.xy) < 0.75) {{
        colour = OUTPUT;
    }}
    // The trace: the hits of every bin under this screen pixel, as phosphor-style brightness.
    let per_pixel = vec2<f32>((DOMAIN.z - DOMAIN.x) / r.z, (DOMAIN.w - DOMAIN.y) / r.w) / {BIN:?};
    let bx0 = (xy.x - DOMAIN.x) / {BIN:?} - per_pixel.x * 0.5;
    let by0 = (xy.y - DOMAIN.y) / {BIN:?} - per_pixel.y * 0.5;
    let nx = max(1u, u32(ceil(per_pixel.x)));
    let ny = max(1u, u32(ceil(per_pixel.y)));
    var hits = 0.0;
    for (var j = 0u; j < ny; j++) {{
        for (var i = 0u; i < nx; i++) {{
            let bx = i32(floor(bx0 + 0.5)) + i32(i);
            let by = i32(floor(by0 + 0.5)) + i32(j);
            if (bx >= 0 && bx < i32(BINS_X) && by >= 0 && by < i32(BINS_Y)) {{
                hits += f32(bins[u32(by) * BINS_X + u32(bx)]);
            }}
        }}
    }}
    if (hits > 0.0) {{
        let n = hits * placement.info.x * 40000.0 / max(placement.info.y, 1.0);
        let b = clamp(max(0.3, log2(1.0 + n) / log2(401.0)), 0.0, 1.0);
        colour = mix(colour, TRACE, b);
    }}
    return vec4<f32>(colour, 1.0);
}}
"#,
        d0 = DOMAIN[0],
        d1 = DOMAIN[1],
        d2 = DOMAIN[2],
        d3 = DOMAIN[3],
        BIN = BIN,
    )
}

struct State {
    bins: wgpu::Buffer,
    measure_bound: Option<(u64, wgpu::BindGroup)>,
    draw_bound: Option<wgpu::BindGroup>,
    /// Pixels of the last frame measured; zero until one has been.
    pixels: u32,
}

/// The plot's GPU state. Cheap to clone: the Feed thread measures through one clone while the window thread draws.
#[derive(Clone)]
pub struct CiePlot {
    device: wgpu::Device,
    queue: wgpu::Queue,
    measure: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    draw: wgpu::RenderPipeline,
    placement: wgpu::Buffer,
    plot: Arc<Mutex<Plot>>,
    state: Arc<Mutex<State>>,
}

impl CiePlot {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> CiePlot {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("cie measure"), source: wgpu::ShaderSource::Wgsl(measure_shader().into()) });
        let measure = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("cie measure"),
            layout: None,
            module: &shader,
            entry_point: Some("measure"),
            compilation_options: Default::default(),
            cache: None,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("cie draw"), source: wgpu::ShaderSource::Wgsl(draw_shader().into()) });
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cie draw"),
            layout: None,
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
        };
        CiePlot {
            device: device.clone(),
            queue: queue.clone(),
            measure,
            params: uniform("cie params", PARAMS_BYTES),
            draw,
            placement: uniform("cie placement", (DRAW_UNIFORM_FLOATS * 4) as u64),
            plot: Arc::new(Mutex::new(Plot::new(None, &Default::default()))),
            state: Arc::new(Mutex::new(State {
                bins: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("cie bins"),
                    size: (BIN_COUNT as u64 + 2) * 4,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                measure_bound: None,
                draw_bound: None,
                pixels: 0,
            })),
        }
    }

    pub fn set_plot(&self, plot: Plot) {
        *self.plot.lock().unwrap() = plot;
    }

    pub fn plot(&self) -> Plot {
        self.plot.lock().unwrap().clone()
    }

    /// Count `frame` into the bins (clearing last frame's counts first), recorded into `encoder`.
    pub fn measure(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let plot = self.plot();
        let m = rgb_to_xyz(&output_chromaticities(plot.output));
        let (code, gamma) = plot.encoding.code();
        let col = |i: usize| [m[0][i] as f32, m[1][i] as f32, m[2][i] as f32, 0.0];
        let mut params = [0u8; PARAMS_BYTES as usize];
        params[..16].copy_from_slice(bytemuck::cast_slice(&col(0)));
        params[16..32].copy_from_slice(bytemuck::cast_slice(&col(1)));
        params[32..48].copy_from_slice(bytemuck::cast_slice(&col(2)));
        params[48..64].copy_from_slice(bytemuck::cast_slice(&[frame.width, frame.height, code, 0]));
        params[64..80].copy_from_slice(bytemuck::cast_slice(&[gamma, 0.0, 0.0, 0.0]));
        self.queue.write_buffer(&self.params, 0, &params);
        let mut state = self.state.lock().unwrap();
        state.pixels = frame.width * frame.height;
        if state.measure_bound.as_ref().is_none_or(|(id, _)| *id != frame.texture_id) {
            let view = frame.texture.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cie measure"),
                layout: &self.measure.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: state.bins.as_entire_binding() },
                ],
            });
            state.measure_bound = Some((frame.texture_id, bind_group));
        }
        encoder.clear_buffer(&state.bins, 0, None);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("cie measure"), timestamp_writes: None });
        pass.set_pipeline(&self.measure);
        pass.set_bind_group(0, &state.measure_bound.as_ref().unwrap().1, &[]);
        pass.dispatch_workgroups(frame.width.div_ceil(8), frame.height.div_ceil(8), 1);
    }

    /// Draw the plot into `area` (left, top, width, height in pixels of a `surface`-sized target). Draws nothing
    /// until a frame has been measured.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let mut state = self.state.lock().unwrap();
        if state.pixels == 0 || area[2] < 1.0 || area[3] < 1.0 {
            return;
        }
        if state.draw_bound.is_none() {
            state.draw_bound = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cie draw"),
                layout: &self.draw.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.placement.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: state.bins.as_entire_binding() },
                ],
            }));
        }
        let plot = self.plot();
        let layout = PlotLayout::new(area, scale);
        let [left, top, width, height] = area;
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let mut u = [0f32; DRAW_UNIFORM_FLOATS];
        u[..4].copy_from_slice(&[clip(left, surface[0]), -clip(top, surface[1]), clip(left + width, surface[0]), -clip(top + height, surface[1])]);
        u[4..8].copy_from_slice(&layout.plot);
        let timeline = plot.timeline_triangle();
        let shown = if timeline.is_some() { 1.0 } else { 0.0 };
        u[8..12].copy_from_slice(&[plot.gain, state.pixels as f32, shown, shown]);
        let triangle = |t: [[f32; 2]; 3]| [t[0][0], t[0][1], t[1][0], t[1][1], t[2][0], t[2][1], 0.0, 0.0];
        u[12..20].copy_from_slice(&triangle(plot.output_triangle()));
        if let Some(t) = timeline {
            u[20..28].copy_from_slice(&triangle(t));
        }
        for (i, point) in SPECTRAL_LOCUS.iter().enumerate() {
            u[32 + i * 2..34 + i * 2].copy_from_slice(point);
        }
        self.queue.write_buffer(&self.placement, 0, bytemuck::cast_slice(&u));
        pass.set_pipeline(&self.draw);
        pass.set_bind_group(0, state.draw_bound.as_ref().unwrap(), &[]);
        pass.draw(0..4, 0..1);
    }

    /// Read the counts back from the GPU (waits for it): for tests and diagnostics, not per frame.
    pub fn counts(&self) -> CieCounts {
        let state = self.state.lock().unwrap();
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cie readback"),
            size: state.bins.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&state.bins, 0, &readback, 0, None);
        self.queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("the GPU finishes");
        let data: Vec<u32> = bytemuck::cast_slice(&readback.get_mapped_range(..).expect("mapped")).to_vec();
        CieCounts { bins: data[..BIN_COUNT].to_vec(), no_light: data[BIN_COUNT], outside: data[BIN_COUNT + 1] }
    }
}

impl crate::display::Display for CiePlot {
    fn name(&self) -> &'static str {
        "CIE chromaticity"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.measure(encoder, frame);
    }

    fn configure(&self, context: &crate::display::DisplayContext) {
        self.set_plot(Plot::new(context.colour_setup.clone(), &context.settings));
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn label(&self) -> String {
        self.plot().label()
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        CiePlot::draw(self, pass, area, surface, scale);
    }

    fn read_back(&self) -> Option<Box<dyn std::any::Any>> {
        Some(Box::new(self.counts()))
    }
}
