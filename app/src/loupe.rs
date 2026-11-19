//! The Loupe (spec #49, #55; shared by every Picture View that wants one, spec #158, #159): a movable square that
//! shows the picture under its middle magnified, drawn on top of a Picture stage.
//!
//! Its zoom is of *source* pixels: at 200 % every picture pixel is a 2 x 2 block of device pixels, at 400 % 4 x 4,
//! whatever the Panel's size, and exact (nearest-neighbour), so single pixels can be told apart. The scroll wheel
//! over it steps the zoom from 100 % to 800 % in whole 100 % steps; dragging moves it; it never leaves the Panel.
//! Its size (small, medium, large) is a menu choice ([`SIZE_SETTING`]); its position and zoom are remembered as
//! state ([`STATE_KEYS`], ADR 0015).
//!
//! The Loupe shows what its Panel shows because it is built from the Panel's own **colour rule** (the WGSL given
//! to the Picture stage): for every pixel it calls the rule's `colour(uv)` with `uv` at the middle of the source
//! texel, and `sample_picture` / `load_picture` give that exact texel. The Panel hands the Loupe the same
//! `Values` it hands the stage.
use crate::display::{LoupeDrawn, Pointer, PointerResult};
use crate::picture_stage::{Shown, place};
use crate::settings::{Choice, SettingDef, Settings};
use bytemuck::Pod;
use std::sync::{Arc, Mutex};

/// The zoom a Loupe starts at, and its limits, in percent of source pixels.
pub const DEFAULT_ZOOM: u32 = 200;
pub const MIN_ZOOM: u32 = 100;
pub const MAX_ZOOM: u32 = 800;
const ZOOM_STEP: u32 = 100;

/// The remembered state a Loupe keeps beside the settings (ADR 0015).
pub const X_KEY: &str = "loupe_x";
pub const Y_KEY: &str = "loupe_y";
pub const ZOOM_KEY: &str = "loupe_zoom";
pub const STATE_KEYS: &[&str] = &[X_KEY, Y_KEY, ZOOM_KEY];

/// The remembered zoom in `settings`, in whole 100 % steps within the limits: the default when absent or not sensible.
pub fn zoom(settings: &Settings) -> u32 {
    let remembered = settings.get(ZOOM_KEY).and_then(|v| v.parse::<u32>().ok()).filter(|z| (MIN_ZOOM..=MAX_ZOOM).contains(z));
    remembered.map_or(DEFAULT_ZOOM, |z| (z + ZOOM_STEP / 2) / ZOOM_STEP * ZOOM_STEP)
}

/// Whether the Loupe is switched on in `settings` (the clean picture's chip).
pub fn is_on(settings: &Settings) -> bool {
    settings.get(crate::toolbar::LOUPE_SETTING).is_some_and(|v| v == "on")
}

/// The Panel title tag while the Loupe is on ("Loupe 200 %"), nothing while it is off.
pub fn tag(settings: &Settings) -> Option<String> {
    is_on(settings).then(|| format!("Loupe {} %", zoom(settings)))
}

/// The Loupe's size choice, for a Picture View's settings menu.
pub const SIZE_SETTING: SettingDef = SettingDef {
    key: "size",
    label: "Loupe size",
    amount: None,
    choices: &[Choice { value: "medium", text: "Medium" }, Choice { value: "small", text: "Small" }, Choice { value: "large", text: "Large" }],
};

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
struct LoupeState {
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

impl LoupeState {
    fn new() -> LoupeState {
        LoupeState { x: 0.5, y: 0.5, zoom: DEFAULT_ZOOM, size: LoupeSize::Medium, grab: None, wheel: 0.0 }
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
        vec![(X_KEY, format!("{:.5}", self.x)), (Y_KEY, format!("{:.5}", self.y))]
    }

    /// Take remembered state, leaving anything that is not sensible at its default.
    fn load(&mut self, state: &Settings) {
        let fraction = |key: &str| state.get(key).and_then(|v| v.parse::<f32>().ok()).filter(|v| (0.0..=1.0).contains(v));
        if let (Some(x), Some(y)) = (fraction(X_KEY), fraction(Y_KEY)) {
            (self.x, self.y) = (x, y);
        }
        self.zoom = zoom(state);
    }
}

/// The Loupe's own draw, built around the Panel's colour rule: a square with the source pixels under it, magnified.
const LOUPE_SHADER: &str = r#"
struct Params {
    quad: vec4<f32>,    // left, top, right, bottom in clip space
    loupe: vec4<f32>,   // left, top, side, device pixels per source pixel
    src: vec4<f32>,     // frame width, height, source pixel at the Loupe's middle (x, y)
    mid: vec4<f32>,     // the Loupe's middle pixel (x, y), bottom row first (> 0.5)
};
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> values: Values;

struct VertexOut { @builtin(position) position: vec4<f32> };

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    var out: VertexOut;
    out.position = vec4<f32>(mix(p.quad.x, p.quad.z, corner.x), mix(p.quad.y, p.quad.w, corner.y), 0.0, 1.0);
    return out;
}

// The exact texel under `uv` (the Loupe magnifies source pixels, so every sample is nearest).
fn load_picture(uv: vec2<f32>) -> vec3<f32> {
    let size = vec2<i32>(i32(p.src.x), i32(p.src.y));
    let at = clamp(vec2<i32>(uv * p.src.xy), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(picture, at, 0).rgb;
}
fn sample_picture(uv: vec2<f32>) -> vec3<f32> { return load_picture(uv); }

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    let pos = in.position.xy;
    let lo = p.loupe;
    let edge = min(min(pos.x - lo.x, lo.x + lo.z - pos.x), min(pos.y - lo.y, lo.y + lo.z - pos.y));
    if edge < 2.0 { return vec4<f32>(0.9, 0.9, 0.9, 1.0); }
    let s = p.src.zw + floor((floor(pos) - p.mid.xy) / lo.w);
    if s.x < 0.0 || s.y < 0.0 || s.x >= p.src.x || s.y >= p.src.y { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    var row = s.y;
    if p.mid.z > 0.5 { row = p.src.y - 1.0 - s.y; }
    return vec4<f32>(colour(vec2<f32>((s.x + 0.5) / p.src.x, (row + 0.5) / p.src.y)), 1.0);
}
"#;

const PARAMS_BYTES: u64 = 64;

struct Inner {
    state: LoupeState,
    /// Remembered state is taken once, from the first [`Loupe::configure`] (the user's file at start-up);
    /// after that the display's own is the newer.
    loaded: bool,
    /// The Panel's area and scale when last drawn or touched, for [`LoupeDrawn`].
    area: Option<([f32; 4], f32)>,
}

/// A Loupe. Cheap to clone: the Feed thread and the window thread may each hold one.
#[derive(Clone)]
pub struct Loupe {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    values: wgpu::Buffer,
    values_size: usize,
    /// The bind group for the frame's texture.
    bound: Arc<Mutex<Option<(u64, wgpu::BindGroup)>>>,
    inner: Arc<Mutex<Inner>>,
}

impl Loupe {
    /// `rule` is the Panel's colour rule (the `wgsl` it gave the stage, with its `Values` struct and `colour`
    /// function); `V` is the plain-data form of `Values`.
    pub fn new<V: Pod>(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, rule: &str) -> Loupe {
        let values_size = size_of::<V>();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("loupe"), source: wgpu::ShaderSource::Wgsl(format!("{rule}{LOUPE_SHADER}").into()) });
        let uniform = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("loupe"),
            entries: &[
                uniform(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                uniform(2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("loupe"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("loupe"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(target.into())], compilation_options: Default::default() }),
            multiview_mask: None,
            cache: None,
        });
        let buffer = |label, size: u64| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        Loupe {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout,
            params: buffer("loupe params", PARAMS_BYTES),
            values: buffer("loupe values", values_size as u64),
            values_size,
            bound: Arc::new(Mutex::new(None)),
            inner: Arc::new(Mutex::new(Inner { state: LoupeState::new(), loaded: false, area: None })),
        }
    }

    /// Take the size choice from `settings`, and, the first time only, the remembered position and zoom.
    pub fn configure(&self, settings: &Settings) {
        let mut inner = self.inner.lock().unwrap();
        inner.state.size = LoupeSize::from_setting(settings.get("size").map(String::as_str));
        if !inner.loaded {
            inner.loaded = true;
            inner.state.load(settings);
        }
    }

    /// The zoom now, in percent.
    pub fn zoom(&self) -> u32 {
        self.inner.lock().unwrap().state.zoom
    }

    /// For tests: what the Loupe reports it drew, `None` before the first draw.
    pub fn drawn(&self) -> Option<LoupeDrawn> {
        let inner = self.inner.lock().unwrap();
        let (area, scale) = inner.area?;
        Some(LoupeDrawn { loupe: inner.state.rect(area, scale), zoom: inner.state.zoom })
    }

    /// Draw the Loupe on top of the picture the stage just drew in `area` (left, top, width, height, device pixels)
    /// on a `surface` of that size, with the same `values` the stage drew with.
    pub fn draw<V: Pod>(&self, pass: &mut wgpu::RenderPass<'_>, shown: &Shown, area: [f32; 4], surface: [f32; 2], scale: f32, values: &V) {
        assert_eq!(size_of::<V>(), self.values_size, "the values are the size the Loupe was made for");
        let mut inner = self.inner.lock().unwrap();
        inner.area = Some((area, scale));
        let Some(p) = place(area, shown.size, 0.0) else { return };
        let rect = inner.state.rect(area, scale);
        let middle = LoupeState::middle(rect);
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
            inner.state.zoom as f32 / 100.0,
            shown.size[0] as f32,
            shown.size[1] as f32,
            source[0],
            source[1],
            mx,
            my,
            if shown.bottom_up { 1.0 } else { 0.0 },
            0.0,
        ];
        self.queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        self.queue.write_buffer(&self.values, 0, bytemuck::bytes_of(values));
        let mut bound = self.bound.lock().unwrap();
        if bound.as_ref().is_none_or(|(id, _)| *id != shown.texture_id) {
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("loupe"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shown.texture.create_view(&Default::default())) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.values.as_entire_binding() },
                ],
            });
            *bound = Some((shown.texture_id, group));
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bound.as_ref().unwrap().1, &[]);
        pass.draw(0..4, 0..1);
    }

    /// Drag and wheel: the Loupe takes the pointer only over itself (or while dragged).
    pub fn pointer(&self, area: [f32; 4], scale: f32, event: Pointer) -> PointerResult {
        let mut inner = self.inner.lock().unwrap();
        inner.area = Some((area, scale));
        let state = &mut inner.state;
        let rect = state.rect(area, scale);
        match event {
            Pointer::Down(p) if LoupeState::contains(rect, p) => {
                let m = LoupeState::middle(rect);
                state.grab = Some([p[0] - m[0], p[1] - m[1]]);
                PointerResult { used: true, remember: Vec::new() }
            }
            Pointer::Move(p) => match state.grab {
                Some(g) => {
                    state.centre_at([p[0] - g[0], p[1] - g[1]], area, scale);
                    PointerResult { used: true, remember: state.position_state() }
                }
                None => PointerResult::default(),
            },
            Pointer::Up => match state.grab.take() {
                Some(_) => PointerResult { used: true, remember: state.position_state() },
                None => PointerResult::default(),
            },
            Pointer::Wheel { at, lines } if LoupeState::contains(rect, at) => {
                state.wheel += lines;
                let steps = state.wheel.trunc();
                state.wheel -= steps;
                let zoom = (state.zoom as i32 + steps as i32 * ZOOM_STEP as i32).clamp(MIN_ZOOM as i32, MAX_ZOOM as i32) as u32;
                // Nothing to remember when the zoom did not change (a part of a notch, or already at its limit).
                let changed = zoom != state.zoom;
                state.zoom = zoom;
                PointerResult { used: true, remember: if changed { vec![(ZOOM_KEY, zoom.to_string())] } else { Vec::new() } }
            }
            _ => PointerResult::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pairs: &[(&str, &str)]) -> Settings {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn the_tag_is_there_only_while_the_loupe_is_on_and_shows_a_sensible_zoom() {
        assert_eq!(tag(&with(&[])), None);
        assert_eq!(tag(&with(&[("loupe", "off"), (ZOOM_KEY, "400")])), None);
        assert_eq!(tag(&with(&[("loupe", "on")])).as_deref(), Some("Loupe 200 %"));
        assert_eq!(tag(&with(&[("loupe", "on"), (ZOOM_KEY, "500")])).as_deref(), Some("Loupe 500 %"));
        assert_eq!(tag(&with(&[("loupe", "on"), (ZOOM_KEY, "rubbish")])).as_deref(), Some("Loupe 200 %"));
        assert_eq!(tag(&with(&[("loupe", "on"), (ZOOM_KEY, "5000")])).as_deref(), Some("Loupe 200 %"));
    }
}
