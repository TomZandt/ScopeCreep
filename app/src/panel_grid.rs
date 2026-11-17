//! Where the Panels go: an ordered grid sized to the number of displays that fills the Window, with borders the
//! user can drag (ADR 0011). This is the one place that decides where each display goes. Pure: no GPU, no window.
//!
//! The shape depends only on the number of displays, never on the window, so remembered border positions stay
//! valid whatever size the window is: rows hold as equal a number of Panels as possible (the longer rows first),
//! and every row fills the width, so nothing is left empty. Each Panel holds its display above and its label
//! under it ([`PanelLayout`]).
//!
//! Sizes are in device pixels; rectangles are left, top, width, height.

/// Logical pixels: the label's text size, and the label strip's height for one line.
pub const LABEL_SIZE: f32 = 12.5;
pub const LABEL_STRIP: f32 = 30.0;
/// The smallest a Panel can be dragged to, in logical pixels (width, height): the display still has room for its
/// scale and the label a whole line.
const MIN_PANEL: [f32; 2] = [120.0, 100.0];

/// The smallest a Panel can be at `scale` device pixels per logical pixel.
pub fn min_panel_size(scale: f32) -> [f32; 2] {
    MIN_PANEL.map(|v| v * scale)
}

/// A border between Panels that can be dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Border {
    /// Between the Panels `after` and `after + 1` of row `row`.
    Column { row: usize, after: usize },
    /// Between rows `after` and `after + 1`.
    Row { after: usize },
}

/// The grid: how many Panels each row holds, and how the width of each row and the height of the window are
/// shared out (weights that add up to 1).
#[derive(Clone, Debug, PartialEq)]
pub struct PanelGrid {
    rows: Vec<usize>,
    row_weights: Vec<f32>,
    column_weights: Vec<Vec<f32>>,
}

impl PanelGrid {
    /// The grid for `panels` displays, every Panel and row the same size.
    pub fn new(panels: usize) -> PanelGrid {
        let rows = if panels == 0 {
            Vec::new()
        } else {
            let columns = ((panels as f32 * 16.0 / 9.0).sqrt().ceil() as usize).clamp(1, panels);
            let count = panels.div_ceil(columns);
            (0..count).map(|r| panels / count + usize::from(r < panels % count)).collect::<Vec<_>>()
        };
        let row_weights = vec![1.0 / rows.len().max(1) as f32; rows.len()];
        let column_weights = rows.iter().map(|&n| vec![1.0 / n as f32; n]).collect();
        PanelGrid { rows, row_weights, column_weights }
    }

    /// How many Panels each row holds, top to bottom.
    pub fn rows(&self) -> &[usize] {
        &self.rows
    }

    /// Start and length of each part of `total` shared out by `weights`, with `gap` between.
    fn spans(start: f32, total: f32, gap: f32, weights: &[f32]) -> Vec<(f32, f32)> {
        let usable = (total - gap * weights.len().saturating_sub(1) as f32).max(0.0);
        let mut at = start;
        weights
            .iter()
            .map(|w| {
                let span = (at, w * usable);
                at += w * usable + gap;
                span
            })
            .collect()
    }

    /// Each Panel's rectangle, in reading order (left to right, then the next row down), filling `area` with
    /// `gap` between Panels.
    pub fn cells(&self, area: [f32; 4], gap: f32) -> Vec<[f32; 4]> {
        let [left, top, width, height] = area;
        let mut cells = Vec::new();
        for (row, (y, h)) in Self::spans(top, height, gap, &self.row_weights).into_iter().enumerate() {
            for (x, w) in Self::spans(left, width, gap, &self.column_weights[row]) {
                cells.push([x, y, w, h]);
            }
        }
        cells
    }

    /// The border under `point`, if any: within `slop` of the gap between two Panels.
    pub fn border_at(&self, area: [f32; 4], gap: f32, point: [f32; 2], slop: f32) -> Option<Border> {
        let [left, top, width, height] = area;
        let rows = Self::spans(top, height, gap, &self.row_weights);
        for (row, &(y, h)) in rows.iter().enumerate() {
            if point[1] < y || point[1] >= y + h {
                continue;
            }
            for (after, pair) in Self::spans(left, width, gap, &self.column_weights[row]).windows(2).enumerate() {
                let middle = pair[0].0 + pair[0].1 + gap / 2.0;
                if (point[0] - middle).abs() <= gap / 2.0 + slop {
                    return Some(Border::Column { row, after });
                }
            }
        }
        for (after, pair) in rows.windows(2).enumerate() {
            let middle = pair[0].0 + pair[0].1 + gap / 2.0;
            if (point[1] - middle).abs() <= gap / 2.0 + slop && point[0] >= left && point[0] <= left + width {
                return Some(Border::Row { after });
            }
        }
        None
    }

    /// Move `border` to `to`: the Panels on each side take up the difference, and neither goes under `min`
    /// (width, height).
    pub fn drag(&mut self, border: Border, to: [f32; 2], area: [f32; 4], gap: f32, min: [f32; 2]) {
        let [left, top, width, height] = area;
        let (spans, position, min, total, after) = match border {
            Border::Column { row, after } => (Self::spans(left, width, gap, &self.column_weights[row]), to[0], min[0], width, after),
            Border::Row { after } => (Self::spans(top, height, gap, &self.row_weights), to[1], min[1], height, after),
        };
        let weights = match border {
            Border::Column { row, .. } => &mut self.column_weights[row],
            Border::Row { .. } => &mut self.row_weights,
        };
        let usable = total - gap * (spans.len() - 1) as f32;
        if usable <= 0.0 {
            return;
        }
        let (start, first) = spans[after];
        let pair = first + spans[after + 1].1;
        let wanted = position - gap / 2.0 - start;
        let new_first = if pair < 2.0 * min { pair / 2.0 } else { wanted.clamp(min, pair - min) };
        weights[after] = new_first / usable;
        weights[after + 1] = (pair - new_first) / usable;
    }

    /// The border positions, to remember between sessions.
    pub fn to_text(&self) -> String {
        let join = |w: &[f32]| w.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
        let mut text = format!("panels {}\nrows {}\n", self.rows.iter().sum::<usize>(), join(&self.row_weights));
        for weights in &self.column_weights {
            text += &format!("columns {}\n", join(weights));
        }
        text
    }

    /// The grid for `panels` displays with the positions `text` holds. Positions saved for another number of
    /// displays, or that don't make sense, are ignored: the default grid is used.
    pub fn from_text(panels: usize, text: &str) -> PanelGrid {
        Self::parse(panels, text).unwrap_or_else(|| PanelGrid::new(panels))
    }

    fn parse(panels: usize, text: &str) -> Option<PanelGrid> {
        let default = PanelGrid::new(panels);
        let mut lines = text.lines();
        if lines.next()? != format!("panels {panels}") {
            return None;
        }
        let weights = |line: &str, key: &str, count: usize| -> Option<Vec<f32>> {
            let values: Vec<f32> = line.strip_prefix(key)?.split_whitespace().map(|v| v.parse().ok()).collect::<Option<_>>()?;
            let sum: f32 = values.iter().sum();
            (values.len() == count && values.iter().all(|v| v.is_finite() && *v > 0.0) && (sum - 1.0).abs() < 0.01).then_some(values)
        };
        let row_weights = weights(lines.next()?, "rows ", default.rows.len())?;
        let mut column_weights = Vec::new();
        for &count in &default.rows {
            column_weights.push(weights(lines.next()?, "columns ", count)?);
        }
        Some(PanelGrid { rows: default.rows, row_weights, column_weights })
    }
}

/// What sits inside one Panel: the display above, and the label under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelLayout {
    pub content: [f32; 4],
    pub label: [f32; 4],
}

impl PanelLayout {
    /// The layout inside `cell` at `scale` device pixels per logical pixel, for a label that wraps onto `lines`
    /// lines. The label keeps its full height however small the Panel; the display takes what is left (at least a
    /// pixel).
    pub fn new([left, top, width, height]: [f32; 4], scale: f32, lines: usize) -> PanelLayout {
        let strip = LABEL_STRIP * scale + (lines.max(1) - 1) as f32 * LABEL_SIZE * scale * 1.4;
        let content = (height - strip).max(1.0);
        PanelLayout { content: [left, top, width.max(1.0), content], label: [left, top + content, width.max(1.0), strip] }
    }
}
