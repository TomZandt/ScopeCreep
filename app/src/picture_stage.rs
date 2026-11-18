//! The Picture stage (spec #106, ticket #110): the one place every Picture View draws through.
//!
//! The stage owns what a Picture View must never get wrong: placing the frame in its Panel (fit, centre, rounding
//! to whole device pixels, any scale), flipping a bottom-up frame, the GPU binding of the frame (cached per
//! texture), the uniform buffers and the legend strip under the picture. A Picture View supplies only
//!
//! - its **colour rule**: WGSL text with a `struct Values { ... }`, a `fn colour(uv: vec2<f32>) -> vec3<f32>`
//!   and, when it has a legend, a `fn legend(uv: vec2<f32>) -> vec3<f32>`. `uv` is the position in the picture
//!   (0..1, top left first, the flip already done); the rule reads the frame with `load_picture(uv)` (the exact
//!   texel, nearest) or `sample_picture(uv)` (smoothed where the device can);
//! - its **values**: a plain-data `#[repr(C)]` struct ([`bytemuck::Pod`]) that is `Values` on the GPU;
//! - optionally a **legend** ([`Legend`]): a strip under the picture, whose bar is drawn by `legend(uv)` with `uv.x`
//!   running across it.
//!
//! The Channel view's Loupe is its own extra draw on top of the picture; it asks the stage where the picture went
//! ([`place`]) and for the frame's texture ([`PictureStage::shown`]).
use crate::gpu_frame::GpuFrame;
use bytemuck::Pod;
use std::sync::{Arc, Mutex};

/// Where the picture is in its Panel, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Fit a `picture` (width, height) in an `area` (left, top, width, height) less a `strip` kept free at the bottom
/// for a legend: scaled to fit with its shape kept, centred, its size and position in whole device pixels.
/// `None` when nothing is left to draw in.
pub fn place(area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<Placement> {
    let [left, top, width, height] = area;
    let room = height - strip;
    if width < 1.0 || room < 1.0 || picture[0] == 0 || picture[1] == 0 {
        return None;
    }
    let fit = (width / picture[0] as f32).min(room / picture[1] as f32);
    let (w, h) = ((picture[0] as f32 * fit).round(), (picture[1] as f32 * fit).round());
    Some(Placement { x: (left + (width - w) / 2.0).round(), y: (top + (room - h) / 2.0).round(), w, h })
}

/// A strip under the picture with a bar drawn by the rule's `legend` function, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Legend {
    /// The height kept free under the picture (the bar and whatever text sits around it).
    pub strip: f32,
    /// Where the bar starts, down from the strip's top.
    pub bar_offset: f32,
    /// The bar's height.
    pub bar: f32,
}

/// The strip the bar legends share (ARRI False Colour, the saturation heatmap): the bar at the top of the strip
/// and its numbers under it, logical pixels.
pub const BAR_STRIP: f32 = 26.0;
pub const BAR: f32 = 8.0;
/// It is only drawn in a Panel at least this tall (logical pixels), so it never crowds the picture out.
pub const BAR_MIN_HEIGHT: f32 = 110.0;

impl Legend {
    /// The shared bar legend in a Panel of `area` at `scale`, when it is `on` and there is room.
    pub fn bar(on: bool, area: [f32; 4], scale: f32) -> Option<Legend> {
        (on && area[3] >= BAR_MIN_HEIGHT * scale).then_some(Legend { strip: BAR_STRIP * scale, bar_offset: 0.0, bar: BAR * scale })
    }
}

const PRELUDE: &str = r#"
struct Stage {
    rect: vec4<f32>,   // left, top, right, bottom in clip space
    info: vec4<f32>,   // picture width, height, bottom row first (> 0.5), drawing the legend (> 0.5)
};
@group(0) @binding(0) var<uniform> stage: Stage;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
@group(0) @binding(3) var<uniform> values: Values;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    var out: VertexOut;
    out.position = vec4<f32>(mix(stage.rect.x, stage.rect.z, corner.x), mix(stage.rect.y, stage.rect.w, corner.y), 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, mix(corner.y, 1.0 - corner.y, step(0.5, stage.info.z)));
    return out;
}

// The frame at `uv`, smoothed where the device can smooth 32-bit float textures.
fn sample_picture(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(picture, picture_sampler, uv, 0.0).rgb;
}

// The exact texel under `uv`.
fn load_picture(uv: vec2<f32>) -> vec3<f32> {
    let size = vec2<i32>(i32(stage.info.x), i32(stage.info.y));
    let at = clamp(vec2<i32>(uv * stage.info.xy), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(picture, at, 0).rgb;
}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    if (stage.info.w > 0.5) { return vec4<f32>(legend(in.uv), 1.0); }
    return vec4<f32>(colour(in.uv), 1.0);
}
"#;

/// A legend that draws nothing, for a rule that has none.
const NO_LEGEND: &str = "fn legend(uv: vec2<f32>) -> vec3<f32> { return vec3<f32>(0.0); }\n";

/// What a Picture View gives the stage once: its label (for GPU debugging), its WGSL and whether that has a
/// `legend` function.
pub struct Rule {
    pub label: &'static str,
    pub wgsl: String,
    pub has_legend: bool,
}

/// The frame on the GPU, with the bind groups that read it.
struct Bound {
    texture_id: u64,
    texture: wgpu::Texture,
    picture_group: wgpu::BindGroup,
    legend_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    bottom_up: bool,
}

/// The frame the stage is showing.
pub struct Shown {
    pub texture_id: u64,
    pub texture: wgpu::Texture,
    pub size: [u32; 2],
    pub bottom_up: bool,
}

/// A Picture View's drawing machinery. Cheap to clone: the Feed thread shows frames through one clone while the
/// window thread draws through another.
#[derive(Clone)]
pub struct PictureStage {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// One for the picture and one for the legend: both are drawn in the same pass, so they cannot share.
    picture_stage: wgpu::Buffer,
    legend_stage: wgpu::Buffer,
    values: wgpu::Buffer,
    values_size: usize,
    bound: Arc<Mutex<Option<Bound>>>,
}

impl PictureStage {
    /// `V` is the plain-data form of the rule's `Values`. `filterable`: the device can smooth 32-bit float
    /// textures (else `sample_picture` gives the nearest texel).
    pub fn new<V: Pod>(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool, rule: Rule) -> PictureStage {
        let values_size = size_of::<V>();
        assert!(values_size > 0 && values_size % 16 == 0, "a Picture View's values are whole vec4s");
        let source = format!("{PRELUDE}{}{}", rule.wgsl, if rule.has_legend { "" } else { NO_LEGEND });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(rule.label), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let uniform = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(rule.label),
            entries: &[
                uniform(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(if filterable { wgpu::SamplerBindingType::Filtering } else { wgpu::SamplerBindingType::NonFiltering }),
                    count: None,
                },
                uniform(3),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(rule.label), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(rule.label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let filter = if filterable { wgpu::FilterMode::Linear } else { wgpu::FilterMode::Nearest };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some(rule.label), mag_filter: filter, min_filter: filter, ..Default::default() });
        let buffer = |size: usize| device.create_buffer(&wgpu::BufferDescriptor { label: Some(rule.label), size: size as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        PictureStage {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout,
            sampler,
            picture_stage: buffer(32),
            legend_stage: buffer(32),
            values: buffer(values_size),
            values_size,
            bound: Arc::new(Mutex::new(None)),
        }
    }

    fn bind_group(&self, stage: &wgpu::Buffer, frame: &GpuFrame) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: stage.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: self.values.as_entire_binding() },
            ],
        })
    }

    /// Show the Feed frame the uploader put on the GPU: bound once per texture, its flip kept up to date.
    pub fn show(&self, frame: &GpuFrame) {
        let mut bound = self.bound.lock().unwrap();
        if bound.as_ref().is_none_or(|b| b.texture_id != frame.texture_id) {
            *bound = Some(Bound {
                texture_id: frame.texture_id,
                texture: frame.texture.clone(),
                picture_group: self.bind_group(&self.picture_stage, frame),
                legend_group: self.bind_group(&self.legend_stage, frame),
                width: frame.width,
                height: frame.height,
                bottom_up: frame.bottom_up,
            });
        }
        if let Some(b) = bound.as_mut() {
            b.bottom_up = frame.bottom_up;
        }
    }

    /// Whether a frame has been shown.
    pub fn has_picture(&self) -> bool {
        self.bound.lock().unwrap().is_some()
    }

    /// The size of the frame on screen (width, height), once one has arrived.
    pub fn size(&self) -> Option<[u32; 2]> {
        self.bound.lock().unwrap().as_ref().map(|b| [b.width, b.height])
    }

    /// The frame as shown, for a view's own extra draw (the Loupe).
    pub fn shown(&self) -> Option<Shown> {
        self.bound.lock().unwrap().as_ref().map(|b| Shown { texture_id: b.texture_id, texture: b.texture.clone(), size: [b.width, b.height], bottom_up: b.bottom_up })
    }

    /// Draw the picture fitted inside `area` (less the `legend`'s strip) on a `surface` of that size in device
    /// pixels, then the legend's bar.
    pub fn draw<V: Pod>(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], values: &V, legend: Option<Legend>) {
        let guard = self.bound.lock().unwrap();
        let Some(bound) = guard.as_ref() else { return };
        let strip = legend.map_or(0.0, |l| l.strip);
        let Some(p) = place(area, [bound.width, bound.height], strip) else { return };
        self.write_values(values);
        self.write_stage(&self.picture_stage, bound, [p.x, p.y, p.w, p.h], surface, false);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bound.picture_group, &[]);
        pass.draw(0..4, 0..1);
        if let Some(l) = legend.filter(|l| l.strip > 0.0) {
            let bar = [area[0], area[1] + area[3] - l.strip + l.bar_offset, area[2], l.bar];
            self.write_stage(&self.legend_stage, bound, bar, surface, true);
            pass.set_bind_group(0, &bound.legend_group, &[]);
            pass.draw(0..4, 0..1);
        }
    }

    /// Fill the whole `area` with the rule's colour (the picture's shape not kept), for a Panel with a message
    /// instead of a picture.
    pub fn fill<V: Pod>(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], values: &V) {
        let guard = self.bound.lock().unwrap();
        let Some(bound) = guard.as_ref() else { return };
        if area[2] < 1.0 || area[3] < 1.0 {
            return;
        }
        self.write_values(values);
        self.write_stage(&self.picture_stage, bound, area, surface, false);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bound.picture_group, &[]);
        pass.draw(0..4, 0..1);
    }

    fn write_values<V: Pod>(&self, values: &V) {
        assert_eq!(size_of::<V>(), self.values_size, "the values are the size the stage was made for");
        self.queue.write_buffer(&self.values, 0, bytemuck::bytes_of(values));
    }

    /// `rect` is left, top, width, height in device pixels.
    fn write_stage(&self, buffer: &wgpu::Buffer, bound: &Bound, rect: [f32; 4], surface: [f32; 2], legend: bool) {
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let words: [f32; 8] = [
            clip(rect[0], surface[0]),
            -clip(rect[1], surface[1]),
            clip(rect[0] + rect[2], surface[0]),
            -clip(rect[1] + rect[3], surface[1]),
            bound.width as f32,
            bound.height as f32,
            if bound.bottom_up { 1.0 } else { 0.0 },
            if legend { 1.0 } else { 0.0 },
        ];
        self.queue.write_buffer(buffer, 0, bytemuck::cast_slice(&words));
    }
}
