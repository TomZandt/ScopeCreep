//! The style system's components (spec #113, #115): the parts every surface of the Window is built from. Each is
//! pure: a size and hit-area description in device pixels at a scale (so layout and input are testable without a
//! GPU or a window), and a drawing as a list of [`Prim`]s (flat rectangles and text), in every state. Colours and
//! sizes come only from the tokens in `style.rs`; the style check covers this file. A legend's colours are data
//! the display passes in, never style.
//!
//! The drawing is renderer-neutral: the app turns the prims into flat rectangles and glyphon text
//! (`main.rs`), and the showcase example (`examples/style_showcase.rs`) draws every component in every state to
//! an image.
use crate::layout_tree::Edge;
use crate::panel_grid::PanelLayout;
use crate::settings::{Menu, MenuRow};
use crate::style::{self, Rgb};

/// A piece of the drawing.
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    /// A flat rectangle: left, top, width, height, and rgba.
    Rect { rect: [f32; 4], colour: [f32; 4] },
    /// Text in `face` and `colour`, set inside `rect` (left, top, width, height): aligned across it and centred
    /// down it, clipped to it.
    Text {
        text: String,
        rect: [f32; 4],
        face: Face,
        colour: [u8; 3],
        align: Align,
    },
}

/// The type scale's faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Notice,
    Label,
    Scale,
    /// Label-sized emphasis: the focused Panel name, headings (DM Mono Medium, #238).
    Strong,
    /// The wordmark and display headings, in Anton (#238). Never small chrome.
    Wordmark,
    /// An icon glyph, in the icon font (#238, ADR 0046).
    Icon,
}

impl Face {
    /// Size and line height (as a multiple of it), in logical pixels.
    pub fn metrics(self) -> (f32, f32) {
        match self {
            Face::Notice => (style::TYPE_NOTICE, style::TYPE_NOTICE_LINE),
            Face::Label => (style::TYPE_LABEL, style::TYPE_LABEL_LINE),
            Face::Scale => (style::TYPE_SCALE, style::TYPE_SCALE_LINE),
            Face::Strong => (style::TYPE_LABEL, style::TYPE_LABEL_LINE),
            Face::Wordmark => (style::TYPE_WORDMARK, style::TYPE_WORDMARK_LINE),
            Face::Icon => (style::TYPE_ICON, style::TYPE_ICON_LINE),
        }
    }

    /// The font role of this face: every face is DM Mono (the interface font) except the wordmark's Anton (#238).
    pub fn font(self) -> style::FontRole {
        match self {
            Face::Notice | Face::Label | Face::Scale => style::FontRole::Interface,
            Face::Strong => style::FontRole::Emphasis,
            Face::Wordmark => style::FontRole::Wordmark,
            Face::Icon => style::FontRole::Icon,
        }
    }
}

/// The icons (ADR 0046): Lucide glyphs from the bundled, subset icon font. This is the one place that maps an icon to
/// its codepoint; to add one, extend the subset and the table in `app/assets/fonts/README.md`, then add a variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Plus,
    ChevronUp,
    ChevronDown,
    ChevronRight,
    /// Reset (Lucide's rotate-ccw).
    Reset,
    /// The settings button (Lucide's ellipsis).
    Ellipsis,
    Close,
}

impl Icon {
    pub const ALL: [Icon; 7] = [
        Icon::Plus,
        Icon::ChevronUp,
        Icon::ChevronDown,
        Icon::ChevronRight,
        Icon::Reset,
        Icon::Ellipsis,
        Icon::Close,
    ];

    /// The glyph's codepoint in the icon font (a private-use character).
    pub fn glyph(self) -> char {
        match self {
            Icon::Plus => '\u{E13D}',
            Icon::ChevronUp => '\u{E070}',
            Icon::ChevronDown => '\u{E06D}',
            Icon::ChevronRight => '\u{E06F}',
            Icon::Reset => '\u{E148}',
            Icon::Ellipsis => '\u{E0B6}',
            Icon::Close => '\u{E1B2}',
        }
    }

    /// The icon centred in `area`, tinted `colour`.
    pub fn draw(self, area: [f32; 4], colour: [u8; 3]) -> Prim {
        text(
            &self.glyph().to_string(),
            area,
            Face::Icon,
            colour,
            Align::Centre,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Centre,
    Right,
}

/// What a control looks like: the state it is in. A control that is on or off is a separate flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Normal,
    Hover,
    Pressed,
    Disabled,
}

/// Every state, for the showcase and for tests.
pub const STATES: [State; 4] = [State::Normal, State::Hover, State::Pressed, State::Disabled];

pub fn contains(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] < rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] < rect[1] + rect[3]
}

pub fn rect(rect: [f32; 4], colour: Rgb) -> Prim {
    Prim::Rect {
        rect,
        colour: style::rgba(colour),
    }
}

pub fn text(text: &str, rect: [f32; 4], face: Face, colour: [u8; 3], align: Align) -> Prim {
    Prim::Text {
        text: text.to_string(),
        rect,
        face,
        colour,
        align,
    }
}

/// A fill with a border drawn round it, outside it (the border's weight is `LINE_WEIGHT`).
fn framed(area: [f32; 4], border: Rgb, fill: Rgb, scale: f32) -> [Prim; 2] {
    let w = style::LINE_WEIGHT * scale;
    [
        rect(
            [
                area[0] - w,
                area[1] - w,
                area[2] + 2.0 * w,
                area[3] + 2.0 * w,
            ],
            border,
        ),
        rect(area, fill),
    ]
}

/// Room a line of `text` takes in the pure layout (the real width is only known once shaped).
pub fn text_width(text: &str, scale: f32) -> f32 {
    text.chars().count() as f32 * style::SPACE_CHAR_WIDTH * scale
}

fn lift(c: Rgb, by: f32) -> Rgb {
    c.map(|v| (v + by).clamp(0.0, 1.0))
}

/// A control's fill: neutral by state when off, `on_colour` when on.
fn fill(state: State, on: bool, on_colour: Rgb) -> Rgb {
    match (state, on) {
        (State::Disabled, true) => style::STATE_HOVER,
        (State::Disabled, false) => style::SURFACE_CONTROL,
        (State::Normal, true) => on_colour,
        (State::Hover, true) => lift(on_colour, style::STATE_ON_HOVER_LIFT),
        (State::Pressed, true) => lift(on_colour, -style::STATE_ON_HOVER_LIFT),
        (State::Normal, false) => style::SURFACE_CONTROL,
        (State::Hover, false) => style::STATE_HOVER,
        (State::Pressed, false) => style::STATE_PRESSED,
    }
}

/// A control's text colour.
fn ink(state: State, on: bool) -> [u8; 3] {
    match (state, on) {
        (State::Disabled, _) => style::TEXT_DISABLED,
        (_, true) => style::TEXT_ON_ACCENT,
        (State::Normal, false) => style::TEXT_SECONDARY,
        (State::Hover, false) => style::TEXT_HIGH,
        (_, false) => style::TEXT_PRIMARY,
    }
}

/// A control that is on in the Hairline look (ADR 0044): a slightly lighter grey chip with the accent as its text and
/// a thin line along its foot. Off, it is a plain grey control that lifts under the pointer (white text, never the
/// accent, so hover never reads as on) and darkens under a press.
fn accent_control(area: [f32; 4], label: &str, on: bool, state: State, scale: f32) -> Vec<Prim> {
    let fill = match (state, on) {
        (State::Disabled, true) => style::STATE_HOVER,
        (State::Disabled, false) => style::SURFACE_CONTROL,
        (State::Normal, true) => style::SURFACE_CONTROL_ON,
        (State::Hover, true) => style::STATE_ON_HOVER,
        (State::Pressed, _) => style::SURFACE_CONTROL_PRESSED,
        (State::Normal, false) => style::SURFACE_CONTROL,
        (State::Hover, false) => style::STATE_HOVER,
    };
    let ink = match (state, on) {
        (State::Disabled, _) => style::TEXT_DISABLED,
        (_, true) => style::accent_text(),
        (State::Normal, false) => style::TEXT_SECONDARY,
        (State::Hover, false) => style::TEXT_HIGH,
        (State::Pressed, false) => style::TEXT_PRIMARY,
    };
    let mut prims = vec![rect(area, fill)];
    if on && state != State::Disabled {
        prims.push(underline(area, style::accent(), scale));
    }
    prims.push(text(label, area, Face::Label, ink, Align::Centre));
    prims
}

/// A menu row under the pointer: a neutral lift and a thin accent bar at its left edge (ADR 0044).
pub fn hovered_row(area: [f32; 4], lift: Rgb, scale: f32) -> [Prim; 2] {
    [
        rect(area, lift),
        rect(
            [area[0], area[1], style::SPACE_ACCENT_LINE * scale, area[3]],
            style::accent(),
        ),
    ]
}

/// A thin line along the foot of `area`, in `colour`.
pub fn underline(area: [f32; 4], colour: Rgb, scale: f32) -> Prim {
    let line = style::SPACE_ACCENT_LINE * scale;
    rect([area[0], area[1] + area[3] - line, area[2], line], colour)
}

// ---- Chips ----

/// The channels a chip can show in its own colour when on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    R,
    G,
    B,
    Y,
}

impl Channel {
    pub fn colour(self) -> Rgb {
        match self {
            Channel::R => style::CHANNEL_R,
            Channel::G => style::CHANNEL_G,
            Channel::B => style::CHANNEL_B,
            Channel::Y => style::CHANNEL_Y,
        }
    }
}

/// How a row of chips behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipMode {
    /// One choice at a time (Channel view's Y R G B).
    Single,
    /// Any combination, at least one always on (the luma waveform's Y R G B).
    Multiple,
    /// Each chip on its own, free to be off (B/W).
    Toggle,
    /// At most one on; none is allowed, and clicking the one that is on turns it off (the clean picture's channel).
    OneOrNone,
}

/// What a row of chips is after a click on chip `index`.
pub fn clicked(mode: ChipMode, on: &[bool], index: usize) -> Vec<bool> {
    let mut next = on.to_vec();
    match mode {
        ChipMode::Single => next
            .iter_mut()
            .enumerate()
            .for_each(|(i, v)| *v = i == index),
        ChipMode::Multiple => {
            let others = on.iter().enumerate().any(|(i, v)| *v && i != index);
            if let Some(v) = next.get_mut(index) {
                *v = !*v || !others;
            }
        }
        ChipMode::OneOrNone => {
            let was_on = on.get(index).copied().unwrap_or(false);
            next.iter_mut()
                .enumerate()
                .for_each(|(i, v)| *v = i == index && !was_on);
        }
        ChipMode::Toggle => {
            if let Some(v) = next.get_mut(index) {
                *v = !*v;
            }
        }
    }
    next
}

/// A small toggle: a letter or a short word, grey when off, the accent (or its channel's colour) when on.
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub text: String,
    pub channel: Option<Channel>,
}

impl Chip {
    pub fn new(text: &str) -> Chip {
        Chip {
            text: text.to_string(),
            channel: None,
        }
    }

    pub fn channel(text: &str, channel: Channel) -> Chip {
        Chip {
            text: text.to_string(),
            channel: Some(channel),
        }
    }

    /// Width and height.
    pub fn size(&self, scale: f32) -> [f32; 2] {
        let wide = 2.0 * style::SPACE_CHIP_PAD * scale + text_width(&self.text, scale);
        [
            wide.max(style::SPACE_CHIP_MIN_WIDTH * scale),
            style::SPACE_CHIP_HEIGHT * scale,
        ]
    }

    /// Where the chip is with its top left at `at`: also its hit area.
    pub fn rect(&self, at: [f32; 2], scale: f32) -> [f32; 4] {
        let [w, h] = self.size(scale);
        [at[0], at[1], w, h]
    }

    pub fn draw(&self, at: [f32; 2], on: bool, state: State, scale: f32) -> Vec<Prim> {
        let area = self.rect(at, scale);
        match self.channel {
            None => accent_control(area, &self.text, on, state, scale),
            Some(channel) => vec![
                rect(area, fill(state, on, channel.colour())),
                text(&self.text, area, Face::Label, ink(state, on), Align::Centre),
            ],
        }
    }
}

/// Where each chip of a row goes, left to right from `at`.
pub fn chip_row(chips: &[Chip], at: [f32; 2], scale: f32) -> Vec<[f32; 4]> {
    chip_row_grouped(chips, &[], at, scale)
}

/// The gap before chip `i` of a row: the chip gap within a group, the wider group gap where `groups[i]` differs
/// from the chip before it (`groups` empty: one group).
fn gap_before(groups: &[usize], i: usize, scale: f32) -> f32 {
    if i == 0 {
        0.0
    } else if groups.get(i) != groups.get(i - 1) {
        style::SPACE_CHIP_GROUP_GAP * scale
    } else {
        style::SPACE_CHIP_GAP * scale
    }
}

/// Where each chip of a row goes when chip `i` belongs to group `groups[i]`: a wider gap between groups than within
/// one (ADR 0040).
pub fn chip_row_grouped(
    chips: &[Chip],
    groups: &[usize],
    at: [f32; 2],
    scale: f32,
) -> Vec<[f32; 4]> {
    let mut left = at[0];
    chips
        .iter()
        .enumerate()
        .map(|(i, chip)| {
            left += gap_before(groups, i, scale);
            let r = chip.rect([left, at[1]], scale);
            left += r[2];
            r
        })
        .collect()
}

/// The width of a row of chips.
pub fn chip_row_width(chips: &[Chip], scale: f32) -> f32 {
    chip_row_width_grouped(chips, &[], scale)
}

/// The width of a row of chips in groups (see [`chip_row_grouped`]).
pub fn chip_row_width_grouped(chips: &[Chip], groups: &[usize], scale: f32) -> f32 {
    chips
        .iter()
        .enumerate()
        .map(|(i, c)| c.size(scale)[0] + gap_before(groups, i, scale))
        .sum()
}

// ---- Segmented control ----

/// A row of joined segments, one selected.
#[derive(Clone, Debug, PartialEq)]
pub struct Segmented {
    pub labels: Vec<String>,
}

impl Segmented {
    pub fn new(labels: &[&str]) -> Segmented {
        Segmented {
            labels: labels.iter().map(|l| l.to_string()).collect(),
        }
    }

    /// Each segment, left to right from `at`, a hairline apart: also the hit areas.
    pub fn rects(&self, at: [f32; 2], scale: f32) -> Vec<[f32; 4]> {
        let gap = style::LINE_WEIGHT * scale;
        let mut left = at[0];
        self.labels
            .iter()
            .map(|label| {
                let w = (2.0 * style::SPACE_SEGMENT_PAD * scale + text_width(label, scale))
                    .max(style::SPACE_CHIP_MIN_WIDTH * scale);
                let r = [left, at[1], w, style::SPACE_SEGMENT_HEIGHT * scale];
                left += w + gap;
                r
            })
            .collect()
    }

    /// Width and height of the whole control.
    pub fn size(&self, scale: f32) -> [f32; 2] {
        let rects = self.rects([0.0, 0.0], scale);
        [
            rects.last().map_or(0.0, |r| r[0] + r[2]),
            style::SPACE_SEGMENT_HEIGHT * scale,
        ]
    }

    /// The segment under `point`.
    pub fn hit(&self, at: [f32; 2], point: [f32; 2], scale: f32) -> Option<usize> {
        self.rects(at, scale)
            .iter()
            .position(|r| contains(*r, point))
    }

    /// `selected` is the one chosen; segment `hot` is in `state` (hover or pressed), the rest are normal. A
    /// disabled control has every segment disabled.
    pub fn draw(
        &self,
        at: [f32; 2],
        selected: usize,
        hot: usize,
        state: State,
        scale: f32,
    ) -> Vec<Prim> {
        let rects = self.rects(at, scale);
        let mut prims = Vec::new();
        if let (Some(first), Some(last)) = (rects.first(), rects.last()) {
            let w = style::LINE_WEIGHT * scale;
            prims.push(rect(
                [
                    first[0] - w,
                    first[1] - w,
                    last[0] + last[2] - first[0] + 2.0 * w,
                    first[3] + 2.0 * w,
                ],
                style::LINE_BORDER,
            ));
        }
        for (i, (r, label)) in rects.iter().zip(&self.labels).enumerate() {
            let own = if state == State::Disabled {
                State::Disabled
            } else if i == hot {
                state
            } else {
                State::Normal
            };
            prims.extend(accent_control(*r, label, i == selected, own, scale));
        }
        prims
    }
}

// ---- Slider with a value box ----

/// Where the parts of a slider are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderLayout {
    /// The whole row, track to reset icon: left, top, width, height.
    pub row: [f32; 4],
    pub track: [f32; 4],
    /// The part of the track up to the thumb.
    pub filled: [f32; 4],
    pub thumb: [f32; 4],
    /// The value box, arrows included.
    pub value_box: [f32; 4],
    /// The number's part of the box (a press types into it).
    pub number: [f32; 4],
    /// The increase and decrease arrows, one above the other at the box's right.
    pub up: [f32; 4],
    pub down: [f32; 4],
    /// The circular reset icon after the box.
    pub reset: [f32; 4],
}

/// A slider with its value box and reset icon beside it (#225).
#[derive(Clone, Debug, PartialEq)]
pub struct Slider {
    /// 0 to 1 along the track.
    pub value: f32,
    /// What the number says ("75 %").
    pub number: String,
}

impl Slider {
    pub fn layout(&self, at: [f32; 2], scale: f32) -> SliderLayout {
        let (height, track_w, thick) = (
            style::SPACE_SLIDER_HEIGHT * scale,
            style::SPACE_SLIDER_TRACK * scale,
            style::SPACE_SLIDER_TRACK_THICKNESS * scale,
        );
        let thumb = style::SPACE_SLIDER_THUMB * scale;
        // The thumb overhangs the track by half its width at either end, so the track starts that far in.
        let track = [
            at[0] + thumb / 2.0,
            at[1] + (height - thick) / 2.0,
            track_w,
            thick,
        ];
        let value = self.value.clamp(0.0, 1.0);
        let x = track[0] + value * track_w;
        let gap = style::SPACE_CHIP_GAP * scale;
        let box_h = height - gap;
        let arrows = style::SPACE_VALUE_ARROWS * scale;
        let number_w = style::SPACE_VALUE_BOX * scale;
        let value_box = [
            track[0] + track_w + thumb / 2.0 + style::SPACE_SLIDER_BOX_GAP * scale,
            at[1] + (height - box_h) / 2.0,
            number_w + 2.0 * arrows,
            box_h,
        ];
        let number = [value_box[0], value_box[1], number_w, box_h];
        // Two buttons side by side at the box's right, each as tall as the box: a hit target of at least 20 x 20.
        let down = [value_box[0] + number_w, value_box[1], arrows, box_h];
        let up = [down[0] + arrows, value_box[1], arrows, box_h];
        let side = style::SPACE_RESET_ICON * scale;
        let reset = [
            value_box[0] + value_box[2] + style::SPACE_RESET_GAP * scale,
            at[1] + (height - side) / 2.0,
            side,
            side,
        ];
        SliderLayout {
            row: [at[0], at[1], reset[0] + reset[2] - at[0], height],
            track,
            filled: [track[0], track[1], x - track[0], thick],
            thumb: [
                x - thumb / 2.0,
                at[1] + (height - style::SPACE_SLIDER_THUMB_HEIGHT * scale) / 2.0,
                thumb,
                style::SPACE_SLIDER_THUMB_HEIGHT * scale,
            ],
            value_box,
            number,
            up,
            down,
            reset,
        }
    }

    /// The hit area that grabs the thumb or the track: the track's length, the thumb's overhang, the row's height.
    pub fn grab(&self, at: [f32; 2], scale: f32) -> [f32; 4] {
        let l = self.layout(at, scale);
        let overhang = style::SPACE_SLIDER_THUMB * scale / 2.0;
        [
            l.track[0] - overhang,
            l.row[1],
            l.track[2] + 2.0 * overhang,
            l.row[3],
        ]
    }

    /// The value (0 to 1) a pointer at `x` means.
    pub fn value_at(&self, at: [f32; 2], x: f32, scale: f32) -> f32 {
        let l = self.layout(at, scale);
        ((x - l.track[0]) / l.track[2]).clamp(0.0, 1.0)
    }

    /// Drawn with the reset icon enabled and nothing being typed.
    pub fn draw(&self, at: [f32; 2], state: State, scale: f32) -> Vec<Prim> {
        self.draw_with(at, state, scale, None, true)
    }

    /// Drawn with `typing` in the value box when it is being typed into, and the reset icon dim when
    /// `reset_enabled` is false (the value is already the default).
    pub fn draw_with(
        &self,
        at: [f32; 2],
        state: State,
        scale: f32,
        typing: Option<&str>,
        reset_enabled: bool,
    ) -> Vec<Prim> {
        self.draw_under(
            at,
            state,
            scale,
            typing,
            reset_enabled,
            [f32::NEG_INFINITY; 2],
        )
    }

    /// As [`Slider::draw_with`], with the arrow or reset icon under `cursor` lit (a lighter ground, white glyph).
    pub fn draw_under(
        &self,
        at: [f32; 2],
        state: State,
        scale: f32,
        typing: Option<&str>,
        reset_enabled: bool,
        cursor: [f32; 2],
    ) -> Vec<Prim> {
        let l = self.layout(at, scale);
        let disabled = state == State::Disabled;
        // A thin accent track and a grey thumb (ADR 0044).
        let thumb = match state {
            State::Normal => style::rgb(style::TEXT_PRIMARY),
            State::Hover => style::rgb(style::TEXT_HIGH),
            State::Pressed => style::rgb(style::TEXT_SECONDARY),
            State::Disabled => style::STATE_PRESSED,
        };
        let ink = if disabled {
            style::TEXT_DISABLED
        } else {
            style::TEXT_PRIMARY
        };
        let w = (style::LINE_WEIGHT * scale).max(1.0);
        let b = l.value_box;
        let mut prims = vec![
            rect(l.track, style::STATE_PRESSED),
            rect(
                l.filled,
                if disabled {
                    style::STATE_PRESSED
                } else {
                    style::accent()
                },
            ),
            rect(l.thumb, thumb),
            // The value box: a frame (the accent while typing) round a dark field.
            rect(
                b,
                if typing.is_some() {
                    style::accent()
                } else {
                    style::LINE_BORDER
                },
            ),
            rect(
                [b[0] + w, b[1] + w, b[2] - 2.0 * w, b[3] - 2.0 * w],
                style::SURFACE_PANEL,
            ),
        ];
        let shown = match typing {
            Some(t) => format!("{t}|"),
            None => self.number.clone(),
        };
        let pad = style::SPACE_CHIP_GAP * 2.0 * scale;
        prims.push(text(
            &shown,
            [
                l.number[0] + pad,
                l.number[1],
                l.number[2] - 2.0 * pad,
                l.number[3],
            ],
            Face::Label,
            ink,
            Align::Right,
        ));
        // The arrows: two buttons with a divider before each, a filled triangle in each.
        for (button, up) in [(l.down, false), (l.up, true)] {
            prims.push(rect(
                [button[0], b[1] + w, w, b[3] - 2.0 * w],
                style::LINE_BORDER,
            ));
            let lit = !disabled && contains(button, cursor);
            if lit {
                prims.push(rect(
                    [button[0] + w, b[1] + w, button[2] - 2.0 * w, b[3] - 2.0 * w],
                    style::STATE_HOVER,
                ));
            }
            prims.push(control_icon(
                if up {
                    Icon::ChevronUp
                } else {
                    Icon::ChevronDown
                },
                button,
                !disabled,
                lit,
            ));
        }
        prims.extend(reset_icon(
            l.reset,
            reset_enabled && !disabled,
            contains(l.reset, cursor),
            scale,
        ));
        prims
    }
}

/// An icon centred in its button's `area`: bright at rest, white when `lit` (under the pointer), dim when not
/// `enabled`. The value box's arrows and the reset use it (#238, ADR 0046).
fn control_icon(icon: Icon, area: [f32; 4], enabled: bool, lit: bool) -> Prim {
    icon.draw(
        area,
        if !enabled {
            style::TEXT_DISABLED
        } else if lit {
            style::TEXT_HIGH
        } else {
            style::TEXT_PRIMARY
        },
    )
}

/// The reset arrow (Lucide's rotate-ccw): dim when there is nothing to reset, bright at rest, white on a lighter
/// ground under the pointer.
pub fn reset_icon(area: [f32; 4], enabled: bool, hover: bool, _scale: f32) -> Vec<Prim> {
    let mut prims = Vec::new();
    if enabled && hover {
        prims.push(rect(area, style::STATE_HOVER));
    }
    prims.push(control_icon(Icon::Reset, area, enabled, hover));
    prims
}

// ---- Popover ----

/// The popover: the settings menu (and later the layout menu) drawn in `area` (as [`Menu::rect`] placed it),
/// with the row under `cursor` lit.
pub fn popover(menu: &Menu, area: [f32; 4], cursor: [f32; 2], scale: f32) -> Vec<Prim> {
    let mut prims: Vec<Prim> = framed(area, style::LINE_BORDER, style::SURFACE_MENU, scale).into();
    let text_width = area[2] - style::SPACE_MENU_TEXT_RIGHT * scale;
    let weight = (style::LINE_WEIGHT * scale).max(1.0);
    for (i, row) in menu.rows.iter().enumerate() {
        let row_rect = menu.row_rect(area, i, scale);
        let top = row_rect[1];
        let own = menu.own_rect(area, i, scale);
        // A group starts after a quiet line in the room above its heading (or its first amount).
        if menu.gap_before(i) > 0.0 {
            let inset = style::SPACE_MENU_HEADING_LEFT * scale;
            prims.push(rect(
                [
                    area[0] + inset,
                    own[1] - (menu.gap_before(i) * scale + weight) / 2.0,
                    area[2] - 2.0 * inset,
                    weight,
                ],
                style::LINE_BORDER,
            ));
        }
        match row {
            MenuRow::Heading(label) => {
                let left = area[0] + style::SPACE_MENU_HEADING_LEFT * scale;
                prims.push(text(
                    label,
                    [left, own[1], text_width, own[3]],
                    Face::Label,
                    style::TEXT_PRIMARY,
                    Align::Left,
                ));
                if label == crate::settings::PANEL_SECTION
                    && let Some(reset) = menu.panel_reset_rect(area, scale)
                {
                    prims.extend(reset_icon(
                        reset,
                        !menu.panel_at_default,
                        contains(reset, cursor),
                        scale,
                    ));
                }
            }
            MenuRow::Slider {
                key,
                label,
                amount,
                value,
            } => {
                let state = if contains(own, cursor) {
                    State::Hover
                } else {
                    State::Normal
                };
                prims.push(text(
                    label,
                    menu.label_rect(area, i, scale),
                    Face::Label,
                    style::TEXT_SECONDARY,
                    Align::Left,
                ));
                let typing = menu.editing().filter(|(k, _)| k == key).map(|(_, t)| t);
                let slider = Slider {
                    value: amount.fraction(*value),
                    number: amount.text(*value),
                };
                let at = menu.slider_origin(area, i, scale);
                prims.extend(slider.draw_under(
                    at,
                    state,
                    scale,
                    typing,
                    *value != amount.default,
                    cursor,
                ));
            }
            MenuRow::Panel(action) => {
                if contains(row_rect, cursor) {
                    prims.extend(hovered_row(row_rect, style::STATE_HOVER, scale));
                }
                let left = area[0] + style::SPACE_MENU_TEXT_LEFT * scale;
                prims.push(text(
                    &action.text(),
                    [left, top, text_width, row_rect[3]],
                    Face::Label,
                    style::TEXT_PRIMARY,
                    Align::Left,
                ));
            }
            MenuRow::Close => {
                if contains(row_rect, cursor) {
                    prims.extend(hovered_row(row_rect, style::STATE_HOVER, scale));
                }
                let left = area[0] + style::SPACE_MENU_TEXT_LEFT * scale;
                prims.push(text(
                    "Close",
                    [left, top, text_width, row_rect[3]],
                    Face::Label,
                    style::TEXT_PRIMARY,
                    Align::Left,
                ));
            }
            MenuRow::Choice {
                text: label, on, ..
            } => {
                if contains(row_rect, cursor) {
                    prims.extend(hovered_row(row_rect, style::STATE_HOVER, scale));
                }
                if *on {
                    let side = style::SPACE_MENU_MARK * scale;
                    prims.push(rect(
                        [
                            area[0] + style::SPACE_MENU_MARK_LEFT * scale,
                            top + (row_rect[3] - side) / 2.0,
                            side,
                            side,
                        ],
                        style::accent(),
                    ));
                }
                let left = area[0] + style::SPACE_MENU_TEXT_LEFT * scale;
                prims.push(text(
                    label,
                    [left, top, text_width, row_rect[3]],
                    Face::Label,
                    style::TEXT_PRIMARY,
                    Align::Left,
                ));
            }
        }
    }
    prims
}

/// The Panel reset's tooltip.
pub const PANEL_RESET_TIP: &str = "Reset this Panel's settings";

/// What a reset icon under `cursor` is called: an amount's ("Reset to 100 %") or the Panel's, with the icon's
/// rectangle to hang the tooltip on. `None` when the cursor is on neither.
pub fn popover_tip(
    menu: &Menu,
    area: [f32; 4],
    cursor: [f32; 2],
    scale: f32,
) -> Option<([f32; 4], String)> {
    if let Some(reset) = menu.panel_reset_rect(area, scale)
        && contains(reset, cursor)
    {
        return Some((reset, PANEL_RESET_TIP.to_string()));
    }
    let (_, crate::settings::AmountHit::Reset) = menu.amount_hit(area, cursor, scale)? else {
        return None;
    };
    let row = menu.row_at(area, cursor, scale)?;
    let MenuRow::Slider { amount, .. } = menu.rows.get(row)? else {
        return None;
    };
    Some((
        menu.slider_layout(area, row, scale).reset,
        format!("Reset to {}", amount.text(amount.default)),
    ))
}

/// The tooltip of a reset icon under the pointer, drawn inside a window of `window` device pixels.
pub fn popover_tooltip(
    menu: &Menu,
    area: [f32; 4],
    cursor: [f32; 2],
    window: [f32; 2],
    scale: f32,
) -> Vec<Prim> {
    match popover_tip(menu, area, cursor, scale) {
        Some((anchor, words)) => {
            let tip = Tooltip::new(&words);
            tip.draw(tip.rect(anchor, window, scale), scale)
        }
        None => Vec::new(),
    }
}

// ---- Panel focus and the empty page (ADR 0038, #206) ----

/// The focused Panel's mark: a thin accent line along the top of its title bar and nothing round the picture
/// (ADR 0044). `frame` is the Panel's frame.
pub fn focus_line(frame: [f32; 4], scale: f32) -> Prim {
    let line = style::SPACE_ACCENT_LINE * scale;
    rect([frame[0], frame[1], frame[2], line], style::accent())
}

/// The dock target's outline: dashed in the accent over a faint grey wash (ADR 0044). `target` is the area a drop
/// would take.
pub fn dock_overlay(target: [f32; 4], scale: f32) -> Vec<Prim> {
    let weight = (style::SPACE_ACCENT_LINE * scale).max(1.0);
    let (dash, gap) = (style::SPACE_DASH * scale, style::SPACE_DASH_GAP * scale);
    let [left, top, width, height] = target;
    let wash = style::SURFACE_DOCK_WASH;
    let mut prims = vec![Prim::Rect {
        rect: target,
        colour: [wash[0], wash[1], wash[2], style::DOCK_WASH_ALPHA],
    }];
    let mut along = |horizontal: bool, fixed: f32, start: f32, length: f32| {
        let mut at = start;
        while at < start + length {
            let run = dash.min(start + length - at);
            prims.push(rect(
                if horizontal {
                    [at, fixed, run, weight]
                } else {
                    [fixed, at, weight, run]
                },
                style::accent(),
            ));
            at += dash + gap;
        }
    };
    along(true, top, left, width);
    along(true, top + height - weight, left, width);
    along(false, left, top, height);
    along(false, left + width - weight, top, height);
    prims
}

/// The empty page's one large button, in the middle of the Panel `area` (never larger than it).
pub fn add_panel_button_rect(area: [f32; 4], scale: f32) -> [f32; 4] {
    let w = (style::SPACE_ADD_BUTTON_WIDTH * scale).min(area[2]);
    let h = (style::SPACE_ADD_BUTTON_HEIGHT * scale).min(area[3]);
    [
        area[0] + (area[2] - w) / 2.0,
        area[1] + (area[3] - h) / 2.0,
        w,
        h,
    ]
}

/// The empty page's Add panel button.
pub fn add_panel_button(area: [f32; 4], state: State, scale: f32) -> Vec<Prim> {
    let button = add_panel_button_rect(area, scale);
    vec![
        rect(button, fill(state, false, style::STATE_ON)),
        text(
            "Add panel",
            button,
            Face::Label,
            ink(state, false),
            Align::Centre,
        ),
    ]
}

// ---- The edge + (ADR 0038, #209) ----

/// The four **+** hit areas of the Panel in `frame`, each a square against its own side: left and right in the
/// middle of the display's height, top and bottom in the middle of its width. All sit inside the Panel, under the
/// title bar and off the dividers' grab zone.
pub fn edge_plus_rects(frame: [f32; 4], scale: f32) -> [(Edge, [f32; 4]); 4] {
    let layout = PanelLayout::new(frame, scale);
    let content = layout.content;
    let size = style::SPACE_EDGE_PLUS * scale;
    let inset = style::SPACE_EDGE_PLUS_INSET * scale;
    let (cx, cy) = (
        content[0] + (content[2] - size) / 2.0,
        content[1] + (content[3] - size) / 2.0,
    );
    [
        (Edge::Left, [frame[0] + inset, cy, size, size]),
        (
            Edge::Right,
            [frame[0] + frame[2] - inset - size, cy, size, size],
        ),
        (Edge::Top, [cx, content[1], size, size]),
        (
            Edge::Bottom,
            [cx, frame[1] + frame[3] - inset - size, size, size],
        ),
    ]
}

/// The side whose **+** is at `point` on the Panel in `frame`, if any.
pub fn edge_plus_at(frame: [f32; 4], point: [f32; 2], scale: f32) -> Option<Edge> {
    edge_plus_rects(frame, scale)
        .into_iter()
        .find(|(_, r)| contains(*r, point))
        .map(|(edge, _)| edge)
}

/// Whether the **+** signs show: only while the pointer is over the Panel.
pub fn edge_plus_shown(frame: [f32; 4], pointer: [f32; 2]) -> bool {
    contains(frame, pointer)
}

/// The four **+** signs, quiet, the one on `lit` lighter (the pointer is on it).
pub fn edge_plus_draw(frame: [f32; 4], lit: Option<Edge>, scale: f32) -> Vec<Prim> {
    let mut prims = Vec::new();
    for (edge, r) in edge_plus_rects(frame, scale) {
        let state = if lit == Some(edge) {
            State::Hover
        } else {
            State::Normal
        };
        prims.push(rect(r, fill(state, false, style::STATE_ON)));
        prims.push(Icon::Plus.draw(r, ink(state, false)));
    }
    prims
}

// ---- Tooltip ----

/// The hover text that names a chip, a button or a tag.
#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
    pub text: String,
}

impl Tooltip {
    pub fn new(text: &str) -> Tooltip {
        Tooltip {
            text: text.to_string(),
        }
    }

    /// The lines of its text (`\n` breaks a line).
    pub fn lines(&self) -> Vec<&str> {
        self.text.split('\n').collect()
    }

    pub fn size(&self, scale: f32) -> [f32; 2] {
        let (size, line) = Face::Label.metrics();
        let lines = self.lines();
        let widest = lines
            .iter()
            .map(|l| text_width(l, scale))
            .fold(0.0, f32::max);
        [
            widest + 2.0 * style::SPACE_TOOLTIP_PAD * scale,
            (size * line * lines.len() as f32 + 2.0 * style::SPACE_TOOLTIP_PAD) * scale,
        ]
    }

    /// Where it goes for something at `anchor`: centred under it, above it when there is no room below, and
    /// always inside the `window` (device pixels).
    pub fn rect(&self, anchor: [f32; 4], window: [f32; 2], scale: f32) -> [f32; 4] {
        let [w, h] = self.size(scale);
        let gap = style::SPACE_TOOLTIP_GAP * scale;
        let below = anchor[1] + anchor[3] + gap;
        let top = if below + h <= window[1] {
            below
        } else {
            (anchor[1] - gap - h).max(0.0)
        };
        let left = (anchor[0] + (anchor[2] - w) / 2.0)
            .min(window[0] - w)
            .max(0.0);
        [left, top, w, h]
    }

    pub fn draw(&self, area: [f32; 4], scale: f32) -> Vec<Prim> {
        let mut prims: Vec<Prim> =
            framed(area, style::LINE_BORDER, style::SURFACE_MENU, scale).into();
        let lines = self.lines();
        if lines.len() == 1 {
            prims.push(text(
                &self.text,
                area,
                Face::Label,
                style::TEXT_PRIMARY,
                Align::Centre,
            ));
            return prims;
        }
        // Several lines, each left-aligned inside the padding.
        let pad = style::SPACE_TOOLTIP_PAD * scale;
        let (size, line) = Face::Label.metrics();
        let row = size * line * scale;
        for (i, l) in lines.iter().enumerate() {
            prims.push(text(
                l,
                [
                    area[0] + pad,
                    area[1] + pad + row * i as f32,
                    area[2] - 2.0 * pad,
                    row,
                ],
                Face::Label,
                style::TEXT_PRIMARY,
                Align::Left,
            ));
        }
        prims
    }
}

// ---- Panel frame and title bar ----

/// A Panel's frame: its border and fill, with the title bar across the top.
pub fn panel_frame(area: [f32; 4], scale: f32) -> Vec<Prim> {
    framed(area, style::LINE_BORDER, style::SURFACE_PANEL, scale).into()
}

/// The Panel's room under its title bar, for the display.
pub fn panel_content(area: [f32; 4], scale: f32) -> [f32; 4] {
    let bar = style::SPACE_TITLE_BAR * scale;
    [area[0], area[1] + bar, area[2], (area[3] - bar).max(0.0)]
}

/// A Panel's title bar: its name, its reading tags (#99) and its few most-used controls as chips, and a small
/// settings button that opens the popover.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleBar {
    pub name: String,
    pub tags: String,
    pub chips: Vec<Chip>,
    /// The group of each chip (empty: one group); groups are set apart by a wider gap.
    pub groups: Vec<usize>,
    pub settings_button: bool,
    /// The view block (Fit, zoom readout, Link, ADR 0042) sits between the chips and the settings button.
    pub view: bool,
}

/// Where the parts of a title bar are.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleLayout {
    pub bar: [f32; 4],
    pub name: [f32; 4],
    /// Beside the name, when there is room.
    pub tags: Option<[f32; 4]>,
    pub chips: Vec<[f32; 4]>,
    pub settings: Option<[f32; 4]>,
    /// The view block's parts, when the Panel has one.
    pub view: Option<crate::view_block::BlockLayout>,
    /// The divider between the Panel's own chips and the view block: only where both are present.
    pub divider: Option<[f32; 4]>,
    /// The tags did not fit and show in the hover instead.
    pub tags_in_tooltip: bool,
    /// The name is cut short to make room.
    pub name_cut: bool,
}

impl TitleBar {
    /// What a title bar with a view block takes at its right end besides the chips and the settings button: the
    /// block, and the rule with its air on both sides when there are chips (what
    /// [`crate::toolbar::Toolbar::fit_to_reserving`] reserves).
    pub fn view_reserve(scale: f32) -> f32 {
        crate::view_block::ViewBlock::width(scale) + Self::rule_room(scale)
    }

    /// The rule between the Panel's own chips and the view block, with the air on each side of it.
    pub fn rule_room(scale: f32) -> f32 {
        (2.0 * style::SPACE_DIVIDER_GAP + style::SPACE_RULE_WIDTH) * scale
    }

    /// The settings button's width and the gap before it.
    pub fn settings_room(scale: f32) -> f32 {
        (style::SPACE_SETTINGS_BUTTON + style::SPACE_SETTINGS_GAP) * scale
    }

    /// The least width a Panel's title bar needs (ADR 0044): the bar's padding, the name cut to a few characters,
    /// the view block (with the settings gap), and the settings button. The Panel's least width
    /// ([`style::SPACE_MIN_PANEL_WIDTH`]) is this, rounded up. Chips and their "+N" need more than this: the "+N"
    /// chip is shown where [`TitleBar::overflow_width`] fits beside the same parts.
    pub fn least_width(scale: f32) -> f32 {
        2.0 * style::SPACE_BAR_PAD * scale
            + (style::SPACE_NAME_MIN + style::SPACE_NAME_GAP) * scale
            + crate::view_block::ViewBlock::width(scale)
            + Self::settings_room(scale)
    }

    /// The width of a title bar that shows the "+N" chip, the rule and the view block beside the least name: the
    /// least a picture Panel that has chips must be for them to show as "+N".
    pub fn overflow_width(scale: f32) -> f32 {
        Self::least_width(scale)
            + Self::rule_room(scale)
            + Chip::new(crate::toolbar::OVERFLOW_WIDEST).size(scale)[0]
            + style::SPACE_CHIP_GROUP_GAP * scale
    }

    /// The bar across the top of a Panel at `area`. The chips and the settings button stay at the right; the name
    /// truncates before they move; the tags go to the hover when they do not fit beside the name.
    pub fn layout(&self, area: [f32; 4], scale: f32) -> TitleLayout {
        let bar = [area[0], area[1], area[2], style::SPACE_TITLE_BAR * scale];
        let pad = style::SPACE_BAR_PAD * scale;
        let gap = style::SPACE_TAG_GAP * scale;
        let name_gap = style::SPACE_NAME_GAP * scale;
        let mid = |height: f32| bar[1] + (bar[3] - height) / 2.0;
        let mut right = bar[0] + bar[2] - pad;
        let settings = self.settings_button.then(|| {
            let side = style::SPACE_SETTINGS_BUTTON * scale;
            let height = style::SPACE_CHIP_HEIGHT * scale;
            right -= side;
            [right, mid(height), side, height]
        });
        let chips_width = chip_row_width_grouped(&self.chips, &self.groups, scale);
        let mut view = None;
        let mut divider = None;
        if settings.is_some() {
            right -= style::SPACE_SETTINGS_GAP * scale;
        }
        if self.view {
            view = Some(crate::view_block::ViewBlock::layout(right, bar, scale));
            right -= crate::view_block::ViewBlock::width(scale);
            if !self.chips.is_empty() {
                let weight = style::SPACE_RULE_WIDTH * scale;
                let air = style::SPACE_DIVIDER_GAP * scale;
                let height = style::SPACE_CHIP_HEIGHT * scale;
                divider = Some([right - air - weight, mid(height), weight, height]);
                right -= 2.0 * air + weight;
            }
        }
        let chips_left = (right - chips_width).max(bar[0] + pad);
        let chips = chip_row_grouped(
            &self.chips,
            &self.groups,
            [chips_left, mid(style::SPACE_CHIP_HEIGHT * scale)],
            scale,
        );
        let room = (chips_left - name_gap - (bar[0] + pad)).max(0.0);
        let name_width = text_width(&self.name, scale);
        let name_cut = name_width > room;
        let name = [bar[0] + pad, bar[1], name_width.min(room), bar[3]];
        let tags_width = text_width(&self.tags, scale);
        let fits = !name_cut && !self.tags.is_empty() && room - name_width - gap >= tags_width;
        let tags = fits.then(|| [name[0] + name[2] + gap, bar[1], tags_width, bar[3]]);
        TitleLayout {
            bar,
            name,
            tags,
            chips,
            settings,
            view,
            divider,
            tags_in_tooltip: !self.tags.is_empty() && !fits,
            name_cut,
        }
    }

    /// `chips` are `(on, state)` per chip; `settings` the button's state.
    pub fn draw(
        &self,
        area: [f32; 4],
        chips: &[(bool, State)],
        settings: State,
        scale: f32,
    ) -> Vec<Prim> {
        self.draw_focused(area, chips, settings, false, scale)
    }

    /// [`TitleBar::draw`] for a Panel that holds the focus when `focused`: a thin accent line along the top of the
    /// bar, and the name in the brightest text (ADR 0044).
    pub fn draw_focused(
        &self,
        area: [f32; 4],
        chips: &[(bool, State)],
        settings: State,
        focused: bool,
        scale: f32,
    ) -> Vec<Prim> {
        let l = self.layout(area, scale);
        let line = style::LINE_WEIGHT * scale;
        let mut prims = vec![
            rect(l.bar, style::SURFACE_PANEL),
            rect(
                [l.bar[0], l.bar[1] + l.bar[3] - line, l.bar[2], line],
                style::LINE_BORDER,
            ),
        ];
        prims.push(text(
            &self.name,
            l.name,
            if focused { Face::Strong } else { Face::Label },
            if focused {
                style::TEXT_HIGH
            } else {
                style::TEXT_PRIMARY
            },
            Align::Left,
        ));
        if focused {
            prims.push(focus_line(l.bar, scale));
        }
        if let Some(tags) = l.tags {
            prims.push(text(
                &self.tags,
                tags,
                Face::Scale,
                style::TEXT_MUTED,
                Align::Left,
            ));
        }
        for ((chip, r), (on, state)) in self.chips.iter().zip(&l.chips).zip(chips) {
            prims.extend(chip.draw([r[0], r[1]], *on, *state, scale));
        }
        if let Some(divider) = l.divider {
            prims.push(rect(divider, style::LINE_RULE));
        }
        if let Some(button) = l.settings {
            prims.extend(settings_button(button, settings, scale));
        }
        prims
    }
}

/// The settings button: three dots in a small button that lights under the pointer (a neutral lift, white dots).
pub fn settings_button(area: [f32; 4], state: State, _scale: f32) -> Vec<Prim> {
    let mut prims = Vec::new();
    if matches!(state, State::Hover | State::Pressed) {
        prims.push(rect(area, fill(state, false, style::STATE_ON)));
    }
    let colour = match state {
        State::Disabled => style::TEXT_DISABLED,
        State::Hover | State::Pressed => style::TEXT_HIGH,
        State::Normal => style::TEXT_SECONDARY,
    };
    prims.push(Icon::Ellipsis.draw(area, colour));
    prims
}

// ---- Top bar and status bar ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarKind {
    /// Across the top of the Window.
    Top,
    /// Along the bottom: the quiet status line.
    Status,
}

impl BarKind {
    pub fn height(self, scale: f32) -> f32 {
        scale
            * match self {
                BarKind::Top => style::SPACE_TOP_BAR,
                BarKind::Status => style::SPACE_STATUS_ROW,
            }
    }
}

/// A bar across `width` at (`left`, `top`): `start` at its left, `end` at its right.
pub fn bar(
    kind: BarKind,
    left: f32,
    top: f32,
    width: f32,
    start: &str,
    end: &str,
    scale: f32,
) -> Vec<Prim> {
    let h = kind.height(scale);
    let area = [left, top, width, h];
    let line = style::LINE_WEIGHT * scale;
    let pad = style::SPACE_BAR_PAD * scale;
    let (fill_colour, line_top, ink) = match kind {
        BarKind::Top => (style::SURFACE_PANEL, top + h - line, style::TEXT_PRIMARY),
        BarKind::Status => (style::SURFACE_WINDOW, top, style::TEXT_SECONDARY),
    };
    let half = (width - 2.0 * pad) / 2.0;
    vec![
        rect(area, fill_colour),
        rect([left, line_top, width, line], style::LINE_BORDER),
        text(
            start,
            [left + pad, top, half, h],
            Face::Label,
            ink,
            Align::Left,
        ),
        text(
            end,
            [left + pad + half, top, half, h],
            Face::Label,
            style::TEXT_SECONDARY,
            Align::Right,
        ),
    ]
}

// ---- Menu bar ----

/// The menu bar across the very top of the Window (#147): the menus' titles, left to right. Thin and quiet: no
/// icons, the panel's fill, the open menu's title lit.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuBar {
    pub titles: Vec<String>,
}

impl MenuBar {
    pub fn height(scale: f32) -> f32 {
        style::SPACE_MENU_BAR * scale
    }

    /// Where each title sits (its hit area), left to right from (`left`, `top`).
    pub fn title_rects(&self, left: f32, top: f32, scale: f32) -> Vec<[f32; 4]> {
        let pad = style::SPACE_MENU_BAR_ITEM_PAD * scale;
        let mut x = left + (style::SPACE_BAR_PAD - style::SPACE_MENU_BAR_ITEM_PAD).max(0.0) * scale;
        self.titles
            .iter()
            .map(|t| {
                let w = text_width(t, scale) + 2.0 * pad;
                let r = [x, top, w, MenuBar::height(scale)];
                x += w;
                r
            })
            .collect()
    }

    /// The title under `point`.
    pub fn hit(&self, left: f32, top: f32, point: [f32; 2], scale: f32) -> Option<usize> {
        self.title_rects(left, top, scale)
            .iter()
            .position(|r| contains(*r, point))
    }

    /// The bar across `width`, with the open menu's title pressed and the title under the pointer lit.
    pub fn draw(
        &self,
        left: f32,
        top: f32,
        width: f32,
        open: Option<usize>,
        hover: Option<usize>,
        scale: f32,
    ) -> Vec<Prim> {
        let h = MenuBar::height(scale);
        let line = style::LINE_WEIGHT * scale;
        let mut prims = vec![
            rect([left, top, width, h], style::SURFACE_PANEL),
            rect([left, top + h - line, width, line], style::LINE_BORDER),
        ];
        for (i, (title, r)) in self
            .titles
            .iter()
            .zip(self.title_rects(left, top, scale))
            .enumerate()
        {
            let state = if open == Some(i) {
                State::Pressed
            } else if hover == Some(i) {
                State::Hover
            } else {
                State::Normal
            };
            match state {
                State::Pressed => {
                    let open = [r[0], r[1], r[2], r[3] - line];
                    prims.push(rect(open, style::SURFACE_MENU));
                    prims.push(underline(open, style::accent(), scale));
                }
                State::Hover => {
                    prims.push(rect([r[0], r[1], r[2], r[3] - line], style::STATE_HOVER))
                }
                _ => {}
            }
            prims.push(text(
                title,
                r,
                Face::Label,
                style::TEXT_PRIMARY,
                Align::Centre,
            ));
        }
        prims
    }
}

/// The width of a row of `n` swatches.
fn swatch_row_width(n: usize, scale: f32) -> f32 {
    n as f32 * style::SPACE_SWATCH * scale
        + n.saturating_sub(1) as f32 * style::SPACE_SWATCH_GAP * scale
}

/// Where the `n` swatches of the menu row `row` are (also their hit areas), from the heading's left edge.
pub fn swatch_rects(row: [f32; 4], n: usize, scale: f32) -> Vec<[f32; 4]> {
    let side = style::SPACE_SWATCH * scale;
    let left = row[0] + style::SPACE_MENU_HEADING_LEFT * scale + style::LINE_WEIGHT * scale;
    (0..n)
        .map(|i| {
            [
                left + i as f32 * (side + style::SPACE_SWATCH_GAP * scale),
                row[1] + (row[3] - side) / 2.0,
                side,
                side,
            ]
        })
        .collect()
}

/// One entry of a menu that drops from the menu bar.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    Item(MenuItem),
    /// A line between groups of entries.
    Separator,
    /// A quiet line naming the group under it; not clickable.
    Heading(String),
    /// A row of colour swatches (Settings > Appearance): each a colour sample (data, not style) and whether it is the
    /// chosen one.
    Swatches(Vec<(Rgb, bool)>),
}

/// A clickable entry: its name, its hotkey as the menu shows it (empty for none), whether it can be used now and
/// whether it is the current one (a small mark, as in the popover).
#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub hotkey: String,
    pub enabled: bool,
    pub on: bool,
}

impl MenuItem {
    pub fn new(label: &str, hotkey: &str) -> MenuItem {
        MenuItem {
            label: label.to_string(),
            hotkey: hotkey.to_string(),
            enabled: true,
            on: false,
        }
    }
}

/// A menu dropped from the menu bar: its entries, each with its hotkey at the right.
#[derive(Clone, Debug, PartialEq)]
pub struct DropMenu {
    pub entries: Vec<MenuEntry>,
}

impl DropMenu {
    fn entry_height(entry: &MenuEntry, scale: f32) -> f32 {
        scale
            * match entry {
                MenuEntry::Separator => style::SPACE_MENU_SEPARATOR,
                _ => style::SPACE_MENU_ROW,
            }
    }

    /// The entries' columns, as entry numbers. A menu is one column; one taller than the style's cap flows on into
    /// the next column, so a long list (the Hotkeys panel) stays within the Window.
    fn columns(&self, scale: f32) -> Vec<Vec<usize>> {
        let cap = style::SPACE_MENU_MAX_HEIGHT * scale;
        let mut columns: Vec<Vec<usize>> = vec![Vec::new()];
        let mut used = 0.0;
        for (i, e) in self.entries.iter().enumerate() {
            let h = DropMenu::entry_height(e, scale);
            if used + h > cap && used > 0.0 {
                columns.push(Vec::new());
                used = 0.0;
            }
            used += h;
            if let Some(column) = columns.last_mut() {
                column.push(i);
            }
        }
        columns
    }

    /// Its size: wide enough for the longest name and hotkey, at least the popover's width, once per column.
    pub fn size(&self, scale: f32) -> [f32; 2] {
        let widest = self
            .entries
            .iter()
            .map(|e| match e {
                MenuEntry::Item(item) => {
                    style::SPACE_MENU_TEXT_LEFT * scale
                        + text_width(&item.label, scale)
                        + style::SPACE_MENU_HOTKEY_GAP * scale
                        + text_width(&item.hotkey, scale)
                        + style::SPACE_MENU_HEADING_LEFT * scale
                }
                MenuEntry::Heading(t) => {
                    style::SPACE_MENU_HEADING_LEFT * scale * 2.0 + text_width(t, scale)
                }
                MenuEntry::Swatches(colours) => {
                    style::SPACE_MENU_HEADING_LEFT * scale * 2.0
                        + swatch_row_width(colours.len(), scale)
                }
                MenuEntry::Separator => 0.0,
            })
            .fold(style::SPACE_MENU_WIDTH * scale, f32::max);
        let columns = self.columns(scale);
        let rows = columns
            .iter()
            .map(|c| {
                c.iter()
                    .map(|&i| DropMenu::entry_height(&self.entries[i], scale))
                    .sum::<f32>()
            })
            .fold(0.0, f32::max);
        [
            widest * columns.len() as f32,
            rows + 2.0 * style::SPACE_MENU_PAD * scale,
        ]
    }

    /// Where it drops: under `title` (the bar title it belongs to), its left edge on the title's, moved left as
    /// needed to stay inside a window of `window` device pixels. It never moves up over the bar, so a Window too
    /// short for it cuts its foot off (the Window has no minimum size yet, #172).
    pub fn rect(&self, title: [f32; 4], window: [f32; 2], scale: f32) -> [f32; 4] {
        let [w, h] = self.size(scale);
        [
            title[0].min(window[0] - w).max(0.0),
            title[1] + title[3],
            w,
            h,
        ]
    }

    /// Where it drops from a button that is not a bar title (the Page bar's **+** and Add panel, an edge **+**, the
    /// empty page's button): under `anchor`, its left edge on the button's, moved left to stay inside a window of
    /// `window` device pixels; above the button instead when it does not fit below and does above.
    pub fn rect_near(&self, anchor: [f32; 4], window: [f32; 2], scale: f32) -> [f32; 4] {
        let [w, h] = self.size(scale);
        let below = anchor[1] + anchor[3];
        let y = if below + h <= window[1] || anchor[1] - h < 0.0 {
            below
        } else {
            anchor[1] - h
        };
        [anchor[0].min(window[0] - w).max(0.0), y, w, h]
    }

    /// Each entry's row inside `area`, column by column.
    pub fn entry_rects(&self, area: [f32; 4], scale: f32) -> Vec<[f32; 4]> {
        let columns = self.columns(scale);
        let width = area[2] / columns.len() as f32;
        let mut rects = vec![[0.0; 4]; self.entries.len()];
        for (c, column) in columns.iter().enumerate() {
            let mut y = area[1] + style::SPACE_MENU_PAD * scale;
            for &i in column {
                let h = DropMenu::entry_height(&self.entries[i], scale);
                rects[i] = [area[0] + c as f32 * width, y, width, h];
                y += h;
            }
        }
        rects
    }

    /// The swatch under `point`: its entry and its place in the row.
    pub fn swatch_at(&self, area: [f32; 4], point: [f32; 2], scale: f32) -> Option<(usize, usize)> {
        self.entry_rects(area, scale)
            .iter()
            .zip(&self.entries)
            .enumerate()
            .find_map(|(i, (r, e))| match e {
                MenuEntry::Swatches(colours) => swatch_rects(*r, colours.len(), scale)
                    .iter()
                    .position(|s| contains(*s, point))
                    .map(|n| (i, n)),
                _ => None,
            })
    }

    /// The enabled item under `point`.
    pub fn hit(&self, area: [f32; 4], point: [f32; 2], scale: f32) -> Option<usize> {
        self.entry_rects(area, scale)
            .iter()
            .zip(&self.entries)
            .position(|(r, e)| {
                contains(*r, point) && matches!(e, MenuEntry::Item(item) if item.enabled)
            })
    }

    /// The menu in `area`, the enabled item under the pointer lit (pressed while the button is down). A disabled
    /// item is greyed and never lights.
    pub fn draw(
        &self,
        area: [f32; 4],
        hover: Option<usize>,
        pressed: bool,
        scale: f32,
    ) -> Vec<Prim> {
        let mut prims: Vec<Prim> =
            framed(area, style::LINE_BORDER, style::SURFACE_MENU, scale).into();
        let line = style::LINE_WEIGHT * scale;
        let heading_left = style::SPACE_MENU_HEADING_LEFT * scale;
        for (i, (entry, r)) in self
            .entries
            .iter()
            .zip(self.entry_rects(area, scale))
            .enumerate()
        {
            match entry {
                MenuEntry::Separator => prims.push(rect(
                    [
                        r[0] + heading_left,
                        r[1] + (r[3] - line) / 2.0,
                        r[2] - 2.0 * heading_left,
                        line,
                    ],
                    style::LINE_BORDER,
                )),
                MenuEntry::Heading(label) => prims.push(text(
                    label,
                    [r[0] + heading_left, r[1], r[2] - 2.0 * heading_left, r[3]],
                    Face::Strong,
                    style::TEXT_SECONDARY,
                    Align::Left,
                )),
                MenuEntry::Swatches(colours) => {
                    for (s, (colour, chosen)) in swatch_rects(r, colours.len(), scale)
                        .into_iter()
                        .zip(colours)
                    {
                        if *chosen {
                            prims.extend(framed(s, style::rgb(style::TEXT_HIGH), *colour, scale));
                        } else {
                            prims.push(rect(s, *colour));
                        }
                    }
                }
                MenuEntry::Item(item) => {
                    let state = if !item.enabled {
                        State::Disabled
                    } else if hover == Some(i) && pressed {
                        State::Pressed
                    } else if hover == Some(i) {
                        State::Hover
                    } else {
                        State::Normal
                    };
                    match state {
                        State::Hover => prims.extend(hovered_row(r, style::STATE_HOVER, scale)),
                        State::Pressed => prims.extend(hovered_row(r, style::STATE_PRESSED, scale)),
                        _ => {}
                    }
                    if item.on {
                        let side = style::SPACE_MENU_MARK * scale;
                        prims.push(rect(
                            [
                                r[0] + style::SPACE_MENU_MARK_LEFT * scale,
                                r[1] + (r[3] - side) / 2.0,
                                side,
                                side,
                            ],
                            style::accent(),
                        ));
                    }
                    let (ink, quiet) = if item.enabled {
                        (style::TEXT_PRIMARY, style::TEXT_SECONDARY)
                    } else {
                        (style::TEXT_DISABLED, style::TEXT_DISABLED)
                    };
                    let left = r[0] + style::SPACE_MENU_TEXT_LEFT * scale;
                    let inner = r[2] - style::SPACE_MENU_TEXT_LEFT * scale - heading_left;
                    prims.push(text(
                        &item.label,
                        [left, r[1], inner, r[3]],
                        Face::Label,
                        ink,
                        Align::Left,
                    ));
                    if !item.hotkey.is_empty() {
                        prims.push(text(
                            &item.hotkey,
                            [left, r[1], inner, r[3]],
                            Face::Label,
                            quiet,
                            Align::Right,
                        ));
                    }
                }
            }
        }
        prims
    }
}

// ---- Legend strip ----

/// One colour of a legend and what it says. The colour is data (a false colour band, a heatmap step, a skin
/// tint), not style.
#[derive(Clone, Debug, PartialEq)]
pub struct LegendItem {
    pub colour: Rgb,
    pub label: String,
}

/// A legend is a horizontal strip under the picture, full width, read left to right. There is no vertical form.
#[derive(Clone, Debug, PartialEq)]
pub struct LegendStrip {
    pub items: Vec<LegendItem>,
}

/// Where a legend's parts are.
#[derive(Clone, Debug, PartialEq)]
pub struct LegendLayout {
    pub strip: [f32; 4],
    /// The colour bars, left to right, filling the width.
    pub bars: Vec<[f32; 4]>,
    /// Each label under its bar, or `None` where the bar is too narrow for it.
    pub labels: Vec<Option<[f32; 4]>>,
}

impl LegendStrip {
    /// The strip across `width` at (`left`, `top`).
    pub fn layout(&self, left: f32, top: f32, width: f32, scale: f32) -> LegendLayout {
        let n = self.items.len().max(1) as f32;
        let cell = width / n;
        let bar_h = style::SPACE_LEGEND_BAR * scale;
        let strip = [left, top, width, style::SPACE_LEGEND_HEIGHT * scale];
        let mut bars = Vec::new();
        let mut labels = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            let x = left + cell * i as f32;
            bars.push([x, top, cell, bar_h]);
            let fits = text_width(&item.label, scale) <= cell;
            labels.push(fits.then(|| [x, top + bar_h, cell, strip[3] - bar_h]));
        }
        LegendLayout {
            strip,
            bars,
            labels,
        }
    }

    pub fn draw(&self, left: f32, top: f32, width: f32, scale: f32) -> Vec<Prim> {
        let l = self.layout(left, top, width, scale);
        let mut prims = Vec::new();
        for ((item, bar), label) in self.items.iter().zip(&l.bars).zip(&l.labels) {
            prims.push(rect(*bar, item.colour));
            if let Some(label_rect) = label {
                prims.push(text(
                    &item.label,
                    *label_rect,
                    Face::Scale,
                    style::TEXT_MUTED,
                    Align::Centre,
                ));
            }
        }
        prims
    }
}
