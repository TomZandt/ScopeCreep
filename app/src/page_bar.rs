//! The Page bar (#212, spec #196, ADR 0041): between the Colour Setup line and the Panels, a tab for each page, a **+**
//! after the last one and the Auto resize and Add panel buttons at its right end. The first nine tabs read "[1] Colour": the number is
//! the tab's position, so the Ctrl+number that goes to it shows, and a tenth page onward has none. Pure: no GPU, no
//! window. Tabs, "+" and Add panel are chips, so they draw through the style tokens.
//!
//! The bar always shows the current page's tab: with more tabs than room, it starts at the first tab that lets the
//! current one fit and drops the ones that do not fit on the right (the Layout menu lists every page).
use crate::components::{Chip, MenuBar, Prim, State, contains};
use crate::pages::{NUMBERED, PageRename};
use crate::style;

/// The texts of the two buttons and their tips.
pub const PLUS_CHIP: &str = "+";
pub const PLUS_TOOLTIP: &str = "New page from…";
pub const ADD_CHIP: &str = "Add panel";
pub const ADD_TOOLTIP: &str = "Add a Panel to this page";
pub const RESIZE_CHIP: &str = "Auto resize";
pub const RESIZE_TOOLTIP: &str = "Resize this page's splits to fit the footage";

/// What a click on the bar can hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarPart {
    /// The tab of page `.0`.
    Tab(usize),
    /// The **+**: New page from.
    Plus,
    /// The Add panel button.
    AddPanel,
    /// The Auto resize button (#213).
    AutoResize,
}

/// Where the bar's parts are (device pixels): also their hit areas.
#[derive(Clone, Debug, PartialEq)]
pub struct BarLayout {
    /// The whole bar.
    pub bar: [f32; 4],
    /// The tabs that fit: each page's number and where its tab is, left to right.
    pub tabs: Vec<(usize, [f32; 4])>,
    pub plus: [f32; 4],
    pub add_panel: [f32; 4],
    pub auto_resize: [f32; 4],
}

/// What the Page bar says now.
#[derive(Clone, Debug, PartialEq)]
pub struct PageBar {
    /// Every page's name, in tab order.
    pub names: Vec<String>,
    /// The shown page.
    pub current: usize,
    /// A name being typed in its tab, if any.
    pub rename: Option<PageRename>,
}

/// What a tab says for page `index` named `name`: "[3] Name" for the first nine pages, the name alone beyond.
pub fn tab_label(index: usize, name: &str) -> String {
    if index < NUMBERED {
        format!("[{}] {}", index + 1, name)
    } else {
        name.to_string()
    }
}

impl PageBar {
    /// The bar's height at `scale`.
    pub fn height(scale: f32) -> f32 {
        style::SPACE_PAGE_BAR * scale
    }

    /// What the tab of page `index` says: the label, or while it is being renamed the name typed so far with a
    /// caret, the number left out.
    pub fn label(&self, index: usize) -> String {
        match &self.rename {
            Some(r) if r.page == index => format!("{}|", r.text),
            _ => tab_label(index, self.names.get(index).map_or("", String::as_str)),
        }
    }

    /// The least width of a Window the bar fits in at `scale`: the Add panel button, the **+** and one tab.
    pub fn min_width(scale: f32) -> f32 {
        let gap = style::SPACE_CHIP_GAP * scale;
        2.0 * style::SPACE_BAR_PAD * scale
            + Chip::new(ADD_CHIP).size(scale)[0]
            + Chip::new(RESIZE_CHIP).size(scale)[0]
            + Chip::new(PLUS_CHIP).size(scale)[0]
            + Chip::new(&tab_label(0, crate::pages::MAIN)).size(scale)[0]
            + 3.0 * gap
    }

    /// Where the parts are, for a bar `width` wide with its top at `top`.
    pub fn layout(&self, top: f32, width: f32, scale: f32) -> BarLayout {
        let gap = style::SPACE_CHIP_GAP * scale;
        let pad = style::SPACE_BAR_PAD * scale;
        let h = Self::height(scale);
        let chip_top = top + (h - style::SPACE_CHIP_HEIGHT * scale) / 2.0;
        let add = Chip::new(ADD_CHIP);
        let add_panel = add.rect([width - pad - add.size(scale)[0], chip_top], scale);
        let resize = Chip::new(RESIZE_CHIP);
        let auto_resize = resize.rect(
            [add_panel[0] - gap - resize.size(scale)[0], chip_top],
            scale,
        );
        let plus_width = Chip::new(PLUS_CHIP).size(scale)[0];
        // Room for tabs: up to the Add panel button, leaving a gap, with the + after the last tab.
        let limit = auto_resize[0] - gap - plus_width - gap;
        let widths: Vec<f32> = (0..self.names.len())
            .map(|i| Chip::new(&self.label(i)).size(scale)[0])
            .collect();
        let current = self.current.min(widths.len().saturating_sub(1));
        // The first tab shown: the lowest that lets the current tab fit.
        let used = |from: usize, to: usize| -> f32 {
            widths[from..=to].iter().sum::<f32>() + gap * (to - from) as f32
        };
        let mut first = 0;
        if !widths.is_empty() {
            while first < current && pad + used(first, current) > limit {
                first += 1;
            }
        }
        let mut tabs = Vec::new();
        let mut x = pad;
        for (i, w) in widths.iter().enumerate().skip(first) {
            if x + w > limit && i > current {
                break;
            }
            tabs.push((i, [x, chip_top, *w, style::SPACE_CHIP_HEIGHT * scale]));
            x += w + gap;
        }
        let plus = Chip::new(PLUS_CHIP).rect([x, chip_top], scale);
        BarLayout {
            bar: [0.0, top, width, h],
            tabs,
            plus,
            add_panel,
            auto_resize,
        }
    }

    /// The part under `point`.
    pub fn hit(&self, top: f32, width: f32, point: [f32; 2], scale: f32) -> Option<BarPart> {
        let l = self.layout(top, width, scale);
        if contains(l.add_panel, point) {
            Some(BarPart::AddPanel)
        } else if contains(l.auto_resize, point) {
            Some(BarPart::AutoResize)
        } else if contains(l.plus, point) {
            Some(BarPart::Plus)
        } else {
            l.tabs
                .iter()
                .find(|(_, r)| contains(*r, point))
                .map(|(i, _)| BarPart::Tab(*i))
        }
    }

    /// The bar: its fill and the line under it, the current tab lit, the part under the pointer in its hover.
    pub fn draw(&self, top: f32, width: f32, hover: Option<BarPart>, scale: f32) -> Vec<Prim> {
        let l = self.layout(top, width, scale);
        let line = style::LINE_WEIGHT * scale;
        let h = Self::height(scale);
        let state = |part: BarPart| {
            if hover == Some(part) {
                State::Hover
            } else {
                State::Normal
            }
        };
        let mut prims = vec![
            crate::components::rect(l.bar, style::SURFACE_PANEL),
            crate::components::rect([0.0, top + h - line, width, line], style::LINE_BORDER),
        ];
        for (i, r) in &l.tabs {
            prims.extend(Chip::new(&self.label(*i)).draw(
                [r[0], r[1]],
                *i == self.current,
                state(BarPart::Tab(*i)),
                scale,
            ));
        }
        prims.extend(Chip::new(PLUS_CHIP).draw(
            [l.plus[0], l.plus[1]],
            false,
            state(BarPart::Plus),
            scale,
        ));
        prims.extend(Chip::new(RESIZE_CHIP).draw(
            [l.auto_resize[0], l.auto_resize[1]],
            false,
            state(BarPart::AutoResize),
            scale,
        ));
        prims.extend(Chip::new(ADD_CHIP).draw(
            [l.add_panel[0], l.add_panel[1]],
            false,
            state(BarPart::AddPanel),
            scale,
        ));
        prims
    }
}

/// The menu bar's row and the Colour Setup line above the Page bar, at `scale`: where the Page bar's top is.
pub fn top(scale: f32) -> f32 {
    MenuBar::height(scale) + crate::components::BarKind::Top.height(scale)
}
