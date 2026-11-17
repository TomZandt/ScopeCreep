//! The saturation heatmap (spec #49 stories 75-78, ticket #64): a Picture View that paints each pixel of the
//! Output picture from cool (neutral) to warm (very saturated), in the shared muted palette.
//!
//! Saturation is the distance from neutral on the vectorscope's scale: Cb/Cr from the Measured Space's luma
//! weights (ADR 0004), each running -0.5 to 0.5, and the distance `sqrt(Cb^2 + Cr^2)` as a share of the
//! vectorscope's outer ring ([`crate::vectorscope::RING`], 0.6). So 100 % is the vectorscope's 100 % ring, near
//! where 100 % colour bars sit (ADR 0023), and a grey is 0 %. The scale setting is the percentage at the warm
//! end; more saturated pixels stay at the warm end.
use crate::display::{Display, DisplayContext, ScaleText};
use crate::gpu_frame::GpuFrame;
use crate::muted_palette;
use crate::output_conversion;
use crate::settings::{Choice, SettingDef};
use crate::vectorscope::RING;
use std::sync::{Arc, Mutex};

pub const SCALE_SETTING: &str = "scale";
pub const LEGEND_SETTING: &str = "legend";

const SETTINGS: [SettingDef; 2] = [
    SettingDef {
        key: SCALE_SETTING,
        label: "Scale (warm end)",
        choices: &[
            Choice { value: "100", text: "100 % (default)" },
            Choice { value: "25", text: "25 %" },
            Choice { value: "50", text: "50 %" },
            Choice { value: "75", text: "75 %" },
            Choice { value: "150", text: "150 %" },
            Choice { value: "200", text: "200 %" },
        ],
    },
    SettingDef { key: LEGEND_SETTING, label: "Legend", choices: &[Choice { value: "on", text: "On" }, Choice { value: "off", text: "Off" }] },
];

/// Legend strip height and its colour bar's height, in logical pixels; only drawn in a Panel this tall.
const LEGEND: f32 = 26.0;
const BAR: f32 = 8.0;
const LEGEND_MIN_HEIGHT: f32 = 110.0;

/// Saturation as a percentage of the vectorscope's 100 % ring, for an encoded `rgb` and luma `weights`.
pub fn saturation_percent(weights: [f64; 3], rgb: [f64; 3]) -> f64 {
    let [cb, cr] = crate::vectorscope::chroma(weights, rgb);
    cb.hypot(cr) / RING * 100.0
}

fn shader() -> String {
    format!(
        r#"
struct Params {{
    rect: vec4<f32>,      // left, top, right, bottom in clip space
    weights: vec4<f32>,   // luma weights for R, G, B; w: 1 / (scale in Cb/Cr)
    info: vec4<f32>,      // picture width, height, bottom row first (> 0.5), legend bar (> 0.5)
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

{palette}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {{
    if (params.info.w > 0.5) {{ return vec4<f32>(muted(in.uv.x), 1.0); }}
    let size = vec2<i32>(i32(params.info.x), i32(params.info.y));
    let at = clamp(vec2<i32>(in.uv * params.info.xy), vec2<i32>(0), size - vec2<i32>(1));
    let rgb = textureLoad(picture, at, 0).rgb;
    let w = params.weights.xyz;
    let y = dot(rgb, w);
    let cb = (rgb.b - y) / (2.0 * (1.0 - w.z));
    let cr = (rgb.r - y) / (2.0 * (1.0 - w.x));
    return vec4<f32>(muted(length(vec2<f32>(cb, cr)) * params.weights.w), 1.0);
}}
"#,
        palette = muted_palette::wgsl()
    )
}

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
    /// The percentage at the warm end.
    scale: f64,
    weights: [f64; 3],
    label: String,
}

/// The heatmap's GPU state. Cheap to clone: the Feed thread takes frames through one clone while the window
/// thread draws through another.
#[derive(Clone)]
pub struct SaturationHeatmap {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    picture_params: wgpu::Buffer,
    legend_params: wgpu::Buffer,
    state: Arc<Mutex<State>>,
}

impl SaturationHeatmap {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> SaturationHeatmap {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("saturation heatmap"), source: wgpu::ShaderSource::Wgsl(shader().into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("saturation heatmap"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("saturation heatmap"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("saturation heatmap"),
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
        let display = SaturationHeatmap {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout,
            picture_params: buffer("heatmap picture"),
            legend_params: buffer("heatmap legend"),
            state: Arc::new(Mutex::new(State { picture: None, legend: true, scale: 100.0, weights: [0.2126, 0.7152, 0.0722], label: String::new() })),
        };
        display.configure(&DisplayContext::default());
        display
    }

    fn bind_group(&self, params: &wgpu::Buffer, frame: &GpuFrame) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("saturation heatmap"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
            ],
        })
    }

    fn legend_height(legend: bool, area: [f32; 4], scale: f32) -> f32 {
        if legend && area[3] >= LEGEND_MIN_HEIGHT * scale { LEGEND * scale } else { 0.0 }
    }
}

impl Display for SaturationHeatmap {
    fn name(&self) -> &'static str {
        "Saturation heatmap"
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
        let measurement = output_conversion::measurement(context.colour_setup.clone(), context.luma);
        let weights = measurement.weights();
        let space = if measurement.assumed { format!("{} assumed, unconverted", measurement.space.name()) } else { measurement.space.name().to_string() };
        let scale = context.settings.get(SCALE_SETTING).and_then(|v| v.parse::<f64>().ok()).unwrap_or(100.0);
        let mut state = self.state.lock().unwrap();
        state.weights = weights;
        state.scale = scale;
        state.legend = context.settings.get(LEGEND_SETTING).is_none_or(|v| v == "on");
        state.label = format!(
            "Saturation heatmap  ·  Output picture, {space}  ·  distance from neutral in Cb/Cr (Y = {:.4} R + {:.4} G + {:.4} B), 100 % = the vectorscope's 100 % ring  ·  cool 0 % to warm {scale} %",
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
        let strip = SaturationHeatmap::legend_height(state.legend, area, scale);
        let room = height - strip;
        if width < 1.0 || room < 1.0 {
            return;
        }
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let gain = (100.0 / (state.scale * RING)) as f32;
        let params = |rect: [f32; 4], legend: bool| -> [f32; 12] {
            [
                clip(rect[0], surface[0]),
                -clip(rect[1], surface[1]),
                clip(rect[0] + rect[2], surface[0]),
                -clip(rect[1] + rect[3], surface[1]),
                state.weights[0] as f32,
                state.weights[1] as f32,
                state.weights[2] as f32,
                gain,
                picture.width as f32,
                picture.height as f32,
                if picture.bottom_up { 1.0 } else { 0.0 },
                if legend { 1.0 } else { 0.0 },
            ]
        };
        let fit = (width / picture.width as f32).min(room / picture.height as f32);
        let (w, h) = ((picture.width as f32 * fit).round(), (picture.height as f32 * fit).round());
        let (x, y) = ((left + (width - w) / 2.0).round(), (top + (room - h) / 2.0).round());
        self.queue.write_buffer(&self.picture_params, 0, bytemuck::cast_slice(&params([x, y, w, h], false)));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &picture.picture_group, &[]);
        pass.draw(0..4, 0..1);
        if strip > 0.0 {
            self.queue.write_buffer(&self.legend_params, 0, bytemuck::cast_slice(&params([left, top + height - strip, width, BAR * scale], true)));
            pass.set_bind_group(0, &picture.legend_group, &[]);
            pass.draw(0..4, 0..1);
        }
    }

    /// The two ends of the legend: 0 % under the cool end and the scale's top under the warm end.
    fn scale_texts(&self, area: [f32; 4], scale: f32) -> Vec<ScaleText> {
        let state = self.state.lock().unwrap();
        let strip = SaturationHeatmap::legend_height(state.legend, area, scale);
        if state.picture.is_none() || strip == 0.0 || area[2] < 80.0 * scale {
            return Vec::new();
        }
        let top = area[1] + area[3] - strip + (BAR + (LEGEND - BAR) / 2.0) * scale;
        let width = area[2] / 2.0;
        vec![
            ScaleText { text: "0 %".into(), left: area[0], top, width, right_aligned: false },
            ScaleText { text: format!("{} %", state.scale), left: area[0] + width, top, width, right_aligned: true },
        ]
    }
}
