//! Flat-coloured rectangles drawn over everything else: the right-click menu's background, its highlight and its
//! tick. Rectangles are in device pixels of the target. Colours are neutral (the same on every channel), so
//! nothing on screen tints the eye.

const MAX_RECTS: usize = 256;
const VERTEX_BYTES: u64 = 6 * 24;

const SHADER: &str = r#"
struct Size { size: vec2<f32>, pad: vec2<f32> }
@group(0) @binding(0) var<uniform> target_size: Size;
struct Vertex { @location(0) position: vec2<f32>, @location(1) colour: vec4<f32> }
struct Out { @builtin(position) position: vec4<f32>, @location(0) colour: vec4<f32> }
@vertex fn vs(v: Vertex) -> Out {
    let ndc = vec2<f32>(v.position.x / target_size.size.x * 2.0 - 1.0, 1.0 - v.position.y / target_size.size.y * 2.0);
    return Out(vec4<f32>(ndc, 0.0, 1.0), v.colour);
}
@fragment fn fs(i: Out) -> @location(0) vec4<f32> { return i.colour; }
"#;

pub struct FlatRects {
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    size: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    count: u32,
}

impl FlatRects {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: wgpu::TextureFormat,
    ) -> FlatRects {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("flat rects"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 8,
                shader_location: 1,
            },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("flat rects"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("flat rects"),
            size: VERTEX_BYTES * MAX_RECTS as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let size = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("flat rects size"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("flat rects"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: size.as_entire_binding(),
            }],
        });
        FlatRects {
            queue: queue.clone(),
            pipeline,
            vertices,
            size,
            bind_group,
            count: 0,
        }
    }

    /// Set what to draw: `(left, top, width, height)` with its rgba, in order, later ones over earlier ones.
    pub fn set(&mut self, target: [f32; 2], rects: &[([f32; 4], [f32; 4])]) {
        let mut data: Vec<f32> = Vec::new();
        for ([l, t, w, h], c) in rects.iter().take(MAX_RECTS) {
            for (x, y) in [
                (*l, *t),
                (l + w, *t),
                (*l, t + h),
                (*l, t + h),
                (l + w, *t),
                (l + w, t + h),
            ] {
                data.extend_from_slice(&[x, y, c[0], c[1], c[2], c[3]]);
            }
        }
        self.count = (data.len() / 6) as u32;
        if !data.is_empty() {
            self.queue
                .write_buffer(&self.vertices, 0, bytemuck::cast_slice(&data));
        }
        self.queue.write_buffer(
            &self.size,
            0,
            bytemuck::cast_slice(&[target[0], target[1], 0.0, 0.0]),
        );
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..self.count, 0..1);
    }
}
