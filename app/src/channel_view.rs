//! The Channel view: one channel of the Output picture drawn as grey, for inspecting noise (spec #49, #55), and
//! the Loupe on top of it.
//!
//! Blue is the default (the noisiest channel of most digital cameras); red and green are in the right-click
//! menu. The Loupe is a movable square that shows the picture under its middle magnified. Its zoom is of *source*
//! pixels: at 200 % every picture pixel is a 2 x 2 block of device pixels, at 400 % 4 x 4, whatever the Panel's
//! size, and exact (nearest-neighbour), so single pixels can be told apart. The scroll wheel over it steps the
//! zoom from 100 % to 800 % in whole 100 % steps; dragging moves it; it never leaves the Panel. Its size (small,
//! medium, large) is a menu choice; its position and zoom are remembered as state (ADR 0015).
use crate::display::{ChannelViewDrawn, Display, DisplayContext, Pointer, PointerResult};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{PictureStage, Rule, place};
use crate::output_conversion::Plan;
use crate::settings::{Choice, SettingDef};
use std::sync::{Arc, Mutex};

/// The zoom a Loupe starts at, and its limits, in percent of source pixels.
pub const DEFAULT_ZOOM: u32 = 200;
pub const MIN_ZOOM: u32 = 100;
pub const MAX_ZOOM: u32 = 800;
const ZOOM_STEP: u32 = 100;

const STATE_KEYS: &[&str] = &["loupe_x", "loupe_y", "loupe_zoom"];

static SETTINGS: [SettingDef; 2] = [
    SettingDef {
        key: "channel",
        label: "Channel",
        choices: &[Choice { value: "blue", text: "Blue" }, Choice { value: "red", text: "Red" }, Choice { value: "green", text: "Green" }],
    },
    SettingDef {
        key: "size",
        label: "Loupe size",
        choices: &[Choice { value: "medium", text: "Medium" }, Choice { value: "small", text: "Small" }, Choice { value: "large", text: "Large" }],
    },
];

/// Which channel is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    Blue,
    Red,
    Green,
}

impl Channel {
    fn from_setting(value: Option<&str>) -> Channel {
        match value {
            Some("red") => Channel::Red,
            Some("green") => Channel::Green,
            _ => Channel::Blue,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Channel::Blue => "Blue",
            Channel::Red => "Red",
            Channel::Green => "Green",
        }
    }
    /// The index the shader picks the channel by.
    fn index(self) -> f32 {
        match self {
            Channel::Red => 0.0,
            Channel::Green => 1.0,
            Channel::Blue => 2.0,
        }
    }
}

/// The Loupe's side, in logical pixels (never more than the Panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoupeSize {
    Small,
    Medium,
    Large,
}

impl LoupeSize {
    fn from_setting(value: Option<&str>) -> LoupeSize {
        match value {
            Some("small") => LoupeSize::Small,
            Some("large") => LoupeSize::Large,
            _ => LoupeSize::Medium,
        }
    }
    fn logical(self) -> f32 {
        match self {
            LoupeSize::Small => 100.0,
            LoupeSize::Medium => 180.0,
            LoupeSize::Large => 300.0,
        }
    }
}

/// Where the Loupe is and how far it magnifies: the part the user changes.
#[derive(Clone, Copy, Debug)]
struct Loupe {
    /// The middle of the Loupe as a fraction of the Panel's width and height.
    x: f32,
    y: f32,
    zoom: u32,
    size: LoupeSize,
    /// While dragged: where the pointer took hold, relative to the Loupe's middle.
    grab: Option<[f32; 2]>,
    /// Scrolling not yet a whole step (a smooth wheel).
    wheel: f32,
}

impl Loupe {
    fn new() -> Loupe {
        Loupe { x: 0.5, y: 0.5, zoom: DEFAULT_ZOOM, size: LoupeSize::Medium, grab: None, wheel: 0.0 }
    }

    /// The square in `area` (left, top, width, height) at `scale`: whole device pixels, wholly inside the area.
    fn rect(&self, area: [f32; 4], scale: f32) -> [f32; 4] {
        let side = (self.size.logical() * scale).min(area[2]).min(area[3]).floor().max(1.0);
        let left = (area[0] + self.x * area[2] - side / 2.0).clamp(area[0], (area[0] + area[2] - side).max(area[0])).round();
        let top = (area[1] + self.y * area[3] - side / 2.0).clamp(area[1], (area[1] + area[3] - side).max(area[1])).round();
        [left, top, side, side]
    }

    /// Put the Loupe's middle at `point` (as far as the area allows).
    fn centre_at(&mut self, point: [f32; 2], area: [f32; 4], scale: f32) {
        self.x = (point[0] - area[0]) / area[2];
        self.y = (point[1] - area[1]) / area[3];
        let rect = self.rect(area, scale);
        self.x = (rect[0] + rect[2] / 2.0 - area[0]) / area[2];
        self.y = (rect[1] + rect[3] / 2.0 - area[1]) / area[3];
    }

    fn middle(rect: [f32; 4]) -> [f32; 2] {
        [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0]
    }

    fn contains(rect: [f32; 4], p: [f32; 2]) -> bool {
        p[0] >= rect[0] && p[0] < rect[0] + rect[2] && p[1] >= rect[1] && p[1] < rect[1] + rect[3]
    }

    fn position_state(&self) -> Vec<(&'static str, String)> {
        vec![("loupe_x", format!("{:.5}", self.x)), ("loupe_y", format!("{:.5}", self.y))]
    }

    /// Take remembered state, leaving anything that is not sensible at its default.
    fn load(&mut self, state: &crate::settings::Settings) {
        let fraction = |key: &str| state.get(key).and_then(|v| v.parse::<f32>().ok()).filter(|v| (0.0..=1.0).contains(v));
        if let (Some(x), Some(y)) = (fraction("loupe_x"), fraction("loupe_y")) {
            (self.x, self.y) = (x, y);
        }
        if let Some(zoom) = state.get("loupe_zoom").and_then(|v| v.parse::<u32>().ok()).filter(|z| (MIN_ZOOM..=MAX_ZOOM).contains(z)) {
            self.zoom = (zoom + ZOOM_STEP / 2) / ZOOM_STEP * ZOOM_STEP;
        }
    }
}

/// The colour rule the stage draws the picture with: one channel as grey.
const RULE: &str = r#"
struct Values { channel: vec4<f32> };   // x: the channel (0 R, 1 G, 2 B)

fn colour(uv: vec2<f32>) -> vec3<f32> {
    let c = sample_picture(uv);
    var v = c.b;
    if values.channel.x < 0.5 { v = c.r; } else if values.channel.x < 1.5 { v = c.g; }
    return vec3<f32>(v);
}
"#;

/// The Loupe's own draw, on top of the picture: a square with the source pixels under it, magnified.
const LOUPE_SHADER: &str = r#"
struct Params {
    quad: vec4<f32>,    // left, top, right, bottom in clip space
    loupe: vec4<f32>,   // left, top, side, device pixels per source pixel
    src: vec4<f32>,     // frame width, height, source pixel at the Loupe's middle (x, y)
    mid: vec4<f32>,     // the Loupe's middle pixel (x, y), channel (0 R, 1 G, 2 B), bottom row first
};
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var picture: texture_2d<f32>;

struct VertexOut { @builtin(position) position: vec4<f32> };

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    var out: VertexOut;
    out.position = vec4<f32>(mix(p.quad.x, p.quad.z, corner.x), mix(p.quad.y, p.quad.w, corner.y), 0.0, 1.0);
    return out;
}

fn pick(c: vec3<f32>) -> f32 {
    if p.mid.z < 0.5 { return c.r; }
    if p.mid.z < 1.5 { return c.g; }
    return c.b;
}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    let pos = in.position.xy;
    let lo = p.loupe;
    let edge = min(min(pos.x - lo.x, lo.x + lo.z - pos.x), min(pos.y - lo.y, lo.y + lo.z - pos.y));
    if edge < 2.0 { return vec4<f32>(0.9, 0.9, 0.9, 1.0); }
    let s = p.src.zw + floor((floor(pos) - p.mid.xy) / lo.w);
    if s.x < 0.0 || s.y < 0.0 || s.x >= p.src.x || s.y >= p.src.y { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    var row = s.y;
    if p.mid.w > 0.5 { row = p.src.y - 1.0 - s.y; }
    let v = pick(textureLoad(picture, vec2<i32>(i32(s.x), i32(row)), 0).rgb);
    return vec4<f32>(v, v, v, 1.0);
}
"#;

const PARAMS_BYTES: u64 = 64;

struct State {
    channel: Channel,
    loupe: Loupe,
    /// Remembered state is taken once, from the first [`Display::configure`] (the user's file at start-up);
    /// after that the display's own is the newer.
    loaded: bool,
    /// The Panel's area and scale when last drawn or touched, for [`ChannelViewDrawn`].
    area: Option<([f32; 4], f32)>,
    tail: String,
}

/// The Channel view. Cheap to clone: the Feed thread hands it frames while the window thread draws.
#[derive(Clone)]
pub struct ChannelView {
    device: wgpu::Device,
    queue: wgpu::Queue,
    stage: PictureStage,
    loupe_pipeline: wgpu::RenderPipeline,
    loupe_layout: wgpu::BindGroupLayout,
    loupe_params: wgpu::Buffer,
    /// The Loupe's bind group for the frame's texture.
    loupe_bound: Arc<Mutex<Option<(u64, wgpu::BindGroup)>>>,
    state: Arc<Mutex<State>>,
}

impl ChannelView {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool) -> ChannelView {
        let stage = PictureStage::new::<[f32; 4]>(device, queue, target, filterable, Rule { label: "channel view", wgsl: RULE.into(), has_legend: false });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("channel view loupe"), source: wgpu::ShaderSource::Wgsl(LOUPE_SHADER.into()) });
        let loupe_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("channel view loupe"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("channel view loupe"), bind_group_layouts: &[Some(&loupe_layout)], immediate_size: 0 });
        let loupe_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("channel view loupe"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let loupe_params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("channel view loupe params"), size: PARAMS_BYTES, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        ChannelView {
            device: device.clone(),
            queue: queue.clone(),
            stage,
            loupe_pipeline,
            loupe_layout,
            loupe_params,
            loupe_bound: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(State { channel: Channel::Blue, loupe: Loupe::new(), loaded: false, area: None, tail: String::new() })),
        }
    }
}

impl Display for ChannelView {
    fn name(&self) -> &'static str {
        "Channel view"
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.stage.show(encoder, frame);
    }

    fn configure(&self, context: &DisplayContext) {
        let mut state = self.state.lock().unwrap();
        state.channel = Channel::from_setting(context.settings.get("channel").map(String::as_str));
        state.loupe.size = LoupeSize::from_setting(context.settings.get("size").map(String::as_str));
        if !state.loaded {
            state.loaded = true;
            state.loupe.load(&context.settings);
        }
        let conversion = Plan::for_setup(context.colour_setup.as_ref().map(|c| &c.setup));
        state.tail = format!("{}  ·  {}", context.colour_label, conversion.label());
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn state_keys(&self) -> &'static [&'static str] {
        STATE_KEYS
    }

    fn label(&self) -> String {
        // The picture first, then the state: the order `draw` takes them in.
        let size = self.stage.size();
        let state = self.state.lock().unwrap();
        let head = format!("{} channel as grey", state.channel.name());
        match size {
            Some([w, h]) => format!("{head}  ·  {w} × {h}  ·  Loupe {} %  ·  {}", state.loupe.zoom, state.tail),
            None => head,
        }
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let Some(shown) = self.stage.shown() else { return };
        if area[2] < 1.0 || area[3] < 1.0 {
            return;
        }
        let mut state = self.state.lock().unwrap();
        state.area = Some((area, scale));
        // The picture, through the stage.
        self.stage.draw(pass, area, surface, &[state.channel.index(), 0.0, 0.0, 0.0], None);
        // The Loupe on top, and the source pixel under its middle.
        let Some(p) = place(area, shown.size, 0.0) else { return };
        let rect = state.loupe.rect(area, scale);
        let middle = Loupe::middle(rect);
        let (mx, my) = (middle[0].round(), middle[1].round());
        let source = [((mx - p.x) / p.w * shown.size[0] as f32).floor(), ((my - p.y) / p.h * shown.size[1] as f32).floor()];
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let params: [f32; 16] = [
            clip(rect[0], surface[0]),
            -clip(rect[1], surface[1]),
            clip(rect[0] + rect[2], surface[0]),
            -clip(rect[1] + rect[3], surface[1]),
            rect[0],
            rect[1],
            rect[2],
            state.loupe.zoom as f32 / 100.0,
            shown.size[0] as f32,
            shown.size[1] as f32,
            source[0],
            source[1],
            mx,
            my,
            state.channel.index(),
            if shown.bottom_up { 1.0 } else { 0.0 },
        ];
        self.queue.write_buffer(&self.loupe_params, 0, bytemuck::cast_slice(&params));
        let mut bound = self.loupe_bound.lock().unwrap();
        if bound.as_ref().is_none_or(|(id, _)| *id != shown.texture_id) {
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("channel view loupe"),
                layout: &self.loupe_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.loupe_params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shown.texture.create_view(&Default::default())) },
                ],
            });
            *bound = Some((shown.texture_id, group));
        }
        pass.set_pipeline(&self.loupe_pipeline);
        pass.set_bind_group(0, &bound.as_ref().unwrap().1, &[]);
        pass.draw(0..4, 0..1);
    }

    fn pointer(&self, area: [f32; 4], scale: f32, event: Pointer) -> PointerResult {
        let mut state = self.state.lock().unwrap();
        state.area = Some((area, scale));
        let rect = state.loupe.rect(area, scale);
        match event {
            Pointer::Down(p) if Loupe::contains(rect, p) => {
                let m = Loupe::middle(rect);
                state.loupe.grab = Some([p[0] - m[0], p[1] - m[1]]);
                PointerResult { used: true, remember: Vec::new() }
            }
            Pointer::Move(p) => match state.loupe.grab {
                Some(g) => {
                    state.loupe.centre_at([p[0] - g[0], p[1] - g[1]], area, scale);
                    PointerResult { used: true, remember: state.loupe.position_state() }
                }
                None => PointerResult::default(),
            },
            Pointer::Up => match state.loupe.grab.take() {
                Some(_) => PointerResult { used: true, remember: state.loupe.position_state() },
                None => PointerResult::default(),
            },
            Pointer::Wheel { at, lines } if Loupe::contains(rect, at) => {
                state.loupe.wheel += lines;
                let steps = state.loupe.wheel.trunc();
                state.loupe.wheel -= steps;
                let zoom = (state.loupe.zoom as i32 + steps as i32 * ZOOM_STEP as i32).clamp(MIN_ZOOM as i32, MAX_ZOOM as i32) as u32;
                // Nothing to remember when the zoom did not change (a part of a notch, or already at its limit).
                let changed = zoom != state.loupe.zoom;
                state.loupe.zoom = zoom;
                PointerResult { used: true, remember: if changed { vec![("loupe_zoom", zoom.to_string())] } else { Vec::new() } }
            }
            _ => PointerResult::default(),
        }
    }

    fn read_back(&self) -> Option<Box<dyn std::any::Any>> {
        let state = self.state.lock().unwrap();
        let (area, scale) = state.area?;
        Some(Box::new(ChannelViewDrawn { loupe: state.loupe.rect(area, scale), zoom: state.loupe.zoom }))
    }
}
