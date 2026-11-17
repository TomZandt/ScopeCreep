//! The black-and-white Picture View: the Output picture with each pixel's grey equal to its Luma, using the same
//! weights as the waveform (the Measured Space's, ADR 0004), so a grey you see lines up exactly with its trace.
//!
//! The weights come from [`Measurement`], the waveform's own type, built from the same Colour Setup and the luma
//! weights chosen on the waveform (`DisplayContext::luma`). The grey is the luma value 0 to 1 itself: the display
//! surface is not sRGB-encoded, so it goes to the screen as it is, like the clean picture's values.
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::waveform::Measurement;
use std::sync::{Arc, Mutex};

const SHADER: &str = r#"
struct Placement {
    rect: vec4<f32>,      // left, top, right, bottom in clip space
    flip: vec4<f32>,      // x > 0.5: the texture's first row is the bottom of the picture
    weights: vec4<f32>,   // luma weights for R, G, B
};
@group(0) @binding(0) var<uniform> placement: Placement;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    var out: VertexOut;
    out.position = vec4<f32>(mix(placement.rect.x, placement.rect.z, corner.x), mix(placement.rect.y, placement.rect.w, corner.y), 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, mix(corner.y, 1.0 - corner.y, step(0.5, placement.flip.x)));
    return out;
}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    let y = dot(textureSampleLevel(picture, picture_sampler, in.uv, 0.0).rgb, placement.weights.xyz);
    return vec4<f32>(y, y, y, 1.0);
}
"#;

/// The frame on the GPU.
struct Picture {
    texture_id: u64,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    bottom_up: bool,
}

/// The black-and-white Panel's GPU state. Cheap to clone, shared between the Feed and window threads.
#[derive(Clone)]
pub struct BlackAndWhite {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    placement: wgpu::Buffer,
    picture: Arc<Mutex<Option<Picture>>>,
    /// What Luma means for the picture: the waveform's own measurement, so the two cannot disagree.
    measurement: Arc<Mutex<Measurement>>,
}

impl BlackAndWhite {
    /// `filterable`: the device can smooth 32-bit float textures (else the picture is drawn nearest-neighbour).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool) -> BlackAndWhite {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("black and white"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("black and white"),
            entries: &[
                entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                entry(
                    1,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                ),
                entry(
                    2,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Sampler(if filterable { wgpu::SamplerBindingType::Filtering } else { wgpu::SamplerBindingType::NonFiltering }),
                ),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("black and white"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("black and white"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let filter = if filterable { wgpu::FilterMode::Linear } else { wgpu::FilterMode::Nearest };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("black and white"), mag_filter: filter, min_filter: filter, ..Default::default() });
        let placement = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("black and white placement"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        BlackAndWhite {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout,
            sampler,
            placement,
            picture: Arc::new(Mutex::new(None)),
            measurement: Arc::new(Mutex::new(Measurement::default())),
        }
    }
}

impl Display for BlackAndWhite {
    fn name(&self) -> &'static str {
        "Black and white"
    }

    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let mut picture = self.picture.lock().unwrap();
        if picture.as_ref().is_none_or(|p| p.texture_id != frame.texture_id) {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("black and white"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.placement.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            *picture = Some(Picture { texture_id: frame.texture_id, bind_group, width: frame.width, height: frame.height, bottom_up: frame.bottom_up });
        }
        if let Some(p) = picture.as_mut() {
            p.bottom_up = frame.bottom_up;
        }
    }

    /// The weights are the waveform's: the same Colour Setup and the same luma weights choice.
    fn configure(&self, context: &DisplayContext) {
        *self.measurement.lock().unwrap() = crate::output_conversion::measurement(context.colour_setup.clone(), context.luma);
    }

    fn label(&self) -> String {
        match self.picture.lock().unwrap().is_some() {
            true => format!("Black and white  ·  Luma  ·  {}", self.measurement.lock().unwrap().weights_label()),
            false => "Black and white".to_string(),
        }
    }

    /// Draw the picture in grey, scaled to fit inside `area` with its shape kept.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], _scale: f32) {
        let guard = self.picture.lock().unwrap();
        let Some(picture) = guard.as_ref() else { return };
        let [left, top, width, height] = area;
        if width < 1.0 || height < 1.0 {
            return;
        }
        let scale = (width / picture.width as f32).min(height / picture.height as f32);
        let (w, h) = (picture.width as f32 * scale, picture.height as f32 * scale);
        let (x, y) = ((left + (width - w) / 2.0).round(), (top + (height - h) / 2.0).round());
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let [r, g, b] = self.measurement.lock().unwrap().weights();
        let placement: [f32; 12] = [
            clip(x, surface[0]),
            -clip(y, surface[1]),
            clip(x + w.round(), surface[0]),
            -clip(y + h.round(), surface[1]),
            if picture.bottom_up { 1.0 } else { 0.0 },
            0.0,
            0.0,
            0.0,
            r as f32,
            g as f32,
            b as f32,
            0.0,
        ];
        self.queue.write_buffer(&self.placement, 0, bytemuck::cast_slice(&placement));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &picture.bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}
