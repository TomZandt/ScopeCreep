//! Where the Panels go: a layout of columns and rows with each Panel at a cell and a span, and borders the user
//! can drag (ADR 0011, ADR 0012). This is the one place that decides where each display goes. Pure: no GPU, no
//! window, no fonts.
//!
//! A layout is a grid of columns and rows. Column widths and row heights are weights that add up to 1, so a
//! remembered border position holds at any window size. Each Panel sits at a column and row and spans a number of
//! columns and rows. The default layout puts the clean picture large at the top left with the scopes beside it and
//! the Picture Views below, every cell used ([`PanelGrid::default_for`]).
//!
//! Each Panel has a thin border and a slim title bar at its top holding its label, and its display fills the rest
//! ([`PanelLayout`]). Sizes are in device pixels; rectangles are left, top, width, height.

use crate::style;

/// Logical pixels: the label's text size, and the title bar's height.
pub const LABEL_SIZE: f32 = style::TYPE_LABEL;
pub const TITLE_BAR: f32 = style::SPACE_TITLE_BAR;
/// The least height a display keeps, in logical pixels.
pub const MIN_DISPLAY: f32 = style::SPACE_MIN_DISPLAY;
/// The smallest a Panel can be dragged to, in logical pixels (width, height): the display keeps [`MIN_DISPLAY`]
/// under a title bar, inside the border.
const MIN_PANEL: [f32; 2] = [style::SPACE_MIN_PANEL_WIDTH, style::SPACE_MIN_PANEL_HEIGHT];
/// The window the saved border positions are checked against (logical pixels, the Panel area and the gap between
/// Panels): the default window size. A saved position that would leave a column or row under its minimum here is
/// rubbish.
const REFERENCE_AREA: [f32; 2] = style::SPACE_REFERENCE_AREA;
const REFERENCE_GAP: f32 = style::SPACE_REFERENCE_GAP;
/// The first line of a saved layout. A grid saved by v0.2.0 starts "panels" and is ignored.
const HEADER: &str = "layout 1";

/// The smallest a Panel can be at `scale` device pixels per logical pixel.
pub fn min_panel_size(scale: f32) -> [f32; 2] {
    MIN_PANEL.map(|v| v * scale)
}

/// A border between columns or between rows that can be dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Border {
    /// Between columns `after` and `after + 1`.
    Column { after: usize },
    /// Between rows `after` and `after + 1`.
    Row { after: usize },
}

/// What a press at a point starts: a border drag or a Panel drag (#97). Never both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Border(Border),
    /// Dragging the Panel (display index) by its title bar, to swap it with another.
    Panel(usize),
}

/// Where one Panel sits: its first column and row, and how many columns and rows it spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub column: usize,
    pub row: usize,
    pub columns: usize,
    pub rows: usize,
}

impl Place {
    fn covers(&self, column: usize, row: usize) -> bool {
        self.column <= column && column < self.column + self.columns && self.row <= row && row < self.row + self.rows
    }
}

/// The default layout, in whole numbers: columns and rows (as weights), then each Panel by display name at
/// `(column, row, columns, rows)`. 5 × 4: the clean picture 3 × 2 at the top left, the luma waveform and the
/// Vectorscope (two cells wide) to its right, the Colour Waveform and the Picture Views below, every cell used.
const DEFAULT_COLUMNS: [u32; 5] = [1, 1, 1, 1, 1];
const DEFAULT_ROWS: [u32; 4] = [14, 14, 11, 11];
const DEFAULT_PLACES: [(&str, [usize; 4]); 9] = [
    ("Clean picture", [0, 0, 3, 2]),
    ("Luma waveform", [3, 0, 2, 1]),
    ("Vectorscope", [3, 1, 2, 1]),
    ("Colour waveform", [0, 2, 2, 2]),
    ("Channel view", [2, 2, 1, 1]),
    ("ARRI False Colour", [3, 2, 2, 1]),
    ("Saturation heatmap", [2, 3, 1, 1]),
    ("Skin tone", [3, 3, 1, 1]),
    ("Stops False Colour", [4, 3, 1, 1]),
];

/// The layout: column and row weights (each adding up to 1), and each Panel's place, in display order.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelGrid {
    columns: Vec<f32>,
    rows: Vec<f32>,
    places: Vec<Place>,
}

fn normalised(whole: &[u32]) -> Vec<f32> {
    let sum: u32 = whole.iter().sum();
    whole.iter().map(|w| *w as f32 / sum as f32).collect()
}

impl PanelGrid {
    /// A plain grid for `panels` displays, every Panel and row the same size, rows as equal as possible and the
    /// last row's Panels spanning columns so every row fills the width. Used when the displays are not the ones
    /// the default layout is made for.
    pub fn automatic(panels: usize) -> PanelGrid {
        if panels == 0 {
            return PanelGrid { columns: Vec::new(), rows: Vec::new(), places: Vec::new() };
        }
        let across = ((panels as f32 * 16.0 / 9.0).sqrt().ceil() as usize).clamp(1, panels);
        let count = panels.div_ceil(across);
        let lengths: Vec<usize> = (0..count).map(|r| panels / count + usize::from(r < panels % count)).collect();
        let columns = *lengths.iter().max().unwrap_or(&1);
        let mut places = Vec::new();
        for (row, &n) in lengths.iter().enumerate() {
            let mut column = 0;
            for i in 0..n {
                let span = columns / n + usize::from(i < columns % n);
                places.push(Place { column, row, columns: span, rows: 1 });
                column += span;
            }
        }
        PanelGrid { columns: vec![1.0 / columns as f32; columns], rows: vec![1.0 / count as f32; count], places }
    }

    /// The default layout for the displays called `names` (in display order). Falls back to [`Self::automatic`]
    /// unless these are exactly the displays the default layout is made for, so every cell is used either way.
    pub fn default_for(names: &[&str]) -> PanelGrid {
        let places: Option<Vec<Place>> = names
            .iter()
            .map(|name| DEFAULT_PLACES.iter().find(|(n, _)| n == name).map(|(_, [column, row, columns, rows])| Place { column: *column, row: *row, columns: *columns, rows: *rows }))
            .collect();
        match places {
            Some(places) if places.len() == DEFAULT_PLACES.len() => PanelGrid { columns: normalised(&DEFAULT_COLUMNS), rows: normalised(&DEFAULT_ROWS), places },
            _ => PanelGrid::automatic(names.len()),
        }
    }

    /// Where each Panel sits, in display order.
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    /// How many columns and rows the grid has.
    pub fn size(&self) -> (usize, usize) {
        (self.columns.len(), self.rows.len())
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

    /// Each Panel's rectangle, in display order, filling `area` with `gap` between columns and rows. A Panel that
    /// spans several columns or rows takes their width or height and the gaps between them.
    pub fn cells(&self, area: [f32; 4], gap: f32) -> Vec<[f32; 4]> {
        let [left, top, width, height] = area;
        let columns = Self::spans(left, width, gap, &self.columns);
        let rows = Self::spans(top, height, gap, &self.rows);
        self.places
            .iter()
            .map(|p| {
                let (x, y) = (columns[p.column].0, rows[p.row].0);
                let (last_x, last_y) = (columns[p.column + p.columns - 1], rows[p.row + p.rows - 1]);
                [x, y, last_x.0 + last_x.1 - x, last_y.0 + last_y.1 - y]
            })
            .collect()
    }

    /// The track (column or row) whose span, or the gap after it, holds `at`.
    fn track_at(spans: &[(f32, f32)], at: f32) -> usize {
        spans.iter().rposition(|(start, _)| *start <= at).unwrap_or(0)
    }

    /// The border under `point`, if any: within `slop` of the gap between two columns or two rows, and not
    /// through the middle of a Panel that spans both sides.
    pub fn border_at(&self, area: [f32; 4], gap: f32, point: [f32; 2], slop: f32) -> Option<Border> {
        let [left, top, width, height] = area;
        let columns = Self::spans(left, width, gap, &self.columns);
        let rows = Self::spans(top, height, gap, &self.rows);
        if point[1] >= top && point[1] <= top + height {
            let row = Self::track_at(&rows, point[1]);
            for (after, pair) in columns.windows(2).enumerate() {
                let middle = pair[0].0 + pair[0].1 + gap / 2.0;
                let through_a_panel = self.places.iter().any(|p| p.covers(after, row) && p.covers(after + 1, row));
                if (point[0] - middle).abs() <= gap / 2.0 + slop && !through_a_panel {
                    return Some(Border::Column { after });
                }
            }
        }
        if point[0] >= left && point[0] <= left + width {
            let column = Self::track_at(&columns, point[0]);
            for (after, pair) in rows.windows(2).enumerate() {
                let middle = pair[0].0 + pair[0].1 + gap / 2.0;
                let through_a_panel = self.places.iter().any(|p| p.covers(column, after) && p.covers(column, after + 1));
                if (point[1] - middle).abs() <= gap / 2.0 + slop && !through_a_panel {
                    return Some(Border::Row { after });
                }
            }
        }
        None
    }

    /// What a press at `point` starts: a border drag when it is on a border (that wins), a Panel drag when it is on
    /// a Panel's title bar (at `scale` device pixels per logical pixel), else nothing.
    pub fn press_at(&self, area: [f32; 4], gap: f32, point: [f32; 2], slop: f32, scale: f32) -> Option<Press> {
        if let Some(border) = self.border_at(area, gap, point, slop) {
            return Some(Press::Border(border));
        }
        let panel = self.panel_at(area, gap, point)?;
        let t = PanelLayout::new(self.cells(area, gap)[panel], scale).title;
        let in_bar = point[0] >= t[0] && point[0] < t[0] + t[2] && point[1] >= t[1] && point[1] < t[1] + t[3];
        in_bar.then_some(Press::Panel(panel))
    }

    /// The Panel whose cell holds `point`, if any (a gap holds none).
    pub fn panel_at(&self, area: [f32; 4], gap: f32, point: [f32; 2]) -> Option<usize> {
        self.cells(area, gap).iter().position(|c| point[0] >= c[0] && point[0] < c[0] + c[2] && point[1] >= c[1] && point[1] < c[1] + c[3])
    }

    /// Swap two Panels: each takes the other's cell and span.
    pub fn swap(&mut self, a: usize, b: usize) {
        if a < self.places.len() && b < self.places.len() {
            self.places.swap(a, b);
        }
    }

    /// Move `border` to `to`: the columns or rows on each side take up the difference, and neither goes under
    /// `min` (width, height), so no Panel can be made smaller than its minimum.
    pub fn drag(&mut self, border: Border, to: [f32; 2], area: [f32; 4], gap: f32, min: [f32; 2]) {
        let [left, top, width, height] = area;
        let (spans, position, min, total, after) = match border {
            Border::Column { after } => (Self::spans(left, width, gap, &self.columns), to[0], min[0], width, after),
            Border::Row { after } => (Self::spans(top, height, gap, &self.rows), to[1], min[1], height, after),
        };
        let weights = match border {
            Border::Column { .. } => &mut self.columns,
            Border::Row { .. } => &mut self.rows,
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

    /// The layout, to remember between sessions: the weights, and each Panel's place by display name (`names` in
    /// display order).
    pub fn to_text(&self, names: &[&str]) -> String {
        let join = |w: &[f32]| w.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
        let mut text = format!("{HEADER}\ncolumns {}\nrows {}\n", join(&self.columns), join(&self.rows));
        for (p, name) in self.places.iter().zip(names) {
            text += &format!("panel {} {} {} {} {name}\n", p.column, p.row, p.columns, p.rows);
        }
        text
    }

    /// The layout `text` holds for the displays called `names`. A saved layout that is not this version's (a
    /// v0.2.0 grid), or does not make sense, is ignored: the default layout is used. A saved Panel that no longer
    /// exists is dropped; a display missing from the saved layout is placed in the first free cell.
    pub fn from_text(names: &[&str], text: &str) -> PanelGrid {
        Self::parse(names, text).unwrap_or_else(|| PanelGrid::default_for(names))
    }

    fn parse(names: &[&str], text: &str) -> Option<PanelGrid> {
        let mut lines = text.lines();
        if lines.next()? != HEADER {
            return None;
        }
        // Weights that add up to 1 (give or take rounding), none of which leaves a column or row under its
        // minimum size in the reference window; they are scaled to add up to exactly 1.
        let weights = |line: &str, key: &str, total: f32, min: f32| -> Option<Vec<f32>> {
            let values: Vec<f32> = line.strip_prefix(key)?.split_whitespace().map(|v| v.parse().ok()).collect::<Option<_>>()?;
            let sum: f32 = values.iter().sum();
            let usable = total - REFERENCE_GAP * values.len().saturating_sub(1) as f32;
            let sane = !values.is_empty() && values.iter().all(|v| v.is_finite() && *v > 0.0) && (sum - 1.0).abs() < 0.01 && values.iter().all(|v| v / sum * usable >= min);
            sane.then(|| values.iter().map(|v| v / sum).collect())
        };
        let columns = weights(lines.next()?, "columns ", REFERENCE_AREA[0], MIN_PANEL[0])?;
        let mut rows = weights(lines.next()?, "rows ", REFERENCE_AREA[1], MIN_PANEL[1])?;
        let mut placed: Vec<Option<Place>> = vec![None; names.len()];
        for line in lines {
            let mut parts = line.strip_prefix("panel ")?.splitn(5, ' ');
            let mut number = || parts.next()?.parse::<usize>().ok();
            let (column, row, across, down) = (number()?, number()?, number()?, number()?);
            let name = parts.next()?;
            let place = Place { column, row, columns: across, rows: down };
            if place.columns == 0 || place.rows == 0 || place.column + place.columns > columns.len() || place.row + place.rows > rows.len() {
                return None;
            }
            // A Panel that no longer exists is dropped; one named twice makes the file rubbish.
            if let Some(i) = names.iter().position(|n| *n == name) {
                if placed[i].replace(place).is_some() {
                    return None;
                }
            }
        }
        let mut used = vec![vec![false; columns.len()]; rows.len()];
        for p in placed.iter().flatten() {
            for r in p.row..p.row + p.rows {
                for c in p.column..p.column + p.columns {
                    if std::mem::replace(&mut used[r][c], true) {
                        return None;
                    }
                }
            }
        }
        // A display the saved layout does not know takes the first free cell (reading order), or a new row.
        for slot in placed.iter_mut().filter(|p| p.is_none()) {
            let free = (0..rows.len()).flat_map(|r| (0..columns.len()).map(move |c| (r, c))).find(|&(r, c)| !used[r][c]);
            let (row, column) = match free {
                Some(cell) => cell,
                None => {
                    let before = rows.len() as f32;
                    rows.iter_mut().for_each(|w| *w *= before / (before + 1.0));
                    rows.push(1.0 / (before + 1.0));
                    used.push(vec![false; columns.len()]);
                    (rows.len() - 1, 0)
                }
            };
            // A display with no free cell gets a new row to itself, across the whole width.
            let columns_wide = if free.is_some() { 1 } else { columns.len() };
            used[row][column..column + columns_wide].fill(true);
            *slot = Some(Place { column, row, columns: columns_wide, rows: 1 });
        }
        // A saved layout that leaves a cell empty no longer fits these displays (a Panel went and left a hole, as
        // when the two vectorscope Panels became one): the default is used.
        if used.iter().flatten().any(|u| !u) {
            return None;
        }
        Some(PanelGrid { columns, rows, places: placed.into_iter().flatten().collect() })
    }
}

/// What sits inside one Panel: a thin border round it, a title bar at its top holding the label, and the display
/// filling the rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelLayout {
    /// The Panel's whole cell: the border is drawn round its edge.
    pub frame: [f32; 4],
    /// The title bar, inside the border.
    pub title: [f32; 4],
    /// The display, under the title bar and inside the border (at least a pixel).
    pub content: [f32; 4],
}

impl PanelLayout {
    /// The layout inside `cell` at `scale` device pixels per logical pixel. The title bar is one line high and the
    /// label is clipped to it, so a long label never takes room from the display.
    pub fn new(cell: [f32; 4], scale: f32) -> PanelLayout {
        let [left, top, width, height] = cell;
        let border = style::LINE_WEIGHT * scale;
        let bar = TITLE_BAR * scale;
        let inner_width = (width - 2.0 * border).max(1.0);
        PanelLayout {
            frame: cell,
            title: [left + border, top + border, inner_width, bar],
            content: [left + border, top + border + bar + border, inner_width, (height - 3.0 * border - bar).max(1.0)],
        }
    }

    /// The border as four thin rectangles (top, bottom, left, right) of `weight` device pixels, and the line under
    /// the title bar: the rectangles the window draws for this Panel.
    pub fn lines(&self, weight: f32) -> [[f32; 4]; 5] {
        let [left, top, width, height] = self.frame;
        let [title_left, title_top, title_width, title_height] = self.title;
        [
            [left, top, width, weight],
            [left, top + height - weight, width, weight],
            [left, top, weight, height],
            [left + width - weight, top, weight, height],
            [title_left, title_top + title_height, title_width, weight],
        ]
    }
}
