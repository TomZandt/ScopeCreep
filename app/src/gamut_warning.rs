//! The gamut/legal warning (spec #49): the Output picture's Luma in black and white, with problem pixels coloured.
//!
//! Luma (from the Output Colour Space's primaries, ADR 0004) is drawn as grey. From the upper threshold (90 %
//! by default) up to legal white a pixel fades towards red, and above legal white it is solid red; from the lower
//! threshold (10 %) down to legal black it fades towards blue, and below legal black it is solid blue. Every
//! channel is checked, not just Luma: a channel above the upper limit or below the lower limit makes the pixel
//! solid red or blue, so a colour outside the Output Colour Space's gamut is caught too (ADR 0009: clipping is
//! judged on the Output picture).
//!
//! Percentages are of the range of the project's Levels (video 64 to 940, data 0 to 1023). Inside Resolve every
//! value is full range, so 0 % is value 0.0 and 100 % is 1.0 whichever Levels are set; the Levels decide the
//! codes the label names. EBU R103 tolerance moves the limits to -5 % and 105 %.
use crate::colour_setup::Provenance;
use crate::display::{Display, DisplayContext};
use crate::gpu_frame::GpuFrame;
use crate::settings::{Choice, SettingDef};
use crate::waveform::{LumaWeights, Measurement};
use std::sync::{Arc, Mutex};

/// EBU R103: signals up to -5 % and 105 % are tolerated.
pub const R103_TOLERANCE: f32 = 0.05;

const SHADER: &str = r#"
struct Params {
    rect: vec4<f32>,      // left, top, right, bottom in clip space
    picture: vec4<f32>,   // x > 0.5: the texture's first row is the bottom; y, z: width, height in pixels
    limits: vec4<f32>,    // blue starts at x and is solid at y; red starts at z and is solid at w
    weights: vec4<f32>,   // luma weights for red, green, blue
};
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var frame: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    var out: VertexOut;
    out.position = vec4<f32>(mix(p.rect.x, p.rect.z, corner.x), mix(p.rect.y, p.rect.w, corner.y), 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, mix(corner.y, 1.0 - corner.y, step(0.5, p.picture.x)));
    return out;
}

const RED = vec3<f32>(1.0, 0.0, 0.0);
const BLUE = vec3<f32>(0.0, 0.0, 1.0);

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(p.picture.y, p.picture.z);
    let texel = vec2<i32>(clamp(floor(in.uv * size), vec2<f32>(0.0), size - 1.0));
    let rgb = textureLoad(frame, texel, 0).rgb;
    let y = dot(rgb, p.weights.xyz);
    let grey = vec3<f32>(clamp(y, 0.0, 1.0));
    // Every channel: past a limit in any of them is solid, whatever the luma says.
    if (max(rgb.r, max(rgb.g, rgb.b)) > p.limits.w) {
        return vec4<f32>(RED, 1.0);
    }
    if (min(rgb.r, min(rgb.g, rgb.b)) < p.limits.y) {
        return vec4<f32>(BLUE, 1.0);
    }
    if (y > p.limits.z) {
        return vec4<f32>(mix(grey, RED, clamp((y - p.limits.z) / (p.limits.w - p.limits.z), 0.0, 1.0)), 1.0);
    }
    if (y < p.limits.x) {
        return vec4<f32>(mix(grey, BLUE, clamp((p.limits.x - y) / (p.limits.x - p.limits.y), 0.0, 1.0)), 1.0);
    }
    return vec4<f32>(grey, 1.0);
}
"#;

const LOW_SETTING: &str = "low";
const HIGH_SETTING: &str = "high";
const R103_SETTING: &str = "r103";

static SETTINGS: [SettingDef; 3] = [
    SettingDef {
        key: LOW_SETTING,
        label: "Blue starts at",
        choices: &[
            Choice { value: "10", text: "10 %" },
            Choice { value: "5", text: "5 %" },
            Choice { value: "15", text: "15 %" },
            Choice { value: "20", text: "20 %" },
        ],
    },
    SettingDef {
        key: HIGH_SETTING,
        label: "Red starts at",
        choices: &[
            Choice { value: "90", text: "90 %" },
            Choice { value: "95", text: "95 %" },
            Choice { value: "85", text: "85 %" },
            Choice { value: "80", text: "80 %" },
        ],
    },
    SettingDef {
        key: R103_SETTING,
        label: "EBU R103 tolerance (-5 % to 105 %)",
        choices: &[Choice { value: "off", text: "Off" }, Choice { value: "on", text: "On" }],
    },
];

/// What the warning is set to: the thresholds (fractions of the range), whether EBU R103 tolerance is on, and the
/// Measurement (Output Colour Space and Levels) it judges in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warning {
    /// Fade to blue starts at this fraction of the range (default 0.10).
    pub low: f32,
    /// Fade to red starts at this fraction of the range (default 0.90).
    pub high: f32,
    pub r103: bool,
    pub measurement: Measurement,
}

impl Default for Warning {
    fn default() -> Warning {
        Warning { low: 0.10, high: 0.90, r103: false, measurement: Measurement::default() }
    }
}

impl Warning {
    pub fn from_context(context: &DisplayContext) -> Warning {
        let percent = |key: &str, default: f32| context.settings.get(key).and_then(|v| v.parse::<f32>().ok()).map_or(default, |v| v / 100.0);
        Warning {
            low: percent(LOW_SETTING, 0.10),
            high: percent(HIGH_SETTING, 0.90),
            r103: context.settings.get(R103_SETTING).is_some_and(|v| v == "on"),
            measurement: crate::output_conversion::measurement(context.colour_setup.clone(), LumaWeights::FromPrimaries),
        }
    }

    /// The value a pixel (or a channel) must pass to be solid blue.
    pub fn lower_limit(&self) -> f32 {
        if self.r103 { -R103_TOLERANCE } else { 0.0 }
    }

    /// The value a pixel (or a channel) must pass to be solid red.
    pub fn upper_limit(&self) -> f32 {
        if self.r103 { 1.0 + R103_TOLERANCE } else { 1.0 }
    }

    /// The code a fraction of the range is, on the 10-bit scale of the Levels.
    fn code(&self, fraction: f32) -> i32 {
        let (scale, offset) = self.measurement.levels.scale_and_offset();
        (fraction as f64 * scale + offset).round() as i32
    }

    /// The quiet label: the Levels and the tolerance in use, and what is judged in (ADR 0007).
    pub fn label(&self) -> String {
        let m = &self.measurement;
        let range = m.levels.range_label();
        let levels = match m.levels_from {
            None => format!("{range} assumed"),
            Some(Provenance::Project) => range.to_string(),
            Some(Provenance::Manual) => format!("{range} set manually"),
        };
        let space = if m.assumed { format!("{} assumed, judged as sent, unconverted", m.space.name()) } else { m.space.name().to_string() };
        let percent = |f: f32| format!("{:.0} %", f * 100.0);
        let tolerance = if self.r103 {
            format!("EBU R103 tolerance on ({} to {})", percent(self.lower_limit()), percent(self.upper_limit()))
        } else {
            "no tolerance".to_string()
        };
        format!(
            "Gamut/legal warning  ·  {levels}  ·  {space}  ·  blue below {} (code {}), red above {} (code {})  ·  {tolerance}  ·  every channel checked",
            percent(self.low),
            self.code(self.low),
            percent(self.high),
            self.code(self.high),
        )
    }
}

struct Picture {
    texture_id: u64,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    bottom_up: bool,
}

/// The gamut/legal warning's GPU state. Cheap to clone, like the other displays.
#[derive(Clone)]
pub struct GamutWarning {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    picture: Arc<Mutex<Option<Picture>>>,
    warning: Arc<Mutex<Warning>>,
}

impl GamutWarning {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> GamutWarning {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("gamut warning"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gamut warning"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("gamut warning"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gamut warning"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("gamut warning params"), size: 64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        GamutWarning { device: device.clone(), queue: queue.clone(), pipeline, layout, params, picture: Arc::new(Mutex::new(None)), warning: Arc::new(Mutex::new(Warning::default())) }
    }

    fn show(&self, frame: &GpuFrame) {
        let mut picture = self.picture.lock().unwrap();
        if picture.as_ref().is_none_or(|p| p.texture_id != frame.texture_id) {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gamut warning"),
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
}

impl Display for GamutWarning {
    fn name(&self) -> &'static str {
        "Gamut/legal warning"
    }

    fn on_frame(&self, _encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.show(frame);
    }

    fn configure(&self, context: &DisplayContext) {
        *self.warning.lock().unwrap() = Warning::from_context(context);
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn label(&self) -> String {
        self.warning.lock().unwrap().label()
    }

    /// Draw the picture's warning, scaled to fit inside `area` with the frame's shape kept.
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
        let warning = *self.warning.lock().unwrap();
        let [r, g, b] = warning.measurement.weights();
        let params: [f32; 16] = [
            clip(x, surface[0]),
            -clip(y, surface[1]),
            clip(x + w.round(), surface[0]),
            -clip(y + h.round(), surface[1]),
            if picture.bottom_up { 1.0 } else { 0.0 },
            picture.width as f32,
            picture.height as f32,
            0.0,
            warning.low,
            warning.lower_limit(),
            warning.high,
            warning.upper_limit(),
            r as f32,
            g as f32,
            b as f32,
            0.0,
        ];
        self.queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &picture.bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}
