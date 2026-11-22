//! The tab strip in a tab group's title bar (ADR 0038, #211): a tab per Panel with the shown one highlighted, in the
//! room the name has. A tab group of one Panel has no strip and looks as a Panel always has. When the tabs do not
//! all fit at the least tab width the shown tab and as many others as fit stay, and an overflow chip says how
//! many are out of sight (a click on it shows the next of them). Pure: no GPU, no window, no fonts.
//! Rectangles are left, top, width, height in device pixels.

use crate::components::{Align, Face, Prim, contains, rect, text, text_width};
use crate::style;

/// The tabs of one tab group, by label, and which one is shown.
#[derive(Clone, Debug, PartialEq)]
pub struct TabStrip {
    pub labels: Vec<String>,
    pub active: usize,
}

/// What a point on the strip is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabHit {
    /// Tab number `n` of the group.
    Tab(usize),
    /// The overflow chip.
    Overflow,
}

/// Where the parts of a strip are.
#[derive(Clone, Debug, PartialEq)]
pub struct TabStripLayout {
    /// The tabs that are in sight, in tab order, each with its number in the group and its rectangle (also its hit
    /// area).
    pub tabs: Vec<(usize, [f32; 4])>,
    /// The tabs out of sight, in tab order.
    pub hidden: Vec<usize>,
    /// The overflow chip, when some tabs are out of sight.
    pub overflow: Option<[f32; 4]>,
}

/// The room a title bar leaves for the tab strip: from the left of the bar to the first chip or the settings
/// button, less the gap kept before them.
pub fn room(title: &crate::components::TitleLayout, scale: f32) -> [f32; 4] {
    let left = title.name[0];
    let right = title
        .chips
        .first()
        .map(|c| c[0])
        .or(title.view.map(|v| v.fit[0]))
        .or(title.settings.map(|s| s[0]))
        .map_or(title.bar[0] + title.bar[2], |x| {
            x - style::SPACE_TAG_GAP * scale
        });
    [left, title.bar[1], (right - left).max(0.0), title.bar[3]]
}

impl TabStrip {
    /// A tab's natural width: its label and the padding, kept between the least and greatest tab width.
    fn natural(&self, label: &str, scale: f32) -> f32 {
        (text_width(label, scale) + 2.0 * style::SPACE_TAB_PAD * scale).clamp(
            style::SPACE_TAB_MIN_WIDTH * scale,
            style::SPACE_TAB_MAX_WIDTH * scale,
        )
    }

    /// The strip laid out in `area` (a title bar's room for it). Tabs fill the bar's height less the line under it.
    /// When they all fit at their natural width they take it; when they fit only if squeezed they share the room
    /// equally (not under the least width); when not even that, the overflow applies.
    /// The width the strip takes when every tab has its natural width.
    pub fn natural_width(&self, scale: f32) -> f32 {
        let gaps = style::SPACE_TAB_GAP * scale * self.labels.len().saturating_sub(1) as f32;
        gaps + self
            .labels
            .iter()
            .map(|l| self.natural(l, scale))
            .sum::<f32>()
    }

    pub fn layout(&self, area: [f32; 4], scale: f32) -> TabStripLayout {
        let n = self.labels.len();
        let gap = style::SPACE_TAB_GAP * scale;
        let min = style::SPACE_TAB_MIN_WIDTH * scale;
        let top = area[1];
        let height = (area[3] - style::LINE_WEIGHT * scale).max(0.0);
        let active = self.active.min(n.saturating_sub(1));
        let widths: Vec<f32> = self.labels.iter().map(|l| self.natural(l, scale)).collect();
        let gaps = |k: usize| gap * k.saturating_sub(1) as f32;
        let natural: f32 = widths.iter().sum::<f32>() + gaps(n);
        let (shown, tab_widths): (Vec<usize>, Vec<f32>) = if natural <= area[2] {
            ((0..n).collect(), widths)
        } else if n as f32 * min + gaps(n) <= area[2] {
            let each = ((area[2] - gaps(n)) / n as f32).min(style::SPACE_TAB_MAX_WIDTH * scale);
            ((0..n).collect(), vec![each; n])
        } else {
            // As many tabs as fit beside the overflow chip, at the least width; the shown tab is always one.
            let chip = style::SPACE_TAB_OVERFLOW_WIDTH * scale + gap;
            let k = (((area[2] - chip + gap) / (min + gap)).floor().max(1.0) as usize).min(n);
            let mut keep: Vec<usize> = (0..n).filter(|i| *i != active).take(k - 1).collect();
            keep.push(active);
            keep.sort_unstable();
            let each = min.min((area[2] - chip).max(0.0));
            (keep, vec![each; k])
        };
        let mut x = area[0];
        let tabs: Vec<(usize, [f32; 4])> = shown
            .iter()
            .zip(&tab_widths)
            .map(|(i, w)| {
                let r = [x, top, *w, height];
                x += w + gap;
                (*i, r)
            })
            .collect();
        let hidden: Vec<usize> = (0..n).filter(|i| !shown.contains(i)).collect();
        let overflow = (!hidden.is_empty()).then(|| {
            let w = style::SPACE_TAB_OVERFLOW_WIDTH * scale;
            [x, top, w, height]
        });
        TabStripLayout {
            tabs,
            hidden,
            overflow,
        }
    }

    /// What is under `point`, if anything.
    pub fn hit(&self, area: [f32; 4], point: [f32; 2], scale: f32) -> Option<TabHit> {
        let l = self.layout(area, scale);
        if let Some(o) = l.overflow
            && contains(o, point)
        {
            return Some(TabHit::Overflow);
        }
        l.tabs
            .iter()
            .find(|(_, r)| contains(*r, point))
            .map(|(i, _)| TabHit::Tab(*i))
    }

    /// The tab the overflow chip shows: the first out of sight after the shown one, else the first out of sight.
    pub fn next_hidden(&self, hidden: &[usize]) -> Option<usize> {
        hidden
            .iter()
            .copied()
            .find(|i| *i > self.active)
            .or_else(|| hidden.first().copied())
    }

    /// The strip drawn, with the tab under the pointer (`hover`) lit.
    pub fn draw(&self, area: [f32; 4], hover: Option<TabHit>, scale: f32) -> Vec<Prim> {
        let l = self.layout(area, scale);
        let underline = style::SPACE_TAB_UNDERLINE * scale;
        let mut prims = Vec::new();
        for (i, r) in &l.tabs {
            let on = *i == self.active;
            if on {
                prims.push(rect(*r, style::SURFACE_CONTROL));
                prims.push(rect(
                    [r[0], r[1] + r[3] - underline, r[2], underline],
                    style::accent(),
                ));
            } else if hover == Some(TabHit::Tab(*i)) {
                prims.push(rect(*r, style::STATE_HOVER));
            }
            let hovered = hover == Some(TabHit::Tab(*i));
            let ink = if on {
                style::accent_text()
            } else if hovered {
                style::TEXT_HIGH
            } else {
                style::TEXT_SECONDARY
            };
            prims.push(text(&self.labels[*i], *r, Face::Label, ink, Align::Centre));
        }
        if let Some(o) = l.overflow {
            if hover == Some(TabHit::Overflow) {
                prims.push(rect(o, style::STATE_HOVER));
            }
            prims.push(text(
                &format!("+{}", l.hidden.len()),
                o,
                Face::Label,
                style::TEXT_MUTED,
                Align::Centre,
            ));
        }
        prims
    }
}
