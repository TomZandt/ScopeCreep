//! Stops False Colour: a Picture View that paints each pixel by how many stops it is above or below middle grey,
//! measured in scene light (ADR 0009, ADR 0010).
//!
//! The Feed's Timeline Colour Space frames (after the grade) are decoded to linear light with that space's own
//! curve; luma is the Y of the space's primaries; middle grey is 0.18 linear in every scene-referred space, so
//! stops = log2(Y / 0.18). The view ignores the Measured Space switch: it always needs scene light. A display-
//! referred Timeline Colour Space (Rec.709 Gamma 2.4) has no scene light, so its display curve is inverted and the
//! label says it is approximated. A space with no curve here is labelled and not drawn, never guessed (ADR 0007).
//!
//! Nothing is measured ahead of drawing: the Panel keeps the latest Timeline Colour Space frame on the GPU and
//! decodes it as it draws, so the Colour Setup and the settings always apply to the frame on screen.
//! [`Display::read_back`] runs the same decoding into a stops map, for tests.
//!
//! Two modes: full zones (whole stops out to six either side of grey, with half-stop zones beside grey) and grey
//! and +1 stop (a black and white picture, green within a sixth of a stop of grey, pink within a sixth of a stop
//! of one stop over: the ColorChecker routine).
use crate::colour_setup::TimelineSpace;
use crate::display::{Display, DisplayContext, ScaleText};
use crate::gpu_frame::GpuFrame;
use crate::settings::{Choice, SettingDef};
use scopecreep_accuracy::colorchecker::luma_weights_adapted_to_d65;
use scopecreep_accuracy::colour::{AP1, DWG, REC709};
use std::sync::{Arc, Mutex};

/// The display's name, which its settings are kept under.
pub const NAME: &str = "Stops False Colour";
pub const MODE_SETTING: &str = "mode";
pub const LEGEND_SETTING: &str = "legend";

/// Middle grey, linear.
pub const MIDDLE_GREY: f64 = 0.18;

/// The edges between the 15 zones, in stops from middle grey: whole stops out to six, half-stop zones either side
/// of the grey zone (which is one stop wide, half a stop each way).
pub const ZONE_EDGES: [f32; 14] = [-6.0, -5.0, -4.0, -3.0, -2.0, -1.0, -0.5, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

/// The colour of each zone, from the darkest (more than six stops under) to the brightest (more than six over).
/// Cool for under, grey for middle grey, warm for over; every zone its own colour.
pub const ZONE_COLOURS: [[f32; 3]; 15] = [
    [0.18, 0.00, 0.32],
    [0.36, 0.00, 0.62],
    [0.10, 0.12, 0.78],
    [0.00, 0.38, 0.88],
    [0.00, 0.68, 0.82],
    [0.00, 0.55, 0.42],
    [0.30, 0.72, 0.40],
    [0.50, 0.50, 0.50],
    [0.96, 0.58, 0.72],
    [0.96, 0.86, 0.18],
    [0.96, 0.60, 0.08],
    [0.90, 0.36, 0.04],
    [0.84, 0.10, 0.10],
    [0.58, 0.00, 0.20],
    [1.00, 1.00, 1.00],
];

/// The two colours of the grey and +1 stop mode, and how close (in stops) a pixel must be.
pub const GREY_GREEN: [f32; 3] = [0.0, 0.85, 0.25];
pub const PLUS_ONE_PINK: [f32; 3] = [1.0, 0.40, 0.70];
pub const CHART_TOLERANCE: f32 = 1.0 / 6.0;

/// How a Timeline Colour Space is decoded to linear light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    AcesCct,
    AcesCc,
    AcesCg,
    DaVinciIntermediate,
    /// Rec.709 Gamma 2.4: a display curve, inverted. Not scene light.
    Gamma24,
}

impl Curve {
    /// The curve for a Timeline Colour Space, `None` for one this view cannot decode.
    pub fn for_space(space: &TimelineSpace) -> Option<Curve> {
        Some(match space {
            TimelineSpace::Acescct => Curve::AcesCct,
            TimelineSpace::Acescc => Curve::AcesCc,
            TimelineSpace::DaVinciWideGamut => Curve::DaVinciIntermediate,
            TimelineSpace::Rec709Gamma24 => Curve::Gamma24,
            TimelineSpace::Other(name) if name == "ACEScg" => Curve::AcesCg,
            TimelineSpace::Other(_) => return None,
        })
    }

    fn code(self) -> u32 {
        match self {
            Curve::AcesCct => 0,
            Curve::AcesCc => 1,
            Curve::DaVinciIntermediate => 2,
            Curve::Gamma24 => 3,
            Curve::AcesCg => 4,
        }
    }

    /// The Y of the space's primaries, as weights for linear RGB, for the colour as seen on a D65 display. A D65
    /// space's weights are the usual luma weights; ACES's white is not D65, so its weights are adapted to it
    /// (neutrals are unchanged).
    pub fn luma_weights(self) -> [f64; 3] {
        match self {
            Curve::AcesCct | Curve::AcesCc | Curve::AcesCg => luma_weights_adapted_to_d65(&AP1),
            Curve::DaVinciIntermediate => luma_weights_adapted_to_d65(&DWG),
            Curve::Gamma24 => luma_weights_adapted_to_d65(&REC709),
        }
    }

    /// Whether the stops are only approximate: a display-referred space has no scene light to decode.
    pub fn approximated(self) -> bool {
        self == Curve::Gamma24
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Zones,
    GreyAndPlusOne,
}

const SETTINGS: [SettingDef; 2] = [
    SettingDef {
        key: MODE_SETTING,
        label: "Mode",
        choices: &[Choice { value: "zones", text: "Full zones" }, Choice { value: "grey-plus-one", text: "Grey and +1 stop" }],
    },
    SettingDef { key: LEGEND_SETTING, label: "Legend", choices: &[Choice { value: "on", text: "Legend on" }, Choice { value: "off", text: "Legend off" }] },
];

/// What a Stops False Colour read back from the GPU: the stops of every pixel (log2 of Y over middle grey).
pub struct StopsMap {
    pub width: u32,
    pub height: u32,
    pub bottom_up: bool,
    /// In the frame's own row order.
    pub stops: Vec<f32>,
}

impl StopsMap {
    /// The stops of the pixel at `x` across and `y` down from the top of the picture.
    pub fn at(&self, x: u32, y: u32) -> f32 {
        let row = if self.bottom_up { self.height - 1 - y } else { y };
        self.stops[(row * self.width + x) as usize]
    }
}

/// Logical pixels: the legend's swatches.
const SWATCH_WIDTH: f32 = 10.0;
const SWATCH_ROW: f32 = 18.0;
const SWATCH_GAP: f32 = 4.0;
/// Logical pixels: the numbers' size, as the waveform's.
const TEXT_LINE: f32 = crate::waveform::SCALE_SIZE * 1.3;

/// Where the picture and the legend go in a Panel.
struct Geometry {
    /// Left, top, width, height in device pixels.
    picture: [f32; 4],
    legend: [f32; 4],
    rows: usize,
    /// Where the numbers go: left and width.
    text: (f32, f32),
}

impl Geometry {
    fn new(area: [f32; 4], scale: f32, mode: Mode, legend_on: bool, picture_size: [u32; 2]) -> Geometry {
        let [left, top, width, height] = area;
        let rows = if mode == Mode::Zones { ZONE_COLOURS.len() } else { 2 };
        let gutter = if legend_on { (crate::layout::GUTTER * scale).round().min(width * 0.5) } else { 0.0 };
        let (inner_left, inner_width) = (left + gutter, (width - gutter).max(1.0));
        let fit = (inner_width / picture_size[0].max(1) as f32).min(height / picture_size[1].max(1) as f32);
        let (w, h) = ((picture_size[0] as f32 * fit).round(), (picture_size[1] as f32 * fit).round());
        let picture = [(inner_left + (inner_width - w) / 2.0).round(), (top + (height - h) / 2.0).round(), w, h];
        let legend = if legend_on {
            let row = (SWATCH_ROW * scale).min((height - 2.0 * SWATCH_GAP * scale) / rows as f32).max(1.0);
            let total = row * rows as f32;
            [left + gutter - (SWATCH_WIDTH + SWATCH_GAP) * scale, top + (height - total) / 2.0, SWATCH_WIDTH * scale, total]
        } else {
            [0.0; 4]
        };
        Geometry { picture, legend, rows, text: (left, (gutter - (SWATCH_WIDTH + 2.0 * SWATCH_GAP) * scale).max(1.0)) }
    }
}

fn wgsl_vec3(c: [f32; 3]) -> String {
    format!("vec3<f32>({:.4}, {:.4}, {:.4})", c[0], c[1], c[2])
}

/// The decoding and the zones, shared by drawing and by the read-back.
fn common_shader() -> String {
    let edges = ZONE_EDGES.iter().map(|e| format!("{e:.3}")).collect::<Vec<_>>().join(", ");
    let colours = ZONE_COLOURS.iter().map(|c| wgsl_vec3(*c)).collect::<Vec<_>>().join(", ");
    format!(
        r#"
struct Params {{
    weights: vec4<f32>,     // the Y weights of the Timeline Colour Space's primaries
    info: vec4<u32>,        // curve, picture width, picture height, mode (0 full zones, 1 grey and +1 stop)
    clip: vec4<f32>,        // the Panel in clip space: left, top, right, bottom
    picture: vec4<f32>,     // the picture on screen, in pixels: left, top, width, height
    legend: vec4<f32>,      // the legend's swatches, in pixels: left, top, width, height (width 0: none)
    flags: vec4<f32>,       // x: the frame's first row is the bottom of the picture; y: legend rows
}};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var frame: texture_2d<f32>;

var<private> EDGES: array<f32, 14> = array<f32, 14>({edges});
var<private> COLOURS: array<vec3<f32>, 15> = array<vec3<f32>, 15>({colours});
const GREEN: vec3<f32> = {green};
const PINK: vec3<f32> = {pink};
const TOLERANCE: f32 = {tolerance:.6};
const MIDDLE_GREY: f32 = {middle_grey};

fn decode(v: f32, curve: u32) -> f32 {{
    switch curve {{
        case 0u: {{
            if v <= 0.155251141552511 {{ return (v - 0.0729055341958355) / 10.5402377416545; }}
            return exp2(v * 17.52 - 9.72);
        }}
        case 1u: {{
            if v < -0.3013698630 {{ return (exp2(v * 17.52 - 9.72) - exp2(-16.0)) * 2.0; }}
            return exp2(min(v, 1.4679) * 17.52 - 9.72);
        }}
        case 2u: {{
            if v > 0.02740668 {{ return exp2(v / 0.07329248 - 7.0) - 0.0075; }}
            return v / 10.44426855;
        }}
        case 3u: {{ return sign(v) * pow(abs(v), 2.4); }}
        default: {{ return v; }}
    }}
}}

// Linear Y of a Timeline Colour Space pixel.
fn luma(texel: vec4<f32>) -> f32 {{
    let c = params.info.x;
    return dot(params.weights.xyz, vec3<f32>(decode(texel.r, c), decode(texel.g, c), decode(texel.b, c)));
}}

fn stops_of(y: f32) -> f32 {{
    return log2(max(y, 1e-10) / MIDDLE_GREY);
}}
"#,
        green = wgsl_vec3(GREY_GREEN),
        pink = wgsl_vec3(PLUS_ONE_PINK),
        tolerance = CHART_TOLERANCE,
        middle_grey = MIDDLE_GREY,
    )
}

fn measure_shader() -> String {
    common_shader()
        + r#"
@group(0) @binding(2) var stops_out: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn measure(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= params.info.y || id.y >= params.info.z { return; }
    let texel = textureLoad(frame, vec2<i32>(id.xy), 0);
    textureStore(stops_out, vec2<i32>(id.xy), vec4<f32>(stops_of(luma(texel)), 0.0, 0.0, 1.0));
}
"#
}

fn draw_shader() -> String {
    common_shader()
        + r#"
@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    return vec4<f32>(mix(params.clip.x, params.clip.z, corner.x), mix(params.clip.y, params.clip.w, corner.y), 0.0, 1.0);
}

fn inside(p: vec2<f32>, r: vec4<f32>) -> bool {
    return p.x >= r.x && p.x < r.x + r.z && p.y >= r.y && p.y < r.y + r.w;
}

fn zone_colour(stops: f32) -> vec3<f32> {
    var zone = 0u;
    for (var i = 0u; i < 14u; i++) {
        if stops >= EDGES[i] { zone = i + 1u; }
    }
    return COLOURS[zone];
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let p = position.xy;
    let chart = params.info.w == 1u;
    if params.legend.z > 0.0 && inside(p, params.legend) {
        let rows = u32(params.flags.y);
        let row = min(u32((p.y - params.legend.y) / params.legend.w * f32(rows)), rows - 1u);
        if chart {
            return vec4<f32>(select(GREEN, PINK, row == 0u), 1.0);
        }
        return vec4<f32>(COLOURS[rows - 1u - row], 1.0);
    }
    if !inside(p, params.picture) { discard; }
    let uv = (p - params.picture.xy) / params.picture.zw;
    let x = min(u32(uv.x * f32(params.info.y)), params.info.y - 1u);
    let y = min(u32(uv.y * f32(params.info.z)), params.info.z - 1u);
    let row = select(y, params.info.z - 1u - y, params.flags.x > 0.5);
    let y_linear = luma(textureLoad(frame, vec2<i32>(vec2<u32>(x, row)), 0));
    let stops = stops_of(y_linear);
    if chart {
        if abs(stops) < TOLERANCE { return vec4<f32>(GREEN, 1.0); }
        if abs(stops - 1.0) < TOLERANCE { return vec4<f32>(PINK, 1.0); }
        let grey = pow(clamp(y_linear, 0.0, 1.0), 1.0 / 2.4);
        return vec4<f32>(grey, grey, grey, 1.0);
    }
    return vec4<f32>(zone_colour(stops), 1.0);
}
"#
}

const PARAMS_BYTES: u64 = 96;

/// What the Colour Setup and the settings made of the display.
struct Config {
    curve: Option<Curve>,
    mode: Mode,
    legend: bool,
    /// The Colour Setup's part of the label.
    colour_label: String,
    timeline: String,
}

struct State {
    frame: Option<GpuFrame>,
    /// The bind group of the draw, for the frame's texture.
    bound: Option<(u64, wgpu::BindGroup)>,
}

/// Stops False Colour. Cheap to clone; the Feed thread hands it frames while the window thread draws.
#[derive(Clone)]
pub struct StopsFalseColour {
    device: wgpu::Device,
    queue: wgpu::Queue,
    draw: wgpu::RenderPipeline,
    measure: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    config: Arc<Mutex<Config>>,
    state: Arc<Mutex<State>>,
}

impl StopsFalseColour {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> StopsFalseColour {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("stops false colour draw"), source: wgpu::ShaderSource::Wgsl(draw_shader().into()) });
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stops false colour draw"),
            layout: None,
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("stops false colour measure"), source: wgpu::ShaderSource::Wgsl(measure_shader().into()) });
        let measure = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("stops false colour measure"),
            layout: None,
            module: &module,
            entry_point: Some("measure"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("stops false colour params"), size: PARAMS_BYTES, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let view = StopsFalseColour {
            device: device.clone(),
            queue: queue.clone(),
            draw,
            measure,
            params,
            config: Arc::new(Mutex::new(Config { curve: None, mode: Mode::Zones, legend: true, colour_label: String::new(), timeline: String::new() })),
            state: Arc::new(Mutex::new(State { frame: None, bound: None })),
        };
        view.configure(&DisplayContext::default());
        view
    }

    fn write_params(&self, frame: &GpuFrame, area: Option<([f32; 4], [f32; 2], f32)>) -> bool {
        let config = self.config.lock().unwrap();
        let Some(curve) = config.curve else { return false };
        let weights = curve.luma_weights();
        let mut words = [0u8; PARAMS_BYTES as usize];
        words[..16].copy_from_slice(bytemuck::cast_slice(&[weights[0] as f32, weights[1] as f32, weights[2] as f32, 0.0]));
        words[16..32].copy_from_slice(bytemuck::cast_slice(&[curve.code(), frame.width, frame.height, (config.mode == Mode::GreyAndPlusOne) as u32]));
        if let Some((area, surface, scale)) = area {
            let geometry = Geometry::new(area, scale, config.mode, config.legend, [frame.width, frame.height]);
            let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
            let [left, top, width, height] = area;
            words[32..48].copy_from_slice(bytemuck::cast_slice(&[clip(left, surface[0]), -clip(top, surface[1]), clip(left + width, surface[0]), -clip(top + height, surface[1])]));
            words[48..64].copy_from_slice(bytemuck::cast_slice(&geometry.picture));
            words[64..80].copy_from_slice(bytemuck::cast_slice(&geometry.legend));
            words[80..96].copy_from_slice(bytemuck::cast_slice(&[frame.bottom_up as u32 as f32, geometry.rows as f32, 0.0, 0.0]));
        }
        self.queue.write_buffer(&self.params, 0, &words);
        true
    }

    /// What the display measures, for tests and diagnostics: the stops of every pixel of the latest frame, decoded
    /// now with the Colour Setup in use (waits for the GPU).
    pub fn stops(&self) -> Option<StopsMap> {
        let state = self.state.lock().unwrap();
        let frame = state.frame.as_ref()?;
        if !self.write_params(frame, None) {
            return None;
        }
        let (width, height) = (frame.width, frame.height);
        let out = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stops map"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stops measure"),
            layout: &self.measure.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&out.create_view(&Default::default())) },
            ],
        });
        let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("stops readback"), size: (row * height) as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.measure);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        encoder.copy_texture_to_buffer(
            out.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("the GPU finishes");
        let mapped = readback.get_mapped_range(..).expect("mapped");
        let stops = (0..height as usize).flat_map(|y| bytemuck::cast_slice::<u8, f32>(&mapped[y * row as usize..y * row as usize + width as usize * 4]).to_vec()).collect();
        Some(StopsMap { width, height, bottom_up: frame.bottom_up, stops })
    }
}

impl Display for StopsFalseColour {
    fn name(&self) -> &'static str {
        NAME
    }

    /// Stops are measured on the Timeline Colour Space frame, not the converted one.
    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, _frame: &GpuFrame) {}

    fn on_timeline_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.state.lock().unwrap().frame = Some(frame.clone());
    }

    fn configure(&self, context: &DisplayContext) {
        let mut config = self.config.lock().unwrap();
        config.curve = context.colour_setup.as_ref().and_then(|c| Curve::for_space(&c.setup.timeline_space));
        config.timeline = context.colour_setup.as_ref().map(|c| c.setup.timeline.clone()).unwrap_or_default();
        config.colour_label = context.colour_label.clone();
        config.mode = if context.settings.get(MODE_SETTING).map(String::as_str) == Some("grey-plus-one") { Mode::GreyAndPlusOne } else { Mode::Zones };
        config.legend = context.settings.get(LEGEND_SETTING).map(String::as_str) != Some("off");
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    /// What is shown and how far it can be trusted: scene light decoded from the Timeline Colour Space, or the
    /// reason nothing is measured.
    fn label(&self) -> String {
        let config = self.config.lock().unwrap();
        let mode = if config.mode == Mode::Zones { "full zones" } else { "grey and +1 stop" };
        let basis = match (config.curve, config.timeline.as_str()) {
            (Some(curve), _) if curve.approximated() => format!("stops from 18 % grey, approximated: {} is a display curve, inverted, not scene light", config.timeline),
            (Some(_), timeline) => format!("stops from 18 % grey, scene light decoded from {timeline}"),
            (None, "") => "not measured: the Timeline Colour Space is not known".to_string(),
            (None, timeline) => format!("not measured: cannot decode {timeline}"),
        };
        format!("{NAME}  ·  {mode}  ·  {basis}  ·  {}", config.colour_label)
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let mut state = self.state.lock().unwrap();
        let Some(frame) = state.frame.clone() else { return };
        if area[2] < 1.0 || area[3] < 1.0 || !self.write_params(&frame, Some((area, surface, scale))) {
            return;
        }
        if state.bound.as_ref().is_none_or(|(id, _)| *id != frame.texture_id) {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("stops false colour draw"),
                layout: &self.draw.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
                ],
            });
            state.bound = Some((frame.texture_id, bind_group));
        }
        pass.set_pipeline(&self.draw);
        pass.set_bind_group(0, &state.bound.as_ref().unwrap().1, &[]);
        pass.draw(0..4, 0..1);
    }

    /// The legend's numbers: the stops at the edge of each zone, or in the grey and +1 stop mode the two colours'.
    fn scale_texts(&self, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        // Lock the state before the config, as everywhere else.
        let state = self.state.lock().unwrap();
        let config = self.config.lock().unwrap();
        let Some(frame) = state.frame.as_ref() else { return Vec::new() };
        if config.curve.is_none() || !config.legend {
            return Vec::new();
        }
        let g = Geometry::new(area, scale, config.mode, true, [frame.width, frame.height]);
        let row = g.legend[3] / g.rows as f32;
        let marks: Vec<(String, f32)> = if config.mode == Mode::GreyAndPlusOne {
            vec![("+1".to_string(), g.legend[1] + row * 0.5), ("0".to_string(), g.legend[1] + row * 1.5)]
        } else {
            // Edge i sits between the zone above and the zone below it.
            ZONE_EDGES.iter().enumerate().map(|(i, e)| (if *e > 0.0 { format!("+{e}") } else { format!("{e}") }, g.legend[1] + row * (ZONE_EDGES.len() - i) as f32)).collect()
        };
        let placed: Vec<(f32, u32)> = marks.iter().map(|(text, y)| (*y, if text == "0" || text == "-1" || text == "+1" { 0 } else { 1 })).collect();
        let shown = crate::waveform::marks_that_fit(&placed, TEXT_LINE * scale);
        marks.into_iter().zip(shown).filter(|(_, shown)| *shown).map(|((text, top), _)| ScaleText { text, left: g.text.0, top, width: g.text.1, right_aligned: true }).collect()
    }

    fn read_back(&self) -> Option<Box<dyn std::any::Any>> {
        self.stops().map(|m| Box::new(m) as Box<dyn std::any::Any>)
    }
}
