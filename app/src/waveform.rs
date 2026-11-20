//! The luma waveform: a Scope that plots, for every column of the picture, how many of its pixels have each luma
//! value, on a 10-bit scale (codes 0 to 1023).
//!
//! Measured on the GPU from every pixel of every Feed frame, at full resolution, in 32-bit float: one compute
//! pass over the frame [`crate::gpu_frame`] already put on the GPU. Each waveform column is one picture column,
//! and holds 1026 counts: one per code value, plus a row for everything below code 0 and a row for everything
//! above code 1023, so out-of-range pixels are counted and shown, never cut off.
//!
//! The Colour Setup's Levels decide where values land on the scale (#18): video levels put black (0.0) at code 64
//! and white (1.0) at 940, full (data) levels at 0 and 1023. Resolve hands the Sender the same values either way.
use crate::colour_setup::Levels;
use crate::gpu_frame::GpuFrame;
use crate::gpu_memory::{self, Part};
use crate::measurement::{LUMA_SETTING, MeasuredSpace, Measurement, luma_weights_of};
use crate::scope_shader::{GRATICULE_AMOUNT, GRATICULE_SETTING, graticule_gain};
use crate::settings::{Amount, Choice, SettingDef};
use crate::waveform_scale::{UNIT_SETTING, Unit};
use std::sync::{Arc, Mutex};

/// Code values on the 10-bit scale: 0 to 1023.
pub const CODES: u32 = 1024;
/// Counts per waveform column: below 0, codes 0 to 1023, above 1023.
const ROWS: u32 = CODES + 2;

impl Levels {
    /// Where values land on the 10-bit scale: code = value × scale + offset. Inside Resolve every value is full
    /// range (0.0 black, 1.0 white) whatever the project's Levels; the Levels decide the codes those values are
    /// delivered as. Video levels: black 64, white 940. Full (data) levels: black 0, white 1023.
    pub fn scale_and_offset(self) -> (f64, f64) {
        match self {
            Levels::Video => (876.0, 64.0),
            Levels::Full => (1023.0, 0.0),
        }
    }

    /// The codes of black and white.
    pub fn black_and_white(self) -> (u32, u32) {
        match self {
            Levels::Video => (64, 940),
            Levels::Full => (0, 1023),
        }
    }

    /// The codes the scale names, top to bottom, besides its edge rows: black, middle and white for video
    /// levels; for full levels black and white are the plot's own edges, so the quarter lines instead.
    pub fn scale_marks(self) -> [u32; 3] {
        match self {
            Levels::Video => [940, 512, 64],
            Levels::Full => [768, 512, 256],
        }
    }

    /// The active range as the label shows it.
    pub fn range_label(self) -> &'static str {
        match self {
            Levels::Video => "Legal 64–940",
            Levels::Full => "Full 0–1023",
        }
    }
}

/// How bright the trace is drawn, in percent of the normal look (the trace brightness slider, #84).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brightness(pub f32);

impl Brightness {
    /// The gain in `1 - exp(-gain * density)`, where density is 1 for a trace spread evenly over every code:
    /// 0.25 at 100 %, the look before the slider (#53).
    fn gain(self) -> f32 {
        0.25 * (self.0 / 100.0)
    }
}

/// The trace brightness slider, the same on every scope that draws a trace: 100 % is the normal look. The lists
/// earlier versions saved (Dimmer, Normal, Brighter) map to the percentages that gave the same gains.
pub const BRIGHTNESS_AMOUNT: Amount = crate::scope_shader::trace_brightness_amount(
    300.0,
    &[
        ("normal", 100.0),
        ("dim", 48.0),
        ("dimmer", 48.0),
        ("brighter", 240.0),
    ],
);

/// How far the trace is spread over its neighbours, in device pixels (the soft spread slider, #85).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread(pub f32);

pub use crate::scope_shader::SPREAD_AMOUNT;

/// How the waveform is drawn. Drawing only: it never changes what is measured or what the label says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub brightness: Brightness,
    /// The soft spread, in device pixels (0 skips it), done in a separate pass into a texture.
    pub spread: Spread,
    /// How bright the grid and scale lines are, as a gain (1 is the normal look; the graticule brightness slider, #127).
    pub graticule: f32,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            brightness: Brightness(BRIGHTNESS_AMOUNT.default),
            spread: Spread(SPREAD_AMOUNT.default),
            graticule: 1.0,
        }
    }
}

/// Which channels the waveform draws, from its Y R G B chips: any combination, at least one (Luma by default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channels {
    pub y: bool,
    pub r: bool,
    pub g: bool,
    pub b: bool,
}

impl Default for Channels {
    fn default() -> Channels {
        Channels {
            y: true,
            r: false,
            g: false,
            b: false,
        }
    }
}

impl Channels {
    /// From the setting's value: the letters of the chips that are on ("y", "rb", ...). Anything with no chip on
    /// is the default.
    pub fn from_setting(value: Option<&str>) -> Channels {
        let on = |c: char| value.is_some_and(|v| v.contains(c));
        let channels = Channels {
            y: on('y'),
            r: on('r'),
            g: on('g'),
            b: on('b'),
        };
        if channels.any() {
            channels
        } else {
            Channels::default()
        }
    }

    pub fn any(&self) -> bool {
        self.y || self.r || self.g || self.b
    }

    /// Whether any of R, G and B is on: the frame then needs the second measuring pass.
    pub fn colour_channels(&self) -> bool {
        self.r || self.g || self.b
    }

    /// Bits 0 to 3: Y, R, G, B.
    fn mask(&self) -> u32 {
        self.y as u32 | (self.r as u32) << 1 | (self.g as u32) << 2 | (self.b as u32) << 3
    }

    /// Only Luma: what the title bar need not name.
    pub fn is_default(&self) -> bool {
        *self == Channels::default()
    }

    /// The channels on, named for a tag: "Y", "R G", "Y R G B".
    pub fn names(&self) -> String {
        [(self.y, "Y"), (self.r, "R"), (self.g, "G"), (self.b, "B")]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, n)| *n)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

const PARAMS_BYTES: u64 = 48;

const MEASURE_SHADER: &str = r#"
struct Params {
    weights: vec4<f32>,   // luma weights for R, G, B
    size: vec4<u32>,      // picture width, height
    levels: vec4<f32>,    // the Levels: code = luma * levels.x + levels.y
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var frame: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> counts: array<u32>;
@group(0) @binding(3) var<storage, read_write> sums: array<atomic<u32>>;   // the colour pass only: R, G, B per count
@group(0) @binding(4) var<storage, read_write> rgb_counts: array<u32>;     // the channel pass only: R, G, B counts, one block each

const ROWS: u32 = 1026u;
const COLUMNS: u32 = 2u;    // picture columns per workgroup
const LANES: u32 = 128u;    // threads per picture column

// The count row a pixel lands in: below code 0 (and NaN: not a value at all), codes 0 to 1023, above code 1023.
fn row_of_code(code: f32) -> u32 {
    if (code >= 1023.5) {
        return ROWS - 1u;
    } else if (code >= -0.5) {
        return u32(code + 0.5) + 1u;
    }
    return 0u;
}
fn row_of(rgb: vec3<f32>) -> u32 {
    return row_of_code(dot(rgb, params.weights.xyz) * params.levels.x + params.levels.y);
}
var<workgroup> local_counts: array<atomic<u32>, 2052>;   // COLUMNS * ROWS

// Each workgroup owns two whole picture columns: it counts them in workgroup memory, then writes every count of
// its columns (zeros too), so the counts buffer needs no clearing between frames.
@compute @workgroup_size(2, 128)
fn measure(@builtin(local_invocation_id) local: vec3<u32>, @builtin(local_invocation_index) index: u32, @builtin(workgroup_id) group: vec3<u32>) {
    for (var i = index; i < COLUMNS * ROWS; i += COLUMNS * LANES) {
        atomicStore(&local_counts[i], 0u);
    }
    workgroupBarrier();
    let x = group.x * COLUMNS + local.x;
    if (x < params.size.x) {
        for (var y = local.y; y < params.size.y; y += LANES) {
            let rgb = textureLoad(frame, vec2<u32>(x, y), 0).rgb;
            atomicAdd(&local_counts[local.x * ROWS + row_of(rgb)], 1u);
        }
    }
    workgroupBarrier();
    for (var i = index; i < COLUMNS * ROWS; i += COLUMNS * LANES) {
        let column = group.x * COLUMNS + i / ROWS;
        if (column < params.size.x) {
            counts[column * ROWS + i % ROWS] = atomicLoad(&local_counts[i]);
        }
    }
}

// The channel chips' pass: the same counting for each of R, G and B on its own (one workgroup row per channel,
// those the mask in size.z names), into one block of `rgb_counts` per channel. Every count is written, zeros too.
@compute @workgroup_size(2, 128)
fn measure_rgb(@builtin(local_invocation_id) local: vec3<u32>, @builtin(local_invocation_index) index: u32, @builtin(workgroup_id) group: vec3<u32>) {
    let channel = group.y;
    if (((params.size.z >> channel) & 1u) == 0u) {
        return;
    }
    for (var i = index; i < COLUMNS * ROWS; i += COLUMNS * LANES) {
        atomicStore(&local_counts[i], 0u);
    }
    workgroupBarrier();
    let x = group.x * COLUMNS + local.x;
    if (x < params.size.x) {
        for (var y = local.y; y < params.size.y; y += LANES) {
            let rgb = textureLoad(frame, vec2<u32>(x, y), 0).rgb;
            atomicAdd(&local_counts[local.x * ROWS + row_of_code(rgb[channel] * params.levels.x + params.levels.y)], 1u);
        }
    }
    workgroupBarrier();
    for (var i = index; i < COLUMNS * ROWS; i += COLUMNS * LANES) {
        let column = group.x * COLUMNS + i / ROWS;
        if (column < params.size.x) {
            rgb_counts[(channel * params.size.x + column) * ROWS + i % ROWS] = atomicLoad(&local_counts[i]);
        }
    }
}

// The Colour Waveform's second pass: the same pixels in the same rows as `measure` (one shared `row_of`), adding
// each pixel's colour, clamped to 0 to 1 and in 1/1023ths, to its row's sums. The sums buffer is cleared first.
@compute @workgroup_size(8, 8)
fn accumulate(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.size.x || id.y >= params.size.y) {
        return;
    }
    let rgb = textureLoad(frame, id.xy, 0).rgb;
    let fixed = vec3<u32>(round(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * 1023.0));
    let at = (id.x * ROWS + row_of(rgb)) * 3u;
    atomicAdd(&sums[at], fixed.x);
    atomicAdd(&sums[at + 1u], fixed.y);
    atomicAdd(&sums[at + 2u], fixed.z);
}
"#;

/// How many pixels of each picture column landed at each code value: the waveform's contents.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveformCounts {
    columns: u32,
    counts: Vec<u32>,
}

impl WaveformCounts {
    /// Waveform columns: one per picture column.
    pub fn columns(&self) -> u32 {
        self.columns
    }
    /// Pixels of picture column `column` (from the left) at 10-bit code `code` (0 to 1023).
    pub fn at(&self, column: u32, code: u32) -> u32 {
        assert!(code < CODES);
        self.counts[(column * ROWS + code + 1) as usize]
    }
    /// Pixels of `column` below code 0.
    pub fn below(&self, column: u32) -> u32 {
        self.counts[(column * ROWS) as usize]
    }
    /// Pixels of `column` above code 1023.
    pub fn above(&self, column: u32) -> u32 {
        self.counts[(column * ROWS + ROWS - 1) as usize]
    }
    /// Every pixel counted.
    pub fn total(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }
}

/// Where the waveform's parts go inside its panel, in pixels: a gutter on the left for the scale's numbers, the
/// plot of codes 0 to 1023, and an edge row above and below it for pixels over and under the scale.
/// Rectangles are left, top, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotLayout {
    plot: [f32; 4],
    above: [f32; 4],
    below: [f32; 4],
    gutter_left: f32,
}

/// Logical pixels.
const EDGE_ROW: f32 = 5.0;
const EDGE_GAP: f32 = 6.0;

impl PlotLayout {
    /// The layout inside `area` (left, top, width, height) at `scale` device pixels per logical pixel. The gutter
    /// keeps its full width (the scale's numbers stay readable however small the panel), so the plot is exactly
    /// the waveform's Panel display area.
    pub fn new([left, top, width, height]: [f32; 4], scale: f32) -> PlotLayout {
        let gutter = (crate::layout::GUTTER * scale)
            .round()
            .min((width - 1.0).max(0.0));
        let (row, gap) = ((EDGE_ROW * scale).round(), (EDGE_GAP * scale).round());
        let (l, w) = (left + gutter, (width - gutter).max(1.0));
        let plot_top = top + row + gap;
        let plot_height = (height - 2.0 * (row + gap)).max(1.0);
        PlotLayout {
            plot: [l, plot_top, w, plot_height],
            above: [l, top, w, row],
            below: [l, plot_top + plot_height + gap, w, row],
            gutter_left: left,
        }
    }
    /// The layout for a frame of `picture` (width, height) pixels: the plot has the frame's aspect, scaled to fit
    /// what [`PlotLayout::new`] would give it and centred in `area`, with the gutter on its left and an edge row
    /// above and below it. The rest of the area is left empty. A frame with no size fills the area.
    pub fn fitted(area: [f32; 4], scale: f32, picture: [u32; 2]) -> PlotLayout {
        let full = PlotLayout::new(area, scale);
        if picture[0] == 0 || picture[1] == 0 {
            return full;
        }
        let left = area[0];
        let [fl, ft, fw, fh] = full.plot;
        let fit = (fw / picture[0] as f32).min(fh / picture[1] as f32);
        let (w, h) = (
            (picture[0] as f32 * fit).round().clamp(1.0, fw),
            (picture[1] as f32 * fit).round().clamp(1.0, fh),
        );
        let (l, t) = ((fl + (fw - w) / 2.0).round(), (ft + (fh - h) / 2.0).round());
        let (row, gap) = (full.above[3], full.below[1] - (ft + fh));
        PlotLayout {
            plot: [l, t, w, h],
            above: [l, t - gap - row, w, row],
            below: [l, t + h + gap, w, row],
            gutter_left: l - (fl - left),
        }
    }
    /// Where the gutter for the scale's numbers starts.
    pub fn gutter_left(&self) -> f32 {
        self.gutter_left
    }
    /// The plot of codes 0 (bottom) to 1023 (top).
    pub fn plot(&self) -> [f32; 4] {
        self.plot
    }
    /// The edge row for pixels above code 1023.
    pub fn above(&self) -> [f32; 4] {
        self.above
    }
    /// The edge row for pixels below code 0.
    pub fn below(&self) -> [f32; 4] {
        self.below
    }
    /// The middle of the band where `code` is plotted.
    pub fn y_of_code(&self, code: u32) -> f32 {
        let [_, top, _, height] = self.plot;
        top + height - (code as f32 + 0.5) / CODES as f32 * height
    }
    /// The middle of where picture column `column` of `columns` is plotted.
    pub fn x_of_column(&self, column: u32, columns: u32) -> f32 {
        let [left, _, width, _] = self.plot;
        left + (column as f32 + 0.5) / columns as f32 * width
    }
}

/// Which of the scale's numbers to show so none overlaps another: `marks` are each number's height on screen
/// and its rank (lower matters more), and a number is kept when it is at least `line` away from every number of
/// the same or a higher rank kept before it. Ties go to the earlier mark.
pub fn marks_that_fit(marks: &[(f32, u32)], line: f32) -> Vec<bool> {
    let mut order: Vec<usize> = (0..marks.len()).collect();
    order.sort_by_key(|&i| marks[i].1);
    let mut shown = vec![false; marks.len()];
    for i in order {
        if (0..marks.len()).all(|j| !shown[j] || (marks[i].0 - marks[j].0).abs() >= line) {
            shown[i] = true;
        }
    }
    shown
}

const TRACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

const DRAW_UNIFORM_BYTES: u64 = 192;

/// The bit of `look.w` that says the traces are coloured (Colorise on, or the Colour Waveform). The shader's
/// `COLORISE_BIT` is the same bit: change them together.
const COLORISE_BIT: u32 = 4;

const DRAW_SHADER: &str = r#"
// The bit of look.w that says the traces are coloured; the same as COLORISE_BIT in Rust, change them together.
const COLORISE_BIT: u32 = 4u;

struct Placement {
    clip: vec4<f32>,      // the panel in clip space: left, top, right, bottom
    plot: vec4<f32>,      // rectangles in pixels: left, top, width, height
    above: vec4<f32>,
    below: vec4<f32>,
    info: vec4<f32>,      // waveform columns, picture height, device pixels per logical pixel
    legal: vec4<f32>,     // the codes of black and white for the Levels in use, the graticule gain
    look: vec4<f32>,      // the gain of the phosphor response, the soft spread in device pixels (0 is off), 1 for the Colour Waveform, the chips on (bits: Y R G B)
    red: vec4<f32>,       // the channels' trace colours
    green: vec4<f32>,
    blue: vec4<f32>,
    grid_a: vec4<f32>,    // the codes of the scale's reference lines (the unit in use names them); -1000 for none
    grid_b: vec4<f32>,
};
@group(0) @binding(0) var<uniform> placement: Placement;
@group(0) @binding(1) var<storage, read> counts: array<u32>;
@group(0) @binding(2) var trace: texture_2d<f32>;   // the plot's trace: brightness-weighted colour, brightness (main pass only)
@group(0) @binding(3) var<storage, read> sums: array<u32>;   // the Colour Waveform's colour sums, 3 per count
@group(0) @binding(4) var<storage, read> rgb_counts: array<u32>;   // the R, G and B counts, one block each

const ROWS: u32 = 1026u;
// The plot, grid and trace greys and the phosphor response are the scope shader library's.
const EDGE: vec3<f32> = vec3<f32>(0.085, 0.085, 0.085);
const LEGAL: vec3<f32> = vec3<f32>(0.30, 0.30, 0.30);

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    return vec4<f32>(mix(placement.clip.x, placement.clip.z, corner.x), mix(placement.clip.y, placement.clip.w, corner.y), 0.0, 1.0);
}

fn inside(p: vec2<f32>, r: vec4<f32>) -> bool {
    return p.x >= r.x && p.x < r.x + r.z && p.y >= r.y && p.y < r.y + r.w;
}

// The picture columns under screen column `x`: [first, end).
fn columns_at(x: f32) -> vec2<u32> {
    let n = placement.info.x;
    let first = clamp(floor((x - placement.plot.x) / placement.plot.z * n), 0.0, n - 1.0);
    let end = clamp(max(ceil((x + 1.0 - placement.plot.x) / placement.plot.z * n), first + 1.0), 1.0, n);
    return vec2<u32>(u32(first), u32(end));
}

// Mean pixels per picture column in rows [first_row, end_row) of the counts, over the picture columns under x.
fn count_at(channel: u32, column: u32, row: u32) -> u32 {
    if (channel == 0u) {
        return counts[column * ROWS + row];
    }
    return rgb_counts[((channel - 1u) * u32(placement.info.x) + column) * ROWS + row];
}

// Channel 0 is Y (the luma counts), 1 to 3 are R, G, B.
fn density(channel: u32, x: f32, first_row: u32, end_row: u32) -> f32 {
    let columns = columns_at(x);
    let step = max(1u, (columns.y - columns.x) / 64u);   // a very narrow panel samples the columns
    var sum = 0.0;
    var sampled = 0.0;
    for (var c = columns.x; c < columns.y; c += step) {
        for (var r = first_row; r < end_row; r++) {
            sum += f32(count_at(channel, c, r));
        }
        sampled += 1.0;
    }
    return sum / sampled;
}

// The colour of the trace over rows [first_row, end_row) at screen column x: the neutral trace for the luma
// waveform; for the Colour Waveform the average colour of the pixels counted there, brought to the neutral
// trace's strength (its strongest channel is the neutral level), so grey pixels draw exactly the luma trace.
fn tint(x: f32, first_row: u32, end_row: u32) -> vec3<f32> {
    if (placement.look.z < 0.5) {
        return TRACE;
    }
    let columns = columns_at(x);
    let step = max(1u, (columns.y - columns.x) / 64u);
    var total = vec3<f32>(0.0);
    var n = 0.0;
    for (var c = columns.x; c < columns.y; c += step) {
        for (var r = first_row; r < end_row; r++) {
            let at = (c * ROWS + r) * 3u;
            total += vec3<f32>(f32(sums[at]), f32(sums[at + 1u]), f32(sums[at + 2u]));
            n += f32(counts[c * ROWS + r]);
        }
    }
    let average = total / max(n, 1.0);
    let top = max(average.x, max(average.y, average.z));
    if (top <= 0.0) {
        return TRACE;
    }
    return average / top * TRACE.x;
}

fn channel_on(channel: u32) -> bool {
    return ((u32(placement.look.w) >> channel) & 1u) == 1u;
}

// The trace colour of a channel: with Colorise on Y is tinted by the pixels' colour and R, G, B take their channel
// colours (look.w bit COLORISE_BIT); off, every trace is neutral.
fn channel_colour(channel: u32, x: f32, first_row: u32, end_row: u32) -> vec3<f32> {
    if (channel == 0u) {
        return tint(x, first_row, end_row);
    } else if (((u32(placement.look.w) >> COLORISE_BIT) & 1u) == 0u) {
        return TRACE;   // Colorise off: every trace is neutral
    } else if (channel == 1u) {
        return placement.red.xyz;
    } else if (channel == 2u) {
        return placement.green.xyz;
    }
    return placement.blue.xyz;
}

// What the chips that are on draw over rows [first_row, end_row) at screen column x, overlaid on one scale: the sum
// of each channel's colour at its own phosphor brightness (xyz), and the strongest of the brightnesses (w).
fn overlay(x: f32, first_row: u32, end_row: u32, expected: f32, floor_level: f32) -> vec4<f32> {
    var rgb = vec3<f32>(0.0);
    var strongest = 0.0;
    for (var channel = 0u; channel < 4u; channel++) {
        if (!channel_on(channel)) {
            continue;
        }
        let brightness = phosphor(placement.look.x, density(channel, x, first_row, end_row), expected, floor_level);
        if (brightness > 0.0) {
            rgb += brightness * channel_colour(channel, x, first_row, end_row);
            strongest = max(strongest, brightness);
        }
    }
    return vec4<f32>(min(rgb, vec3<f32>(1.0)), strongest);
}

fn y_of_code(code: f32) -> f32 {
    return placement.plot.y + placement.plot.w - (code + 0.5) / 1024.0 * placement.plot.w;
}

fn on_line(y: f32, code: f32) -> bool {
    return floor(y_of_code(code)) == y;
}

fn on_grid(y: f32) -> bool {
    for (var i = 0; i < 4; i++) {
        if (on_line(y, placement.grid_a[i]) || on_line(y, placement.grid_b[i])) {
            return true;
        }
    }
    return false;
}

// The plot's trace: the expected density for a screen row is the pixels a column would hold in that many codes
// if its tones were spread evenly over all 1024 (picture height / 1024 per code).
fn trace_at(p: vec2<f32>) -> vec4<f32> {
    let bottom = placement.plot.y + placement.plot.w;
    let first = u32(clamp(floor((bottom - p.y - 1.0) / placement.plot.w * 1024.0), 0.0, 1023.0));
    let end = u32(clamp(max(ceil((bottom - p.y) / placement.plot.w * 1024.0), f32(first) + 1.0), 1.0, 1024.0));
    let expected = placement.info.y * f32(end - first) / 1024.0;
    let drawn = overlay(p.x, first + 1u, end + 1u, expected, 0.08);
    return vec4<f32>(drawn.xyz, drawn.w);
}

@vertex
fn vs_trace(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    return vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
}

// Pass 1: the trace's brightness, one value per device pixel of the plot, into a texture.
@fragment
fn fs_trace(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return trace_at(floor(position.xy) + floor(placement.plot.xy));
}

// Pass 2 reads it back, spread by up to 3 device pixels (the soft spread amount; 0 reads the pixel as it is). At 1
// pixel the weights are 0.6 for the pixel and 0.1 for each of its four neighbours; each further pixel of the amount
// adds a ring of neighbours one pixel out at half that weight, and a part of a pixel adds that part of the weight.
fn tap(at: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return textureLoad(trace, clamp(at, vec2<i32>(0), size - vec2<i32>(1)), 0);
}

fn soft(q: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(trace));
    let at = vec2<i32>(q);
    let centre = tap(at, size);
    let amount = placement.look.y;
    if (amount <= 0.0) {
        return centre;
    }
    let near = clamp(amount, 0.0, 1.0);
    var sum = (0.6 + 0.4 * (1.0 - near)) * centre;
    sum += 0.1 * near * (tap(at + vec2<i32>(1, 0), size) + tap(at - vec2<i32>(1, 0), size) + tap(at + vec2<i32>(0, 1), size) + tap(at - vec2<i32>(0, 1), size));
    for (var d = 2; d <= 3; d++) {
        let part = clamp(amount - f32(d - 1), 0.0, 1.0);
        if (part > 0.0) {
            let o = vec2<i32>(d, 0);
            let p = vec2<i32>(0, d);
            sum += 0.05 * part * (tap(at + o, size) + tap(at - o, size) + tap(at + p, size) + tap(at - p, size));
            sum -= 0.2 * part * centre;
        }
    }
    return max(sum, centre);   // a faint stray pixel never fades below the floor
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let p = floor(position.xy);
    if (inside(p, placement.plot)) {
        var base = PLOT;
        if (on_grid(p.y)) {
            base = graticule(GRID, placement.legal.z);
        }
        if (on_line(p.y, placement.legal.x) || on_line(p.y, placement.legal.y)) {
            base = graticule(LEGAL, placement.legal.z);
        }
        // The codes under this screen row (bottom of the plot is code 0).
        let bottom = placement.plot.y + placement.plot.w;
        let first = u32(clamp(floor((bottom - p.y - 1.0) / placement.plot.w * 1024.0), 0.0, 1023.0));
        let end = u32(clamp(max(ceil((bottom - p.y) / placement.plot.w * 1024.0), f32(first) + 1.0), 1.0, 1024.0));
        let trace_here = soft(p - floor(placement.plot.xy));
        return vec4<f32>(base * (1.0 - trace_here.a) + trace_here.rgb, 1.0);
    }
    if (inside(p, placement.above)) {
        let drawn = overlay(p.x, ROWS - 1u, ROWS, placement.info.y / placement.plot.w, 0.8);
        return vec4<f32>(EDGE * (1.0 - drawn.w) + drawn.xyz, 1.0);
    }
    if (inside(p, placement.below)) {
        let drawn = overlay(p.x, 0u, 1u, placement.info.y / placement.plot.w, 0.8);
        return vec4<f32>(EDGE * (1.0 - drawn.w) + drawn.xyz, 1.0);
    }
    discard;
}
"#;

#[derive(Clone)]
struct Counts {
    buffer: wgpu::Buffer,
    columns: u32,
    /// Picture rows: the most pixels one column can hold.
    picture_height: u32,
}

/// The plot's trace brightness, drawn in its own pass (one value per device pixel of the plot) and read by the
/// main pass, which spreads it when asked.
struct Trace {
    view: wgpu::TextureView,
    size: [u32; 2],
}

struct State {
    counts: Option<Counts>,
    /// The Colour Waveform's colour sums (R, G, B per count), as big as the counts; a token buffer otherwise.
    sums: Option<wgpu::Buffer>,
    bound_colour: Option<(u64, wgpu::BindGroup)>,
    bound: Option<(u64, wgpu::BindGroup)>,
    draw_bound: Option<wgpu::BindGroup>,
    trace: Option<Trace>,
    trace_bound: Option<wgpu::BindGroup>,
    /// The R, G and B counts (three blocks as big as the counts), while any of those chips is on.
    rgb: Option<wgpu::Buffer>,
    rgb_bound: Option<(u64, wgpu::BindGroup)>,
}

/// Short of GPU memory: let every frame-sized buffer go, so the waveform draws nothing (the notice says why) and
/// the next frame makes them all again.
fn forget(state: &mut State) {
    state.counts = None;
    state.sums = None;
    state.rgb = None;
    state.bound = None;
    state.bound_colour = None;
    state.rgb_bound = None;
    state.draw_bound = None;
    state.trace_bound = None;
}

/// The waveform's GPU state. Cheap to clone: the Feed thread measures through one clone while the window thread
/// draws through another.
#[derive(Clone)]
pub struct Waveform {
    device: wgpu::Device,
    queue: wgpu::Queue,
    measure: wgpu::ComputePipeline,
    /// The Colour Waveform's second pass.
    accumulate: wgpu::ComputePipeline,
    /// The channel chips' pass: R, G and B counted on their own.
    measure_rgb: wgpu::ComputePipeline,
    /// Colour: the trace is tinted by the average colour of the pixels counted at each spot.
    colour: bool,
    /// The Colour Waveform counts nothing itself: it reads the counts the luma waveform made of the same frame
    /// (same pixels, same weights, same Levels) and adds only its colour sums.
    source: Option<Arc<Mutex<State>>>,
    params: wgpu::Buffer,
    draw: wgpu::RenderPipeline,
    trace_pass: wgpu::RenderPipeline,
    placement: wgpu::Buffer,
    measurement: Arc<Mutex<Measurement>>,
    look: Arc<Mutex<Look>>,
    channels: Arc<Mutex<Channels>>,
    /// Colorise (ADR 0032): the Y trace takes the colour of the pixels and R, G and B their channel colours; off,
    /// every trace is neutral. A drawing choice: the counts are the same either way.
    colorise: Arc<Mutex<bool>>,
    /// What the scale is written in, as chosen (#125); [`Waveform::unit`] is the one in use.
    unit: Arc<Mutex<Unit>>,
    state: Arc<Mutex<State>>,
}

impl Waveform {
    /// `target`: the format of what [`Waveform::draw`] draws into.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: wgpu::TextureFormat,
    ) -> Waveform {
        Waveform::build(device, queue, target, false)
    }

    /// The Colour Waveform: the same plot, tinted by the average colour of the pixels counted at each spot (#59).
    /// It shares the counts of `luma`, the luma waveform, which must measure each frame first (as the displays are
    /// handed a frame in order) and be configured with the same Colour Setup and luma weights.
    pub fn new_colour(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: wgpu::TextureFormat,
        luma: &Waveform,
    ) -> Waveform {
        let mut colour = Waveform::build(device, queue, target, true);
        colour.source = Some(luma.state.clone());
        colour
    }

    fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: wgpu::TextureFormat,
        colour: bool,
    ) -> Waveform {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("waveform measure"),
            source: wgpu::ShaderSource::Wgsl(MEASURE_SHADER.into()),
        });
        let measure = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("waveform measure"),
            layout: None,
            module: &shader,
            entry_point: Some("measure"),
            compilation_options: Default::default(),
            cache: None,
        });
        let accumulate = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("waveform colour"),
            layout: None,
            module: &shader,
            entry_point: Some("accumulate"),
            compilation_options: Default::default(),
            cache: None,
        });
        let measure_rgb = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("waveform channels"),
            layout: None,
            module: &shader,
            entry_point: Some("measure_rgb"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform params"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("waveform draw"),
            source: wgpu::ShaderSource::Wgsl(crate::scope_shader::with_library(DRAW_SHADER).into()),
        });
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("waveform draw"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
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
        let trace_pass = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("waveform trace"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_trace"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_trace"),
                targets: &[Some(TRACE_FORMAT.into())],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let placement = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform placement"),
            size: DRAW_UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Waveform {
            device: device.clone(),
            queue: queue.clone(),
            measure,
            accumulate,
            measure_rgb,
            colour,
            source: None,
            params,
            draw,
            trace_pass,
            placement,
            measurement: Arc::new(Mutex::new(Measurement::default())),
            look: Arc::new(Mutex::new(Look::default())),
            channels: Arc::new(Mutex::new(Channels::default())),
            colorise: Arc::new(Mutex::new(false)),
            unit: Arc::new(Mutex::new(Unit::Codes)),
            state: Arc::new(Mutex::new(State {
                counts: None,
                sums: None,
                bound_colour: None,
                bound: None,
                draw_bound: None,
                trace: None,
                trace_bound: None,
                rgb: None,
                rgb_bound: None,
            })),
        }
    }

    /// What to measure in from the next frame on (from the Colour Setup, or the user's choice of luma weights).
    pub fn set_measurement(&self, measurement: Measurement) {
        *self.measurement.lock().unwrap() = measurement;
    }

    /// How to draw from the next draw on (brightness and soft spread; never what is measured).
    pub fn set_look(&self, look: Look) {
        *self.look.lock().unwrap() = look;
    }

    /// Which channels to measure and draw from the next frame on (the Colour Waveform only ever shows Luma).
    pub fn set_channels(&self, channels: Channels) {
        *self.channels.lock().unwrap() = if self.colour {
            Channels::default()
        } else {
            channels
        };
    }

    pub fn channels(&self) -> Channels {
        *self.channels.lock().unwrap()
    }

    /// Colorise on or off from the next frame on (ADR 0032). Only the luma waveform reads it: the Colour Waveform is
    /// always tinted, whatever this says.
    pub fn set_colorise(&self, on: bool) {
        *self.colorise.lock().unwrap() = on;
    }

    /// The unit the scale is written in, as chosen (#125). Drawing only: nothing measured changes.
    pub fn set_unit(&self, unit: Unit) {
        *self.unit.lock().unwrap() = unit;
    }

    /// The unit in use: the chosen one, with nits falling back to 10-bit codes where the Output Colour Space has no light.
    pub fn unit(&self) -> Unit {
        let m = self.measurement();
        (*self.unit.lock().unwrap()).effective(m.space, m.assumed)
    }

    pub fn colorise(&self) -> bool {
        *self.colorise.lock().unwrap()
    }

    /// Whether the colour sums are needed, which also tells the draw pass to tint Y: the Colour Waveform always, the
    /// luma waveform with Colorise on and Y showing.
    fn needs_colour_sums(&self) -> bool {
        self.colour || (self.colorise() && self.channels().y)
    }

    pub fn measurement(&self) -> Measurement {
        *self.measurement.lock().unwrap()
    }

    /// The quiet label: the scale, the Measured Space and the luma weights in use.
    pub fn label(&self) -> String {
        self.measurement().label()
    }

    /// One of the waveform's frame-sized buffers, or None when the GPU has no memory for it ([`gpu_memory`]).
    fn buffer(&self, label: &str, size: u64, usage: wgpu::BufferUsages) -> Option<wgpu::Buffer> {
        gpu_memory::allocate(&self.device, Part::Scopes, size, || {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        })
    }

    /// Record the measurement of `frame` into `encoder` (see [`crate::gpu_frame::FrameUploader::upload`]).
    pub fn measure(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        let measurement = self.measurement();
        let [r, g, b] = measurement.weights();
        let (scale, offset) = measurement.levels.scale_and_offset();
        let mut params = [0u8; PARAMS_BYTES as usize];
        params[..16].copy_from_slice(bytemuck::cast_slice(&[r as f32, g as f32, b as f32, 0.0]));
        let channels = self.channels();
        params[16..28].copy_from_slice(bytemuck::cast_slice(&[
            frame.width,
            frame.height,
            channels.mask() >> 1,
        ]));
        params[32..40].copy_from_slice(bytemuck::cast_slice(&[scale as f32, offset as f32]));
        self.queue.write_buffer(&self.params, 0, &params);

        // A Colour Waveform takes the luma waveform's counts of this frame (taken before its own lock: the luma
        // waveform's state is never locked while this one is held). Nothing to do until they exist.
        let shared = match &self.source {
            Some(source) => match source.lock().unwrap().counts.clone() {
                Some(counts) if counts.columns == frame.width => Some(counts),
                _ => return,
            },
            None => None,
        };
        let mut state = self.state.lock().unwrap();
        if state
            .counts
            .as_ref()
            .is_none_or(|c| c.columns != frame.width)
        {
            let counts = match shared.clone() {
                Some(counts) => counts,
                None => {
                    let size = frame.width as u64 * ROWS as u64 * 4;
                    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
                    let Some(buffer) = self.buffer("waveform counts", size, usage) else {
                        return forget(&mut state);
                    };
                    Counts {
                        buffer,
                        columns: frame.width,
                        picture_height: frame.height,
                    }
                }
            };
            state.counts = Some(counts);
            state.bound = None;
            state.draw_bound = None;
            state.trace_bound = None;
        }
        state.counts.as_mut().unwrap().picture_height = frame.height;
        // The colour sums exist only while the Y trace is tinted; a token buffer keeps the draw shader's bindings
        // whole otherwise.
        let sums_size = if self.needs_colour_sums() {
            frame.width as u64 * ROWS as u64 * 12
        } else {
            16
        };
        if state.sums.as_ref().is_none_or(|b| b.size() != sums_size) {
            let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
            let Some(sums) = self.buffer("waveform colour sums", sums_size, usage) else {
                return forget(&mut state);
            };
            state.sums = Some(sums);
            state.bound_colour = None;
            state.draw_bound = None;
            state.trace_bound = None;
        }
        if let Some(shared) = shared {
            // The same buffer as the luma waveform's, unless it was made again since.
            state.counts = Some(shared);
            return self.measure_colour(&mut state, encoder, frame);
        }
        if state
            .bound
            .as_ref()
            .is_none_or(|(id, _)| *id != frame.texture_id)
        {
            let view = frame.texture.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("waveform measure"),
                layout: &self.measure.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: state.counts.as_ref().unwrap().buffer.as_entire_binding(),
                    },
                ],
            });
            state.bound = Some((frame.texture_id, bind_group));
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("waveform measure"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.measure);
        pass.set_bind_group(0, &state.bound.as_ref().unwrap().1, &[]);
        pass.dispatch_workgroups(frame.width.div_ceil(2), 1, 1);
        drop(pass);
        if self.needs_colour_sums() {
            self.measure_colour(&mut state, encoder, frame);
        }
        self.measure_channels(&mut state, encoder, frame, channels);
    }

    /// The R, G and B counts of this frame, for the chips that are on: one more pass over the frame, only while
    /// one of them is. Turning them all off frees the counts.
    fn measure_channels(
        &self,
        state: &mut State,
        encoder: &mut wgpu::CommandEncoder,
        frame: &GpuFrame,
        channels: Channels,
    ) {
        if !channels.colour_channels() {
            if state.rgb.take().is_some() {
                state.rgb_bound = None;
                state.draw_bound = None;
                state.trace_bound = None;
            }
            return;
        }
        let size = frame.width as u64 * ROWS as u64 * 3 * 4;
        if state.rgb.as_ref().is_none_or(|b| b.size() != size) {
            let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
            let Some(rgb) = self.buffer("waveform channel counts", size, usage) else {
                return forget(state);
            };
            state.rgb = Some(rgb);
            state.rgb_bound = None;
            state.draw_bound = None;
            state.trace_bound = None;
        }
        if state
            .rgb_bound
            .as_ref()
            .is_none_or(|(id, _)| *id != frame.texture_id)
        {
            let view = frame.texture.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("waveform channels"),
                layout: &self.measure_rgb.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: state.rgb.as_ref().unwrap().as_entire_binding(),
                    },
                ],
            });
            state.rgb_bound = Some((frame.texture_id, bind_group));
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("waveform channels"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.measure_rgb);
        pass.set_bind_group(0, &state.rgb_bound.as_ref().unwrap().1, &[]);
        pass.dispatch_workgroups(frame.width.div_ceil(2), 3, 1);
    }

    /// The Colour Waveform's second pass: the same pixels into the same rows as the counts, adding their colours.
    fn measure_colour(
        &self,
        state: &mut State,
        encoder: &mut wgpu::CommandEncoder,
        frame: &GpuFrame,
    ) {
        // The sums start from zero.
        let sums = state.sums.clone().unwrap();
        encoder.clear_buffer(&sums, 0, None);
        if state
            .bound_colour
            .as_ref()
            .is_none_or(|(id, _)| *id != frame.texture_id)
        {
            let view = frame.texture.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("waveform colour"),
                layout: &self.accumulate.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: sums.as_entire_binding(),
                    },
                ],
            });
            state.bound_colour = Some((frame.texture_id, bind_group));
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("waveform colour"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.accumulate);
        pass.set_bind_group(0, &state.bound_colour.as_ref().unwrap().1, &[]);
        pass.dispatch_workgroups(frame.width.div_ceil(8), frame.height.div_ceil(8), 1);
    }

    /// Draw the waveform into `area` (left, top, width, height in pixels of a `surface`-sized target), laid out as
    /// [`PlotLayout::new`] says. Draws nothing until a frame has been measured.
    pub fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        area: [f32; 4],
        surface: [f32; 2],
        scale: f32,
    ) {
        let mut state = self.state.lock().unwrap();
        let Some(counts) = &state.counts else { return };
        let [left, top, width, height] = area;
        if width < 1.0 || height < 1.0 {
            return;
        }
        let (columns, picture_height) = (counts.columns, counts.picture_height);
        let layout = PlotLayout::fitted(area, scale, [columns, picture_height]);
        // The trace texture covers the plot, one texel per device pixel.
        let trace_size = [
            layout.plot[2].ceil().max(1.0) as u32,
            layout.plot[3].ceil().max(1.0) as u32,
        ];
        if state.trace.as_ref().is_none_or(|t| t.size != trace_size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("waveform trace"),
                size: wgpu::Extent3d {
                    width: trace_size[0],
                    height: trace_size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TRACE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            state.trace = Some(Trace {
                view: texture.create_view(&Default::default()),
                size: trace_size,
            });
            state.trace_bound = None;
            state.draw_bound = None;
        }
        if state.draw_bound.is_none() {
            let counts = state.counts.as_ref().unwrap();
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("waveform draw"),
                layout: &self.draw.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.placement.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: counts.buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &state.trace.as_ref().unwrap().view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: state.sums.as_ref().unwrap().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: state
                            .rgb
                            .as_ref()
                            .or(state.sums.as_ref())
                            .unwrap()
                            .as_entire_binding(),
                    },
                ],
            });
            state.draw_bound = Some(bind_group);
        }
        if state.trace_bound.is_none() {
            let counts = state.counts.as_ref().unwrap();
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("waveform trace"),
                layout: &self.trace_pass.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.placement.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: counts.buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: state.sums.as_ref().unwrap().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: state
                            .rgb
                            .as_ref()
                            .or(state.sums.as_ref())
                            .unwrap()
                            .as_entire_binding(),
                    },
                ],
            });
            state.trace_bound = Some(bind_group);
        }
        let clip = |px: f32, size: f32| px / size * 2.0 - 1.0;
        let look = *self.look.lock().unwrap();
        let mut placement = [0f32; (DRAW_UNIFORM_BYTES / 4) as usize];
        placement[..4].copy_from_slice(&[
            clip(left, surface[0]),
            -clip(top, surface[1]),
            clip(left + width, surface[0]),
            -clip(top + height, surface[1]),
        ]);
        placement[4..8].copy_from_slice(&layout.plot);
        placement[8..12].copy_from_slice(&layout.above);
        placement[12..16].copy_from_slice(&layout.below);
        placement[16..19].copy_from_slice(&[columns as f32, picture_height as f32, scale]);
        let (black, white) = self.measurement().levels.black_and_white();
        placement[20..23].copy_from_slice(&[black as f32, white as f32, look.graticule]);
        let channels = self.channels();
        let mask = if state.rgb.is_some() || !channels.colour_channels() {
            channels.mask()
        } else {
            channels.mask() & 1
        };
        placement[24..28].copy_from_slice(&[
            look.brightness.gain(),
            look.spread.0,
            if self.needs_colour_sums() { 1.0 } else { 0.0 },
            (mask
                | if self.colour || self.colorise() {
                    1 << COLORISE_BIT
                } else {
                    0
                }) as f32,
        ]);
        for (i, colour) in [
            crate::style::CHANNEL_R,
            crate::style::CHANNEL_G,
            crate::style::CHANNEL_B,
        ]
        .into_iter()
        .enumerate()
        {
            placement[28 + i * 4..31 + i * 4].copy_from_slice(&colour);
        }
        // The reference lines of the scale's unit (at most 8): the plot is the same, only where the lines sit differs.
        let measurement = self.measurement();
        let unit = self.unit();
        let mut lines = [-1000f32; 8];
        for (slot, code) in lines
            .iter_mut()
            .zip(unit.lines(measurement.levels, measurement.space))
        {
            *slot = code as f32;
        }
        placement[40..48].copy_from_slice(&lines);
        self.queue
            .write_buffer(&self.placement, 0, bytemuck::cast_slice(&placement));

        // Pass 1, on its own: the trace's brightness into the texture. It is submitted ahead of the caller's
        // commands, which read the texture.
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("waveform trace"),
            });
        {
            let mut trace_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("waveform trace"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &state.trace.as_ref().unwrap().view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            trace_pass.set_pipeline(&self.trace_pass);
            trace_pass.set_bind_group(0, state.trace_bound.as_ref().unwrap(), &[]);
            trace_pass.draw(0..4, 0..1);
        }
        self.queue.submit([encoder.finish()]);

        pass.set_pipeline(&self.draw);
        pass.set_bind_group(0, state.draw_bound.as_ref().unwrap(), &[]);
        pass.draw(0..4, 0..1);
    }

    /// The counts of one of R, G or B (as the chips that are on measured them), read back like [`Waveform::counts`].
    /// Empty until a frame has been measured with that chip on.
    pub fn channel_counts(&self, channel: crate::components::Channel) -> WaveformCounts {
        use crate::components::Channel;
        let block = match channel {
            Channel::R => 0,
            Channel::G => 1,
            Channel::B => 2,
            Channel::Y => return self.counts(),
        };
        let state = self.state.lock().unwrap();
        let (Some(counts), Some(rgb)) = (&state.counts, &state.rgb) else {
            return WaveformCounts {
                columns: 0,
                counts: Vec::new(),
            };
        };
        let bytes = counts.columns as u64 * ROWS as u64 * 4;
        let data = self.read_buffer(rgb, block * bytes, bytes);
        WaveformCounts {
            columns: counts.columns,
            counts: data,
        }
    }

    fn read_buffer(&self, buffer: &wgpu::Buffer, offset: u64, size: u64) -> Vec<u32> {
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, offset, &readback, 0, Some(size));
        self.queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the GPU finishes");

        bytemuck::cast_slice(&readback.get_mapped_range(..).expect("mapped")).to_vec()
    }

    /// Read the waveform's contents back from the GPU (waits for it): for tests and diagnostics, not per frame.
    pub fn counts(&self) -> WaveformCounts {
        let state = self.state.lock().unwrap();
        let Some(counts) = &state.counts else {
            return WaveformCounts {
                columns: 0,
                counts: Vec::new(),
            };
        };
        let data = self.read_buffer(&counts.buffer, 0, counts.buffer.size());
        WaveformCounts {
            columns: counts.columns,
            counts: data,
        }
    }
}

/// What the luma waveform is called: its Panel's settings are kept under this name.
pub const NAME: &str = "Luma waveform";

/// The luma waveform's right-click menu: how Luma is weighted (ADR 0004). The label names the weights in use.
pub const SETTINGS: [SettingDef; 7] = [
    SettingDef {
        key: LUMA_SETTING,
        label: "Luma weights",
        amount: None,
        choices: &[
            Choice {
                value: "primaries",
                text: "From the Measured Space's primaries",
            },
            Choice {
                value: "match-resolve",
                text: "Match Resolve's waveform",
            },
        ],
    },
    SettingDef {
        key: BRIGHTNESS_SETTING,
        label: "Trace brightness",
        amount: Some(BRIGHTNESS_AMOUNT),
        choices: &[],
    },
    SettingDef {
        key: SPREAD_SETTING,
        label: "Soft spread",
        amount: Some(SPREAD_AMOUNT),
        choices: &[],
    },
    SettingDef {
        key: GRATICULE_SETTING,
        label: "Graticule brightness",
        amount: Some(GRATICULE_AMOUNT),
        choices: &[],
    },
    // Shown as the Y R G B chips on the title bar, not in the popover: the letters of the chips that are on.
    SettingDef {
        key: CHANNELS_SETTING,
        label: "Channels",
        amount: None,
        choices: &[
            Choice {
                value: "y",
                text: "Y",
            },
            Choice {
                value: "r",
                text: "R",
            },
            Choice {
                value: "g",
                text: "G",
            },
            Choice {
                value: "b",
                text: "B",
            },
            Choice {
                value: "yr",
                text: "Y R",
            },
            Choice {
                value: "yg",
                text: "Y G",
            },
            Choice {
                value: "yb",
                text: "Y B",
            },
            Choice {
                value: "rg",
                text: "R G",
            },
            Choice {
                value: "rb",
                text: "R B",
            },
            Choice {
                value: "gb",
                text: "G B",
            },
            Choice {
                value: "yrg",
                text: "Y R G",
            },
            Choice {
                value: "yrb",
                text: "Y R B",
            },
            Choice {
                value: "ygb",
                text: "Y G B",
            },
            Choice {
                value: "rgb",
                text: "R G B",
            },
            Choice {
                value: "yrgb",
                text: "Y R G B",
            },
        ],
    },
    // Shown as the Colorise chip on the title bar, not in the popover (ADR 0032). Off first: it is the default.
    SettingDef {
        key: COLORISE_SETTING,
        label: "Colorise",
        amount: None,
        choices: &[
            Choice {
                value: crate::toolbar::TOGGLE_OFF,
                text: "Off",
            },
            Choice {
                value: "on",
                text: "On",
            },
        ],
    },
    SCALE_DEF,
];

/// The scale's units (#125): 10-bit codes by default, percent of the Levels' range, or nits (only offered for PQ and HLG).
const SCALE_DEF: SettingDef = SettingDef {
    key: UNIT_SETTING,
    label: "Scale units",
    amount: None,
    choices: &[
        Choice {
            value: "codes",
            text: "10-bit codes",
        },
        Choice {
            value: "percent",
            text: "Percent",
        },
        Choice {
            value: "nits",
            text: "Nits",
        },
    ],
};

/// The same without nits, for an Output Colour Space that has no light to name.
const SCALE_DEF_NO_NITS: SettingDef = SettingDef {
    key: UNIT_SETTING,
    label: "Scale units",
    amount: None,
    choices: SCALE_DEF.choices.split_at(2).0,
};

/// The key of the Colorise setting: whether the traces are drawn in colour.
pub const COLORISE_SETTING: &str = "colorise";

/// The key of the channels setting: which of the Y R G B chips are on.
pub const CHANNELS_SETTING: &str = "channels";

/// The chips on the luma waveform's title bar: Y R G B, any combination, at least one on (#117).
pub static TOOLBAR: [crate::toolbar::ToolbarDef; 2] = [
    crate::toolbar::ToolbarDef {
        key: CHANNELS_SETTING,
        mode: crate::components::ChipMode::Multiple,
        chips: &[
            crate::toolbar::ToolbarChip {
                value: "y",
                text: "Y",
                tooltip: "Luma (Y)",
                channel: Some(crate::components::Channel::Y),
            },
            crate::toolbar::ToolbarChip {
                value: "r",
                text: "R",
                tooltip: "Red",
                channel: Some(crate::components::Channel::R),
            },
            crate::toolbar::ToolbarChip {
                value: "g",
                text: "G",
                tooltip: "Green",
                channel: Some(crate::components::Channel::G),
            },
            crate::toolbar::ToolbarChip {
                value: "b",
                text: "B",
                tooltip: "Blue",
                channel: Some(crate::components::Channel::B),
            },
        ],
    },
    crate::toolbar::ToolbarDef {
        key: COLORISE_SETTING,
        mode: crate::components::ChipMode::Toggle,
        chips: &[crate::toolbar::ToolbarChip {
            value: "on",
            text: "Col",
            tooltip: "Colorise: traces drawn in colour, Y in the picture's colour (off: white)",
            channel: None,
        }],
    },
];

/// What the Colour Waveform is called: its Panel's settings are kept under this name.
pub const COLOUR_NAME: &str = "Colour waveform";

/// The Colour Waveform's right-click menu: the luma waveform's drawing settings. It has no luma weights of its
/// own: it follows the luma waveform's, so the two always place a trace at the same code.
pub const COLOUR_SETTINGS: [SettingDef; 4] = [SETTINGS[1], SETTINGS[2], SETTINGS[3], SCALE_DEF];

/// The key of the trace brightness setting.
pub const BRIGHTNESS_SETTING: &str = "brightness";
/// The key of the soft spread setting.
pub const SPREAD_SETTING: &str = "spread";

/// How the waveform is drawn, from its settings (a value that is not a choice falls back to the default).
pub fn look_from(settings: &crate::settings::Settings) -> Look {
    let brightness =
        Brightness(BRIGHTNESS_AMOUNT.read(settings.get(BRIGHTNESS_SETTING).map(String::as_str)));
    Look {
        brightness,
        spread: Spread(SPREAD_AMOUNT.read(settings.get(SPREAD_SETTING).map(String::as_str))),
        graticule: graticule_gain(settings.get(GRATICULE_SETTING).map(String::as_str)),
    }
}

/// Logical pixels: the size of the numbers on the scale.
pub const SCALE_SIZE: f32 = crate::style::TYPE_SCALE;

impl Waveform {
    /// `label` with the unit of the scale when it is not 10-bit codes.
    fn with_unit(&self, label: String) -> String {
        match self.unit() {
            Unit::Codes => label,
            Unit::Percent => format!("{label}  ·  scale in % of the Levels' range"),
            Unit::Nits => format!(
                "{label}  ·  scale in nits ({})",
                if self.measurement().space == MeasuredSpace::Rec2100Hlg {
                    "HLG on a 1000 nit display"
                } else {
                    "PQ"
                }
            ),
        }
    }
}

impl crate::display::Display for Waveform {
    fn name(&self) -> &'static str {
        if self.colour { COLOUR_NAME } else { NAME }
    }

    fn on_frame(&self, encoder: &mut wgpu::CommandEncoder, frame: &GpuFrame) {
        self.measure(encoder, frame);
    }

    fn configure(&self, context: &crate::display::DisplayContext) {
        // The waveform measures in the Colour Setup's Output Colour Space (the default Measured Space), with the
        // luma weights the user chose (ADR 0004).
        // The Colour Waveform has no weights menu of its own: it follows the luma waveform's choice.
        let luma = if self.colour {
            context.luma
        } else {
            luma_weights_of(&context.settings)
        };
        self.set_look(look_from(&context.settings));
        self.set_channels(Channels::from_setting(
            context.settings.get(CHANNELS_SETTING).map(String::as_str),
        ));
        self.set_colorise(context.settings.get(COLORISE_SETTING).map(String::as_str) == Some("on"));
        self.set_unit(Unit::from_setting(
            context.settings.get(UNIT_SETTING).map(String::as_str),
        ));
        self.set_measurement(crate::output_conversion::measurement(
            context.colour_setup.clone(),
            luma,
        ));
    }

    fn setting_defs(&self) -> &'static [SettingDef] {
        if self.colour {
            &COLOUR_SETTINGS
        } else {
            &SETTINGS
        }
    }

    fn toolbar_defs(&self) -> &'static [crate::toolbar::ToolbarDef] {
        if self.colour { &[] } else { &TOOLBAR }
    }

    /// Nits are offered only when the Output Colour Space is PQ or HLG.
    fn offered_defs(&self) -> Vec<SettingDef> {
        let m = self.measurement();
        let offer_nits = crate::waveform_scale::nits_offered(m.space, m.assumed);
        self.setting_defs()
            .iter()
            .map(|d| {
                if d.key == UNIT_SETTING && !offer_nits {
                    SCALE_DEF_NO_NITS
                } else {
                    *d
                }
            })
            .collect()
    }

    fn label(&self) -> String {
        if self.colour {
            return self.with_unit(self.measurement().label_titled("Colour waveform"));
        }
        // The channels named when not just Luma; the luma weights only while Luma is drawn.
        let channels = self.channels();
        let title = if channels.is_default() {
            "Luma".to_string()
        } else {
            channels.names()
        };
        self.with_unit(self.measurement().label_for(&title, channels.y))
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, area: [f32; 4], surface: [f32; 2], scale: f32) {
        Waveform::draw(self, pass, area, surface, scale);
    }

    /// The codes the Levels name, and the edge rows, left out where they would overlap: black and white stay
    /// longest, then the edge rows, then the middle.
    fn scale_texts(&self, area: [f32; 4], scale: f32) -> Vec<crate::display::ScaleText> {
        let picture = match &self.state.lock().unwrap().counts {
            Some(counts) => [counts.columns, counts.picture_height],
            None => return Vec::new(),
        };
        let plot = PlotLayout::fitted(area, scale, picture);
        let (levels, space, unit) = (
            self.measurement().levels,
            self.measurement().space,
            self.unit(),
        );
        let middle = |r: [f32; 4]| r[1] + r[3] / 2.0;
        // (text, height on screen, rank: lower matters more)
        let mut marks = vec![(unit.above_text(levels, space), middle(plot.above()), 1)];
        marks.extend(
            unit.marks(levels, space)
                .into_iter()
                .map(|m| (m.text, plot.y_of_code(m.code), m.rank)),
        );
        marks.push((unit.below_text(levels), middle(plot.below()), 1));
        let placed: Vec<(f32, u32)> = marks.iter().map(|(_, y, rank)| (*y, *rank)).collect();
        let shown = marks_that_fit(&placed, SCALE_SIZE * scale * 1.3);
        let width = ((crate::layout::GUTTER - 6.0) * scale).max(1.0);
        marks
            .into_iter()
            .zip(shown)
            .filter(|(_, shown)| *shown)
            .map(|((text, top, _), _)| crate::display::ScaleText {
                text,
                left: plot.gutter_left(),
                top,
                width,
                right_aligned: true,
                centred: false,
            })
            .collect()
    }
}
