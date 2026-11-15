//! Drawing the Picture View: the latest Feed frame as an image, untouched.
//!
//! The frame is the 32-bit float texture at full resolution that `gpu_frame` uploads, drawn scaled to fit. No colour
//! conversion, no tone curve: the Sender's values go to the screen as they are (clamped to 0..1 by the display
//! surface, which is not sRGB-encoded for that reason).
use scopecreep::gpu_frame::GpuFrame;
use std::sync::{Arc, Mutex};

const SHADER: &str = r#"
struct Placement {
    rect: vec4<f32>,   // left, top, right, bottom in clip space
    flip: vec4<f32>,   // x > 0.5: the texture's first row is the bottom of the picture
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
    return vec4<f32>(textureSampleLevel(picture, picture_sampler, in.uv, 0.0).rgb, 1.0);
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

/// The Picture View's GPU state. Cheap to clone: the Feed thread uploads frames through one clone while the
/// window thread draws through another.
#[derive(Clone)]
pub struct PictureView {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    placement: wgpu::Buffer,
    picture: Arc<Mutex<Option<Picture>>>,
}

impl PictureView {
    /// `filterable`: the device can smooth 32-bit float textures (else the picture is drawn nearest-neighbour).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool) -> PictureView {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("picture view"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("picture view"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(if filterable { wgpu::SamplerBindingType::Filtering } else { wgpu::SamplerBindingType::NonFiltering }),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("picture view"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("picture view"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(target.into())],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let filter = if filterable { wgpu::FilterMode::Linear } else { wgpu::FilterMode::Nearest };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("picture view"), mag_filter: filter, min_filter: filter, ..Default::default() });
        let placement = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("picture placement"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        PictureView { device: device.clone(), queue: queue.clone(), pipeline, layout, sampler, placement, picture: Arc::new(Mutex::new(None)) }
    }

    /// Show the Feed frame [`FrameUploader`](scopecreep::gpu_frame::FrameUploader) put on the GPU. Called on
    /// the Feed thread.
    pub fn show(&self, frame: &GpuFrame) {
        let mut picture = self.picture.lock().unwrap();
        if picture.as_ref().is_none_or(|p| p.texture_id != frame.texture_id) {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("picture view"),
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

    /// Draw the picture, scaled to fit inside `area` (left, top, width, height in pixels of a `surface`-sized
    /// target) with its shape kept. Draws nothing until a frame has arrived.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2]) {
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
        let placement: [f32; 8] = [
            clip(x, surface[0]),
            -clip(y, surface[1]),
            clip(x + w.round(), surface[0]),
            -clip(y + h.round(), surface[1]),
            if picture.bottom_up { 1.0 } else { 0.0 },
            0.0,
            0.0,
            0.0,
        ];
        self.queue.write_buffer(&self.placement, 0, bytemuck::cast_slice(&placement));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &picture.bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}
