//! The Channel view: one channel of the Output picture drawn as grey, for inspecting noise (spec #49, #55), and
//! the Loupe on top of it.
//!
//! Blue is the default (the noisiest channel of most digital cameras); red and green are in the right-click
//! menu. The Loupe on top of it is the shared one in `loupe.rs`, built from this view's colour rule.
use crate::display::{LoupeDrawn, Display, DisplayContext, Pointer, PointerResult};
use crate::loupe::{self, Loupe};
use crate::gpu_frame::GpuFrame;
use crate::picture_stage::{PictureStage, Rule};
use crate::output_conversion::Plan;
use crate::components::{Channel as ChipColour, ChipMode};
use crate::settings::{Choice, SettingDef};
use crate::toolbar::{ToolbarChip, ToolbarDef};
use std::sync::{Arc, Mutex};

pub static SETTINGS: [SettingDef; 2] = [
    SettingDef {
        key: "channel",
        label: "Channel",
        amount: None,
        choices: &[Choice { value: "blue", text: "Blue" }, Choice { value: "red", text: "Red" }, Choice { value: "green", text: "Green" }, Choice { value: "luma", text: "Luma" }],
    },
    loupe::SIZE_SETTING,
];

/// The channel as chips on the title bar: Y R G B, one at a time (Y is Luma as grey), the model control of the
/// Panel toolbar (#116).
pub static TOOLBAR: [ToolbarDef; 1] = [ToolbarDef {
    key: "channel",
    mode: ChipMode::Single,
    chips: &[
        ToolbarChip { value: "luma", text: "Y", tooltip: "Luma (Y)", channel: Some(ChipColour::Y) },
        ToolbarChip { value: "red", text: "R", tooltip: "Red", channel: Some(ChipColour::R) },
        ToolbarChip { value: "green", text: "G", tooltip: "Green", channel: Some(ChipColour::G) },
        ToolbarChip { value: "blue", text: "B", tooltip: "Blue", channel: Some(ChipColour::B) },
    ],
}];

/// Which channel is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    Blue,
    Red,
    Green,
    Luma,
}

impl Channel {
    fn from_setting(value: Option<&str>) -> Channel {
        match value {
            Some("red") => Channel::Red,
            Some("green") => Channel::Green,
            Some("luma") => Channel::Luma,
            _ => Channel::Blue,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Channel::Blue => "Blue",
            Channel::Red => "Red",
            Channel::Green => "Green",
            Channel::Luma => "Luma",
        }
    }
    /// The index the shader picks the channel by.
    fn index(self) -> f32 {
        match self {
            Channel::Red => 0.0,
            Channel::Green => 1.0,
            Channel::Blue => 2.0,
            Channel::Luma => 3.0,
        }
    }
}

/// The colour rule the stage draws the picture with: one channel as grey.
const RULE: &str = r#"
struct Values { channel: vec4<f32>, weights: vec4<f32> };   // x: the channel (0 R, 1 G, 2 B, 3 Luma); luma weights

fn colour(uv: vec2<f32>) -> vec3<f32> {
    let c = sample_picture(uv);
    var v = c.b;
    if values.channel.x < 0.5 { v = c.r; } else if values.channel.x < 1.5 { v = c.g; } else if values.channel.x > 2.5 { v = dot(c.rgb, values.weights.xyz); }
    return vec3<f32>(v);
}
"#;

struct State {
    channel: Channel,
    /// The luma weights the Luma choice uses: the waveform's, so a grey lines up with its trace.
    weights: [f32; 3],
    tail: String,
}

impl State {
    /// The values the colour rule reads, for the stage and the Loupe alike.
    fn values(&self) -> [[f32; 4]; 2] {
        [[self.channel.index(), 0.0, 0.0, 0.0], [self.weights[0], self.weights[1], self.weights[2], 0.0]]
    }
}

/// The Channel view. Cheap to clone: the Feed thread hands it frames while the window thread draws.
#[derive(Clone)]
pub struct ChannelView {
    stage: PictureStage,
    loupe: Loupe,
    state: Arc<Mutex<State>>,
}

impl ChannelView {
    /// For tests: what the Loupe reports it drew, `None` before the first draw.
    pub fn drawn(&self) -> Option<LoupeDrawn> {
        self.loupe.drawn()
    }

    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat, filterable: bool) -> ChannelView {
        let stage = PictureStage::new::<[[f32; 4]; 2]>(device, queue, target, filterable, Rule { label: "channel view", wgsl: RULE.into(), has_legend: false });
        // The Loupe is built from the same colour rule, so it shows what the Panel shows.
        let loupe = Loupe::new::<[[f32; 4]; 2]>(device, queue, target, RULE);
        ChannelView {
            stage,
            loupe,
            state: Arc::new(Mutex::new(State { channel: Channel::Blue, weights: crate::measurement::REC709_LUMA.map(|v| v as f32), tail: String::new() })),
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
        let [wr, wg, wb] = crate::output_conversion::measurement(context.colour_setup.clone(), context.luma).weights();
        state.weights = [wr as f32, wg as f32, wb as f32];
        self.loupe.configure(&context.settings);
        let conversion = Plan::for_setup(context.colour_setup.as_ref().map(|c| &c.setup));
        state.tail = format!("{}  ·  {}", context.colour_label, conversion.label());
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        &SETTINGS
    }

    fn toolbar_defs(&self) -> &'static [ToolbarDef] {
        &TOOLBAR
    }

    fn state_keys(&self) -> &'static [&'static str] {
        loupe::STATE_KEYS
    }

    fn label(&self) -> String {
        // The picture first, then the state: the order `draw` takes them in.
        let size = self.stage.size();
        let state = self.state.lock().unwrap();
        let head = match state.channel {
            Channel::Luma => "Luma as grey".to_string(),
            channel => format!("{} channel as grey", channel.name()),
        };
        match size {
            Some([w, h]) => format!("{head}  ·  {w} × {h}  ·  Loupe {} %  ·  {}", self.loupe.zoom(), state.tail),
            None => head,
        }
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        let Some(shown) = self.stage.shown() else { return };
        if area[2] < 1.0 || area[3] < 1.0 {
            return;
        }
        let values = self.state.lock().unwrap().values();
        self.stage.draw(pass, area, surface, &values, None);
        self.loupe.draw(pass, &shown, area, surface, scale, &values);
    }

    fn pointer(&self, area: [f32; 4], scale: f32, event: Pointer) -> PointerResult {
        self.loupe.pointer(area, scale, event)
    }
}
