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
//! The stage also finds the **mattes** (black bars: whole rows at the top and bottom or whole columns at the
//! sides in which every pixel is exactly black) once per frame, on the GPU and without reading anything back, and
//! draws them black whatever the colour rule would make of them (#102, ADR 0029).
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
    info: vec4<f32>,   // picture width, height, bottom row first (> 0.5), what is drawn: 0 picture, 1 legend, 2 fill
};
@group(0) @binding(0) var<uniform> stage: Stage;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
@group(0) @binding(3) var<uniform> values: Values;
// The picture's matte: first and last picture row and column that are not black bars (texel coordinates).
@group(0) @binding(4) var<storage, read> matte: array<u32>;

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
    if (stage.info.w > 0.5 && stage.info.w < 1.5) { return vec4<f32>(legend(in.uv), 1.0); }
    if (stage.info.w < 0.5) {
        let at = vec2<u32>(clamp(in.uv * stage.info.xy, vec2<f32>(0.0), stage.info.xy - vec2<f32>(1.0)));
        if (at.y < matte[0] || at.y > matte[1] || at.x < matte[2] || at.x > matte[3]) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    }
    return vec4<f32>(colour(in.uv), 1.0);
}
"#;

/// Finds the black bars of a frame in two compute steps: `mark` flags each row and column that holds a pixel that is
/// not exactly black, `find` walks in from each edge to the first flagged one and writes the picture's first and
/// last row and column (the whole frame when it is all black).
const MATTE: &str = r#"
@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> flags: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> bounds: array<u32>;

@compute @workgroup_size(8, 8)
fn mark(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(frame);
    if (id.x >= size.x || id.y >= size.y) { return; }
    if (any(textureLoad(frame, vec2<i32>(id.xy), 0).rgb != vec3<f32>(0.0))) {
        atomicStore(&flags[id.y], 1u);
        atomicStore(&flags[size.y + id.x], 1u);
    }
}

@compute @workgroup_size(1)
fn find() {
    let size = textureDimensions(frame);
    var top = 0u;
    while (top < size.y && atomicLoad(&flags[top]) == 0u) { top++; }
    if (top == size.y) {
        bounds[0] = 0u; bounds[1] = size.y - 1u; bounds[2] = 0u; bounds[3] = size.x - 1u;
        return;
    }
    var bottom = size.y - 1u;
    while (atomicLoad(&flags[bottom]) == 0u) { bottom--; }
    var left = 0u;
    while (atomicLoad(&flags[size.y + left]) == 0u) { left++; }
    var right = size.x - 1u;
    while (atomicLoad(&flags[size.y + right]) == 0u) { right--; }
    bounds[0] = top; bounds[1] = bottom; bounds[2] = left; bounds[3] = right;
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
    /// Per frame: which rows and columns hold picture, and the compute's bindings.
    flags: wgpu::Buffer,
    matte_group: wgpu::BindGroup,
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
    matte_layout: wgpu::BindGroupLayout,
    matte_mark: wgpu::ComputePipeline,
    matte_find: wgpu::ComputePipeline,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let matte_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matte"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                storage(1),
                storage(2),
            ],
        });
        let matte_module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("matte"), source: wgpu::ShaderSource::Wgsl(MATTE.into()) });
        let matte_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("matte"), bind_group_layouts: &[Some(&matte_layout)], immediate_size: 0 });
        let matte_step = |entry: &'static str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&matte_pipeline_layout),
                module: &matte_module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let (matte_mark, matte_find) = (matte_step("mark"), matte_step("find"));
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
            matte_layout,
            matte_mark,
            matte_find,
            picture_stage: buffer(32),
            legend_stage: buffer(32),
            values: buffer(values_size),
            values_size,
            bound: Arc::new(Mutex::new(None)),
        }
    }

    fn bind_group(&self, stage: &wgpu::Buffer, frame: &GpuFrame, bounds: &wgpu::Buffer) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: stage.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: self.values.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: bounds.as_entire_binding() },
            ],
        })
    }

    /// Show the Feed frame the uploader put on the GPU: bound once per texture, its flip kept up to date, its
    /// mattes found by a small GPU job recorded into `encoder` after the frame's own upload (nothing is read back,
    /// so nothing waits).
    pub fn show(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.bind(frame, Some(encoder));
    }

    /// As [`show`](Self::show) for a frame that is already on the GPU: the mattes are found in a job of their own.
    pub fn show_now(&self, frame: &GpuFrame) {
        self.bind(frame, None);
    }

    fn bind(&self, frame: &GpuFrame, encoder: Option<&mut wgpu::CommandEncoder>) {
        let mut bound = self.bound.lock().unwrap();
        if bound.as_ref().is_none_or(|b| b.texture_id != frame.texture_id) {
            let storage = |size: u64| self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("matte"), size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let flags = storage((frame.width as u64 + frame.height as u64) * 4);
            let bounds = storage(16);
            // Until the first frame is measured there is no matte.
            self.queue.write_buffer(&bounds, 0, bytemuck::cast_slice(&[0u32, frame.height.saturating_sub(1), 0, frame.width.saturating_sub(1)]));
            let matte_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("matte"),
                layout: &self.matte_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&frame.texture.create_view(&Default::default())) },
                    wgpu::BindGroupEntry { binding: 1, resource: flags.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: bounds.as_entire_binding() },
                ],
            });
            *bound = Some(Bound {
                texture_id: frame.texture_id,
                texture: frame.texture.clone(),
                picture_group: self.bind_group(&self.picture_stage, frame, &bounds),
                legend_group: self.bind_group(&self.legend_stage, frame, &bounds),
                flags,
                matte_group,
                width: frame.width,
                height: frame.height,
                bottom_up: frame.bottom_up,
            });
        }
        if let Some(b) = bound.as_mut() {
            b.bottom_up = frame.bottom_up;
            match encoder {
                Some(encoder) => self.find_mattes(encoder, b),
                None => {
                    let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("matte") });
                    self.find_mattes(&mut encoder, b);
                    self.queue.submit([encoder.finish()]);
                }
            }
        }
    }

    /// Mark every row and column that holds picture, then walk in from the edges to the bars' inner sides.
    fn find_mattes(&self, encoder: &mut wgpu::CommandEncoder, bound: &Bound) {
        encoder.clear_buffer(&bound.flags, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("matte"), timestamp_writes: None });
            pass.set_bind_group(0, &bound.matte_group, &[]);
            pass.set_pipeline(&self.matte_mark);
            pass.dispatch_workgroups(bound.width.div_ceil(8), bound.height.div_ceil(8), 1);
            pass.set_pipeline(&self.matte_find);
            pass.dispatch_workgroups(1, 1, 1);
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
        self.write_stage(&self.picture_stage, bound, [p.x, p.y, p.w, p.h], surface, 0.0);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bound.picture_group, &[]);
        pass.draw(0..4, 0..1);
        if let Some(l) = legend.filter(|l| l.strip > 0.0) {
            let bar = [area[0], area[1] + area[3] - l.strip + l.bar_offset, area[2], l.bar];
            self.write_stage(&self.legend_stage, bound, bar, surface, 1.0);
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
        self.write_stage(&self.picture_stage, bound, area, surface, 2.0);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bound.picture_group, &[]);
        pass.draw(0..4, 0..1);
    }

    fn write_values<V: Pod>(&self, values: &V) {
        assert_eq!(size_of::<V>(), self.values_size, "the values are the size the stage was made for");
        self.queue.write_buffer(&self.values, 0, bytemuck::bytes_of(values));
    }

    /// `rect` is left, top, width, height in device pixels.
    fn write_stage(&self, buffer: &wgpu::Buffer, bound: &Bound, rect: [f32; 4], surface: [f32; 2], what: f32) {
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let words: [f32; 8] = [
            clip(rect[0], surface[0]),
            -clip(rect[1], surface[1]),
            clip(rect[0] + rect[2], surface[0]),
            -clip(rect[1] + rect[3], surface[1]),
            bound.width as f32,
            bound.height as f32,
            if bound.bottom_up { 1.0 } else { 0.0 },
            what,
        ];
        self.queue.write_buffer(buffer, 0, bytemuck::cast_slice(&words));
    }
}
