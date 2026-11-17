//! The Skin Tone Panel (spec #49 stories 68-74): the Output picture in grey (Luma), with the pixels whose hue is
//! near the skin-tone line lit up: yellow on the line, green or magenta as the hue drifts to either side, grey for
//! neutral pixels and for hues far from skin. Only hue matters, not brightness, so it works for every skin tone.
//!
//! Hue is the angle of the pixel's Cb/Cr (the vectorscope's, with the same luma weights), measured from the
//! shared [`SKIN_TONE_LINE_DEGREES`], which is the angle Resolve 21.2's own vectorscope draws its line at (#60, ADR 0018). The
//! tolerance `t` is a list of choices (ADR 0013, ADR 0021): yellow within `t` of the line, the colour fading in
//! (yellow to green or magenta) out to 4 t, then fading back to grey, which it is beyond 6 t. At the default 5
//! degrees that is the spec's 5, 20 and 30. Towards a larger Cb/Cr angle (towards yellow) is green, the other way
//! magenta. The tints share one brightness and keep the grey's lightness, so the shape of the picture stays readable.
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::output_conversion::{Plan, measurement};
use crate::settings::{Choice, SettingDef};
use crate::skin_tone::SKIN_TONE_LINE_DEGREES;
use std::sync::{Arc, Mutex};

pub const TOLERANCE_SETTING: &str = "tolerance";

const DEFAULT_TOLERANCE: &str = "5";

const SETTINGS: [SettingDef; 1] = [SettingDef {
    key: TOLERANCE_SETTING,
    label: "On-line tolerance",
    choices: &[
        Choice { value: DEFAULT_TOLERANCE, text: "±5° (default)" },
        Choice { value: "2", text: "±2°" },
        Choice { value: "3", text: "±3°" },
        Choice { value: "8", text: "±8°" },
        Choice { value: "10", text: "±10°" },
    ],
}];

const SHADER: &str = r#"
struct Params {
    rect: vec4<f32>,      // left, top, right, bottom in clip space
    flip: vec4<f32>,      // x > 0.5: the texture's first row is the bottom of the picture
    weights: vec4<f32>,   // luma weights for R, G, B
    skin: vec4<f32>,      // x: tolerance in degrees, y: skin-tone line angle in degrees
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

const PI: f32 = 3.14159265358979;

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(picture));
    let at = vec2<i32>(min(in.uv * size, size - vec2<f32>(1.0)));
    let rgb = textureLoad(picture, at, 0).rgb;
    let w = params.weights.xyz;
    let y = dot(rgb, w);
    let grey = clamp(y, 0.0, 1.0);
    let cb = (rgb.b - y) / (2.0 * (1.0 - w.z));
    let cr = (rgb.r - y) / (2.0 * (1.0 - w.x));
    // Degrees from the skin-tone line, wrapped to -180..180: positive is towards yellow (green), negative towards red (magenta).
    let turn = atan2(cr, cb) - params.skin.y * PI / 180.0;
    let offset = atan2(sin(turn), cos(turn)) * 180.0 / PI;
    let distance = abs(offset);
    let t = params.skin.x;
    let to_side = clamp((distance - t) / (3.0 * t), 0.0, 1.0);
    let to_grey = 1.0 - clamp((distance - 4.0 * t) / (2.0 * t), 0.0, 1.0);
    // Nearly neutral pixels have no hue to speak of.
    let strength = to_grey * smoothstep(0.01, 0.03, length(vec2<f32>(cb, cr)));
    let yellow = vec3<f32>(0.70, 0.58, 0.10);
    let green = vec3<f32>(0.20, 0.68, 0.35);
    let magenta = vec3<f32>(0.85, 0.45, 0.80);
    let tint = mix(yellow, select(magenta, green, offset > 0.0), to_side) * (0.4 + 0.6 * grey);
    return vec4<f32>(mix(vec3<f32>(grey), tint, strength), 1.0);
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
    /// The tolerance as the setting says it (degrees).
    degrees: String,
    tolerance: f32,
    /// How the picture was made, for the label (ADR 0007).
    source: String,
}

impl Default for Config {
    fn default() -> Config {
        Config { weights: [0.2126, 0.7152, 0.0722], degrees: DEFAULT_TOLERANCE.into(), tolerance: 5.0, source: String::new() }
    }
}

/// The display's GPU state. Cheap to clone: the Feed thread takes frames through one clone while the window
/// thread draws through another.
#[derive(Clone)]
pub struct SkinTone {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    picture: Arc<Mutex<Option<Picture>>>,
    config: Arc<Mutex<Config>>,
}

impl SkinTone {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, _filterable: bool) -> SkinTone {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("skin tone"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skin tone"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("skin tone"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("skin tone"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("skin tone params"), size: 64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        SkinTone { device: device.clone(), queue: queue.clone(), pipeline, layout, params, picture: Arc::new(Mutex::new(None)), config: Arc::new(Mutex::new(Config::default())) }
    }
}

impl Display for SkinTone {
    fn name(&self) -> &'static str {
        "Skin tone"
    }

    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let mut picture = self.picture.lock().unwrap();
        if picture.as_ref().is_none_or(|p| p.texture_id != frame.texture_id) {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("skin tone"),
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
        // The weights the vectorscope uses, so the hue here is the angle the vectorscope shows.
        let [r, g, b] = measurement(context.colour_setup.clone(), context.luma).weights();
        let degrees = context.settings.get(TOLERANCE_SETTING).cloned().unwrap_or_else(|| DEFAULT_TOLERANCE.into());
        let tolerance = degrees.parse::<f32>().unwrap_or(5.0);
        // The picture is the Output picture only once it has been converted (ADR 0007).
        let what = if plan.method().is_some() || plan.measured_space(setup).is_some() { "Output picture" } else { "Picture as sent" };
        let source = format!("{what}  ·  {}  ·  {}", context.colour_label, plan.label());
        *self.config.lock().unwrap() = Config { weights: [r as f32, g as f32, b as f32], degrees, tolerance, source };
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    /// States only what is proven (ADR 0007): the bands in degrees from the line, and that the line's angle is
    /// measured in Resolve 21.2 (#60).
    fn label(&self) -> String {
        if self.picture.lock().unwrap().is_none() {
            return "Skin tone".to_string();
        }
        let config = self.config.lock().unwrap();
        let t: f32 = config.tolerance;
        format!(
            "Skin tone  ·  yellow within ±{}° of the skin-tone line {SKIN_TONE_LINE_DEGREES}° (as Resolve 21.2 draws it), green or magenta out to ±{}°, grey beyond ±{}° and where saturation is low  ·  {}",
            config.degrees,
            t * 4.0,
            t * 6.0,
            config.source
        )
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
            SKIN_TONE_LINE_DEGREES as f32,
            0.0,
            0.0,
        ];
        self.queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &picture.bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}
