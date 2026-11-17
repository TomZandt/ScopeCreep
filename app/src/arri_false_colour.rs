//! ARRI False Colour (spec #49, ticket #57): a Picture View that paints the Output picture by its Luma, in the
//! Rec.709 percentage bands third parties publish as ARRI's. Everything outside a band is shown as its Luma in
//! grey. Modelled, never claimed (ADR 0010); how the edges and the out-of-gamut channels are treated is ADR 0016.
//!
//! The Luma is the weighted sum of the Output picture's own channel values (the same number the luma waveform
//! traces, with the same weights from the Measured Space, ADR 0004), taken as a percentage of the full-range
//! signal. Nothing is clamped first: a channel below 0 counts as the negative number it is.
use crate::display::{Display, DisplayContext, ScaleText};
use crate::gpu_frame::GpuFrame;
use crate::output_conversion;
use crate::settings::{Choice, SettingDef};
use std::sync::{Arc, Mutex};

/// One band of the table: a name, its edges in percent of full-range Luma, and the colour it is shown in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub name: &'static str,
    /// Lower edge, inclusive.
    pub from: f64,
    /// Upper edge, exclusive.
    pub to: f64,
    pub colour: [f32; 3],
}

/// The bands third parties publish as ARRI's Rec.709 look (SmallHD, Sigma), from the darkest up.
pub const BANDS: [Band; 6] = [
    Band { name: "purple", from: 0.0, to: 2.5, colour: [0.55, 0.22, 0.85] },
    Band { name: "blue", from: 2.5, to: 4.0, colour: [0.15, 0.40, 1.0] },
    Band { name: "green", from: 38.0, to: 42.0, colour: [0.0, 0.72, 0.22] },
    Band { name: "pink", from: 52.0, to: 56.0, colour: [1.0, 0.45, 0.72] },
    Band { name: "yellow", from: 97.0, to: 99.0, colour: [1.0, 0.90, 0.0] },
    Band { name: "red", from: 99.0, to: 100.0, colour: [0.95, 0.0, 0.0] },
];

/// The band a Luma (percent of the full-range signal) falls in, `None` where the picture is shown as grey. A band
/// includes its lower edge and excludes its upper edge. The ends are open: below 0 % is the purple band (crushed
/// past black) and 100 % and over is the red band (clipped), because those are the pixels the view exists to flag.
pub fn band_at(percent: f64) -> Option<&'static Band> {
    let last = BANDS.len() - 1;
    BANDS.iter().enumerate().find(|(i, b)| (percent >= b.from || *i == 0) && (percent < b.to || *i == last)).map(|(_, b)| b)
}

/// The settings: only the legend, on by default.
const SETTINGS: [SettingDef; 1] = [SettingDef {
    key: "legend",
    label: "Legend",
    choices: &[Choice { value: "on", text: "On" }, Choice { value: "off", text: "Off" }],
}];

/// Height of the legend (swatches and their numbers) under the picture, in logical pixels, and of the swatches.
const LEGEND: f32 = 26.0;
const SWATCH: f32 = 8.0;
/// The legend is only drawn in a Panel at least this tall (logical pixels), so it never crowds the picture out.
const LEGEND_MIN_HEIGHT: f32 = 110.0;

/// The WGSL for the band lookup, from [`BANDS`] so there is one table.
fn band_function() -> String {
    let mut code = String::from("fn band(p: f32) -> vec4<f32> {\n");
    for (i, b) in BANDS.iter().enumerate() {
        let mut conditions = Vec::new();
        if i > 0 {
            conditions.push(format!("p >= {:?}", b.from as f32));
        }
        if i < BANDS.len() - 1 {
            conditions.push(format!("p < {:?}", b.to as f32));
        }
        let condition = if conditions.is_empty() { "true".to_string() } else { conditions.join(" && ") };
        code += &format!("    if ({condition}) {{ return vec4<f32>({:?}, {:?}, {:?}, 1.0); }}\n", b.colour[0], b.colour[1], b.colour[2]);
    }
    code += "    return vec4<f32>(0.0);\n}\n";
    code
}

fn shader() -> String {
    format!(
        r#"
struct Params {{
    rect: vec4<f32>,      // left, top, right, bottom in clip space
    weights: vec4<f32>,   // luma weights for R, G, B
    info: vec4<f32>,      // picture width, height, bottom row first (> 0.5), legend (> 0.5)
}};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var picture: texture_2d<f32>;

struct VertexOut {{
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {{
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    var out: VertexOut;
    out.position = vec4<f32>(mix(params.rect.x, params.rect.z, corner.x), mix(params.rect.y, params.rect.w, corner.y), 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, mix(corner.y, 1.0 - corner.y, step(0.5, params.info.z)));
    return out;
}}

{band}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {{
    if (params.info.w > 0.5) {{
        // The legend: six swatches side by side, a hair of black between them.
        let cell = in.uv.x * {count}.0;
        if (fract(cell) < 0.03 || fract(cell) > 0.97) {{ return vec4<f32>(0.0, 0.0, 0.0, 1.0); }}
        return vec4<f32>(swatch(u32(cell)), 1.0);
    }}
    let size = vec2<i32>(i32(params.info.x), i32(params.info.y));
    let at = clamp(vec2<i32>(in.uv * params.info.xy), vec2<i32>(0), size - vec2<i32>(1));
    let rgb = textureLoad(picture, at, 0).rgb;
    let luma = dot(rgb, params.weights.xyz);
    let coloured = band(luma * 100.0);
    if (coloured.a > 0.5) {{ return vec4<f32>(coloured.rgb, 1.0); }}
    let grey = clamp(luma, 0.0, 1.0);
    return vec4<f32>(grey, grey, grey, 1.0);
}}
"#,
        band = band_function() + &swatch_function(),
        count = BANDS.len()
    )
}

fn swatch_function() -> String {
    let mut code = String::from("fn swatch(i: u32) -> vec3<f32> {\n");
    for (i, b) in BANDS.iter().enumerate() {
        code += &format!("    if (i == {i}u) {{ return vec3<f32>({:?}, {:?}, {:?}); }}\n", b.colour[0], b.colour[1], b.colour[2]);
    }
    code += "    return vec3<f32>(0.0);\n}\n";
    code
}

/// The frame on the GPU, with the bind groups that read it.
struct Picture {
    texture_id: u64,
    picture_group: wgpu::BindGroup,
    legend_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    bottom_up: bool,
}

struct State {
    picture: Option<Picture>,
    legend: bool,
    weights: [f64; 3],
    label: String,
}

/// ARRI False Colour's GPU state. Cheap to clone: the Feed thread takes frames through one clone while the
/// window thread draws through another.
#[derive(Clone)]
pub struct ArriFalseColour {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    picture_params: wgpu::Buffer,
    legend_params: wgpu::Buffer,
    state: Arc<Mutex<State>>,
}

impl ArriFalseColour {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> ArriFalseColour {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("arri false colour"), source: wgpu::ShaderSource::Wgsl(shader().into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("arri false colour"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("arri false colour"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("arri false colour"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let buffer = |label| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: 48, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let display = ArriFalseColour {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout,
            picture_params: buffer("arri picture"),
            legend_params: buffer("arri legend"),
            state: Arc::new(Mutex::new(State { picture: None, legend: true, weights: [0.2126, 0.7152, 0.0722], label: String::new() })),
        };
        display.configure(&DisplayContext::default());
        display
    }

    fn bind_group(&self, params: &wgpu::Buffer, frame: &GpuFrame) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arri false colour"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
            ],
        })
    }

    /// The legend's height in device pixels in a Panel of `area`, when there is room for it and it is on.
    fn legend_height(legend: bool, area: [f32; 4], scale: f32) -> f32 {
        if legend && area[3] >= LEGEND_MIN_HEIGHT * scale { LEGEND * scale } else { 0.0 }
    }
}

impl Display for ArriFalseColour {
    fn name(&self) -> &'static str {
        "ARRI False Colour"
    }

    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let mut state = self.state.lock().unwrap();
        if state.picture.as_ref().is_none_or(|p| p.texture_id != frame.texture_id) {
            state.picture = Some(Picture {
                texture_id: frame.texture_id,
                picture_group: self.bind_group(&self.picture_params, frame),
                legend_group: self.bind_group(&self.legend_params, frame),
                width: frame.width,
                height: frame.height,
                bottom_up: frame.bottom_up,
            });
        }
        if let Some(p) = state.picture.as_mut() {
            p.bottom_up = frame.bottom_up;
        }
    }

    fn configure(&self, context: &DisplayContext) {
        // The Output picture's Luma, weighted as the luma waveform weights it (ADR 0004): Rec.709 is assumed, and
        // the label says so, until the Colour Setup is known and the picture is converted.
        let measurement = output_conversion::measurement(context.colour_setup.clone(), context.luma);
        let weights = measurement.weights();
        let space = if measurement.assumed { format!("{} assumed, unconverted", measurement.space.name()) } else { measurement.space.name().to_string() };
        let mut state = self.state.lock().unwrap();
        state.weights = weights;
        state.legend = context.settings.get("legend").is_none_or(|v| v == "on");
        state.label = format!(
            "ARRI-style False Colour  ·  Output Luma, {space}  ·  Y = {:.4} R + {:.4} G + {:.4} B  ·  Rec.709 percentage bands published by third parties; not ARRI's specification, not affiliated with ARRI",
            weights[0], weights[1], weights[2]
        );
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn label(&self) -> String {
        self.state.lock().unwrap().label.clone()
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let state = self.state.lock().unwrap();
        let Some(picture) = state.picture.as_ref() else { return };
        let [left, top, width, height] = area;
        let strip = ArriFalseColour::legend_height(state.legend, area, scale);
        let room = height - strip;
        if width < 1.0 || room < 1.0 {
            return;
        }
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let params = |rect: [f32; 4], legend: bool| -> [f32; 12] {
            [
                clip(rect[0], surface[0]),
                -clip(rect[1], surface[1]),
                clip(rect[0] + rect[2], surface[0]),
                -clip(rect[1] + rect[3], surface[1]),
                state.weights[0] as f32,
                state.weights[1] as f32,
                state.weights[2] as f32,
                0.0,
                picture.width as f32,
                picture.height as f32,
                if picture.bottom_up { 1.0 } else { 0.0 },
                if legend { 1.0 } else { 0.0 },
            ]
        };
        // The picture, scaled to fit above the legend with its shape kept.
        let fit = (width / picture.width as f32).min(room / picture.height as f32);
        let (w, h) = ((picture.width as f32 * fit).round(), (picture.height as f32 * fit).round());
        let (x, y) = ((left + (width - w) / 2.0).round(), (top + (room - h) / 2.0).round());
        self.queue.write_buffer(&self.picture_params, 0, bytemuck::cast_slice(&params([x, y, w, h], false)));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &picture.picture_group, &[]);
        pass.draw(0..4, 0..1);
        if strip > 0.0 {
            let swatches = [left, top + height - strip, width, SWATCH * scale];
            self.queue.write_buffer(&self.legend_params, 0, bytemuck::cast_slice(&params(swatches, true)));
            pass.set_bind_group(0, &picture.legend_group, &[]);
            pass.draw(0..4, 0..1);
        }
    }

    /// The band edges under their swatches, where they fit.
    fn scale_texts(&self, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        let state = self.state.lock().unwrap();
        if state.picture.is_none() {
            return Vec::new();
        }
        let strip = ArriFalseColour::legend_height(state.legend, area, scale);
        let cell = area[2] / BANDS.len() as f32;
        // "97–99" at the scale text size needs about this much room.
        if strip == 0.0 || cell < 34.0 * scale {
            return Vec::new();
        }
        let top = area[1] + area[3] - strip + (SWATCH + (LEGEND - SWATCH) / 2.0) * scale;
        BANDS
            .iter()
            .enumerate()
            .map(|(i, b)| ScaleText { text: format!("{}–{}", b.from, b.to), left: area[0] + i as f32 * cell, top, width: cell, right_aligned: false })
            .collect()
    }
}
