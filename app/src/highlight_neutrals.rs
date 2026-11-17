//! Highlight Neutrals (spec #49 stories 79-80): the Output picture in grey, with the pixels whose red, green and
//! blue are nearly equal shown in green, for checking that whites and greys are balanced.
//!
//! The grey is Luma with the weights the luma waveform uses by default (ADR 0004), so a grey here lines up with
//! its trace. A pixel is neutral when its largest and smallest channel differ by no more than the tolerance, a
//! share of the full signal range (the Output Colour Space's encoded values, 0 to 1). The tolerance is a list of
//! choices (ADR 0013), the first being the default of 3 %.
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::output_conversion::{Plan, measurement};
use crate::settings::{Choice, SettingDef};
use std::sync::{Arc, Mutex};

pub const TOLERANCE_SETTING: &str = "tolerance";

const SETTINGS: [SettingDef; 1] = [SettingDef {
    key: TOLERANCE_SETTING,
    label: "Neutral tolerance",
    choices: &[
        Choice { value: "3", text: "3 % (default)" },
        Choice { value: "0.5", text: "0.5 %" },
        Choice { value: "1", text: "1 %" },
        Choice { value: "2", text: "2 %" },
        Choice { value: "5", text: "5 %" },
        Choice { value: "10", text: "10 %" },
    ],
}];

const SHADER: &str = r#"
struct Params {
    rect: vec4<f32>,      // left, top, right, bottom in clip space
    flip: vec4<f32>,      // x > 0.5: the texture's first row is the bottom of the picture
    weights: vec4<f32>,   // luma weights for R, G, B
    tolerance: vec4<f32>, // x: the largest channel difference still neutral
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var picture: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    var out: VertexOut;
    out.position = vec4<f32>(mix(params.rect.x, params.rect.z, corner.x), mix(params.rect.y, params.rect.w, corner.y), 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, mix(corner.y, 1.0 - corner.y, step(0.5, params.flip.x)));
    return out;
}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(picture));
    let at = vec2<i32>(min(in.uv * size, size - vec2<f32>(1.0)));
    let rgb = textureLoad(picture, at, 0).rgb;
    let grey = clamp(dot(rgb, params.weights.xyz), 0.0, 1.0);
    let spread = max(max(rgb.r, rgb.g), rgb.b) - min(min(rgb.r, rgb.g), rgb.b);
    if (spread <= params.tolerance.x) {
        // Neutral: green, keeping the grey's lightness so the shape of the picture stays readable.
        return vec4<f32>(0.25 * grey, 0.4 + 0.6 * grey, 0.25 * grey, 1.0);
    }
    return vec4<f32>(grey, grey, grey, 1.0);
}
"#;

struct Picture {
    texture_id: u64,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    bottom_up: bool,
}

/// What the last [`DisplayContext`] set, for drawing and for the label.
#[derive(Clone)]
struct Config {
    weights: [f32; 3],
    /// The tolerance as the setting says it (a percentage).
    percent: String,
    tolerance: f32,
    /// How the picture was made, for the label (ADR 0007).
    source: String,
}

impl Default for Config {
    fn default() -> Config {
        Config { weights: [0.2126, 0.7152, 0.0722], percent: "3".into(), tolerance: 0.03, source: String::new() }
    }
}

/// The display's GPU state. Cheap to clone: the Feed thread takes frames through one clone while the window
/// thread draws through another.
#[derive(Clone)]
pub struct HighlightNeutrals {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    picture: Arc<Mutex<Option<Picture>>>,
    config: Arc<Mutex<Config>>,
}

impl HighlightNeutrals {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, _filterable: bool) -> HighlightNeutrals {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("highlight neutrals"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("highlight neutrals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("highlight neutrals"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("highlight neutrals"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("highlight neutrals params"), size: 64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        HighlightNeutrals { device: device.clone(), queue: queue.clone(), pipeline, layout, params, picture: Arc::new(Mutex::new(None)), config: Arc::new(Mutex::new(Config::default())) }
    }
}

impl Display for HighlightNeutrals {
    fn name(&self) -> &'static str {
        "Highlight neutrals"
    }

    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let mut picture = self.picture.lock().unwrap();
        if picture.as_ref().is_none_or(|p| p.texture_id != frame.texture_id) {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("highlight neutrals"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
                ],
            });
            *picture = Some(Picture { texture_id: frame.texture_id, bind_group, width: frame.width, height: frame.height, bottom_up: frame.bottom_up });
        }
        if let Some(p) = picture.as_mut() {
            p.bottom_up = frame.bottom_up;
        }
    }

    fn configure(&self, context: &DisplayContext) {
        let setup = context.colour_setup.as_ref().map(|c| &c.setup);
        let plan = Plan::for_setup(setup);
        let [r, g, b] = measurement(context.colour_setup.clone(), context.luma).weights();
        let percent = context.settings.get(TOLERANCE_SETTING).cloned().unwrap_or_else(|| "3".into());
        let tolerance = percent.parse::<f32>().map_or(0.03, |p| p / 100.0);
        // The picture is the Output picture only once it has been converted (ADR 0007).
        let what = if plan.method().is_some() || plan.measured_space(setup).is_some() { "Output picture" } else { "Picture as sent" };
        let source = format!("{what}  ·  {}  ·  {}", context.colour_label, plan.label());
        *self.config.lock().unwrap() = Config { weights: [r as f32, g as f32, b as f32], percent, tolerance, source };
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn label(&self) -> String {
        if self.picture.lock().unwrap().is_none() {
            return "Highlight neutrals".to_string();
        }
        let config = self.config.lock().unwrap();
        format!("Highlight neutrals  ·  green where R, G and B are within {} % of each other, the rest in Luma grey  ·  {}", config.percent, config.source)
    }

    /// Draw the picture, scaled to fit inside `area` with its shape kept.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], _scale: f32) {
        let guard = self.picture.lock().unwrap();
        let Some(picture) = guard.as_ref() else { return };
        let [left, top, width, height] = area;
        if width < 1.0 || height < 1.0 {
            return;
        }
        let fit = (width / picture.width as f32).min(height / picture.height as f32);
        let (w, h) = (picture.width as f32 * fit, picture.height as f32 * fit);
        let (x, y) = ((left + (width - w) / 2.0).round(), (top + (height - h) / 2.0).round());
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let config = self.config.lock().unwrap().clone();
        let params: [f32; 16] = [
            clip(x, surface[0]),
            -clip(y, surface[1]),
            clip(x + w.round(), surface[0]),
            -clip(y + h.round(), surface[1]),
            if picture.bottom_up { 1.0 } else { 0.0 },
            0.0,
            0.0,
            0.0,
            config.weights[0],
            config.weights[1],
            config.weights[2],
            0.0,
            config.tolerance,
            0.0,
            0.0,
            0.0,
        ];
        self.queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &picture.bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}
