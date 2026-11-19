//! The proportional scale bar (#122): the legend strip ARRI False Colour and the saturation heatmap share. A bar
//! under the picture, the full width of the Panel, runs over a range (0 to 100 %, or 0 to the heatmap's scale)
//! with tick numbers along its top, and short marks under it at the meaningful points. This module is the pure
//! part: where the strip, the numbers and the marks go, and which of them fit. The bar's own colours are drawn
//! by the Picture View's `legend` function, one pixel column per position on the range.
//!
//! Text is dropped, never overlapped: ticks thin out to a coarser step as the Panel narrows, and a mark that
//! would touch another or leave the bar is not drawn.
use crate::display::ScaleText;
use crate::picture_stage::{BAR_MIN_HEIGHT, Legend};

/// Logical pixels: the bar, and the gap between it and the line of marks under it.
const BAR: f32 = 8.0;
const GAP: f32 = 2.0;
/// The numbers' line height and a rough width of one character, as the waveform's scale numbers.
const LINE: f32 = crate::waveform::SCALE_SIZE * 1.3;
const CHAR_WIDTH: f32 = crate::waveform::SCALE_SIZE * 0.6;

/// A mark under the bar: `text` for the part of the range from `from` to `to` (equal for a single point).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarMark {
    pub from: f64,
    pub to: f64,
    pub text: &'static str,
}

/// A scale bar over `from` to `to` in `unit` (" %"), ticked at the first of `steps` whose numbers fit, with
/// `marks` in order of importance.
pub struct ScaleBar<'a> {
    pub from: f64,
    pub to: f64,
    pub unit: &'a str,
    pub steps: &'a [f64],
    pub marks: &'a [BarMark],
}

/// Where the strip, the tick numbers and the marks are, in device pixels of the window.
#[derive(Clone, Debug, PartialEq)]
pub struct BarLayout {
    pub legend: Legend,
    pub ticks: Vec<ScaleText>,
    pub marks: Vec<ScaleText>,
}

fn number(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 { format!("{}", v.round() as i64) } else { format!("{v}") }
}

impl ScaleBar<'_> {
    /// The strip's height in a Panel at `scale`: tick numbers, the bar and, when there are marks, their line.
    pub fn strip(&self, scale: f32) -> f32 {
        let marks = if self.marks.is_empty() { 0.0 } else { GAP + LINE };
        (LINE + BAR + GAP + marks) * scale
    }

    /// The layout in a Panel of `area` (left, top, width, height) at `scale`, or `None` when `on` is false or the
    /// Panel is too short to spare the strip. `with_text` false keeps the strip but drops its text (no picture yet).
    pub fn layout(&self, on: bool, area: [f32; 4], scale: f32, with_text: bool) -> Option<BarLayout> {
        if !on || area[3] < BAR_MIN_HEIGHT * scale {
            return None;
        }
        let strip = self.strip(scale);
        let legend = Legend { strip, bar_offset: LINE * scale, bar: BAR * scale };
        let top = area[1] + area[3] - strip;
        let (ticks, marks) = if with_text {
            (self.ticks(area[0], area[2], top + LINE * scale / 2.0, scale), self.marks(area[0], area[2], top + (LINE + BAR + GAP) * scale + LINE * scale / 2.0, scale))
        } else {
            (Vec::new(), Vec::new())
        };
        Some(BarLayout { legend, ticks, marks })
    }

    fn x(&self, left: f32, width: f32, v: f64) -> f32 {
        left + width * ((v - self.from) / (self.to - self.from)) as f32
    }

    /// The left edge of a line of text `w` wide centred on `at`, moved to stay inside the bar.
    fn inside(left: f32, width: f32, at: f32, w: f32) -> f32 {
        (at - w / 2.0).clamp(left, (left + width - w).max(left))
    }

    /// The tick numbers at the finest step whose numbers do not touch; the range's two ends are always ticks.
    fn ticks(&self, left: f32, width: f32, y: f32, scale: f32) -> Vec<ScaleText> {
        let char_width = CHAR_WIDTH * scale;
        let text_width = |t: &str| t.chars().count() as f32 * char_width;
        let end = format!("{}{}", number(self.to), self.unit);
        let start = number(self.from);
        if text_width(&end) + text_width(&start) + char_width > width {
            return Vec::new();
        }
        let make = |l: f32, w: f32, text: String| ScaleText { text, left: l, top: y, width: w, right_aligned: false, centred: true };
        let end_width = text_width(&end);
        let end_left = left + width - end_width;
        for step in self.steps {
            let mut texts: Vec<(f32, f32, String)> = Vec::new();
            let mut k = (self.from / step).ceil();
            while k * step < self.to - 1e-9 {
                let v = k * step;
                let (t, w) = (number(v), text_width(&number(v)));
                texts.push((Self::inside(left, width, self.x(left, width, v), w), w, t));
                k += 1.0;
            }
            if texts.first().is_none_or(|t| t.0 > left + 0.5) {
                texts.insert(0, (left, text_width(&start), start.clone()));
            }
            // A multiple of the step too near the end gives way to the end itself.
            while texts.len() > 1 && texts.last().is_some_and(|(l, w, _)| l + w + char_width > end_left) {
                texts.pop();
            }
            texts.push((end_left, end_width, end.clone()));
            if texts.windows(2).all(|p| p[0].0 + p[0].1 + char_width <= p[1].0) {
                return texts.into_iter().map(|(l, w, t)| make(l, w, t)).collect();
            }
        }
        vec![make(left, text_width(&start), start), make(end_left, end_width, end)]
    }

    /// The marks that fit, most important first and each kept when it clears the ones kept before it, left to right.
    fn marks(&self, left: f32, width: f32, y: f32, scale: f32) -> Vec<ScaleText> {
        let char_width = CHAR_WIDTH * scale;
        let mut kept: Vec<ScaleText> = Vec::new();
        for m in self.marks {
            let w = m.text.chars().count() as f32 * char_width;
            if w > width {
                continue;
            }
            let at = (self.x(left, width, m.from) + self.x(left, width, m.to)) / 2.0;
            let x0 = Self::inside(left, width, at, w);
            if kept.iter().any(|k| x0 < k.left + k.width + char_width && x0 + w + char_width > k.left) {
                continue;
            }
            kept.push(ScaleText { text: m.text.to_string(), left: x0, top: y, width: w, right_aligned: false, centred: true });
        }
        kept.sort_by(|a, b| a.left.total_cmp(&b.left));
        kept
    }
}
