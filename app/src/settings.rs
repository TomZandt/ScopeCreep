//! Panel settings and the right-click menu that edits them (spec #49, "Settings"). Pure: no GPU, no window.
//!
//! A display lists the settings it offers as [`SettingDef`]s: either a short list of choices (the first is the
//! default) or an [`Amount`] set with a slider, so only sensible values can be picked. The user's choices are kept per Panel in [`PanelSettings`],
//! and remembered between sessions as text. [`Menu`] is the right-click menu for one Panel: where its rows go and
//! which choice a click lands on.
use std::collections::BTreeMap;

/// One value a setting can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    /// What is stored, and what the display reads.
    pub value: &'static str,
    /// What the menu says.
    pub text: &'static str,
}

/// An amount: a number in a range, set with a slider (spec #83, ADR 0013). Whatever is stored, a reader gets a
/// value in the range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Amount {
    pub min: f32,
    pub max: f32,
    /// What the slider moves in: a dragged value is a multiple of it from `min`.
    pub step: f32,
    pub default: f32,
    /// Shown after the number, with its space (" %").
    pub unit: &'static str,
    /// Digits after the decimal point in the number shown.
    pub decimals: usize,
    /// Values an earlier version saved as a list choice, and the number each one produced.
    pub legacy: &'static [(&'static str, f32)],
}

impl Amount {
    /// The valid value for what was stored (`None`, or something unreadable, is the default; a leftover list
    /// value is the number it produced; a number out of the range is clamped).
    pub fn read(&self, stored: Option<&str>) -> f32 {
        let Some(stored) = stored else {
            return self.default;
        };
        let number = self
            .legacy
            .iter()
            .find(|(old, _)| *old == stored)
            .map(|(_, v)| *v)
            .or_else(|| stored.parse::<f32>().ok().filter(|v| v.is_finite()));
        number.map_or(self.default, |v| v.clamp(self.min, self.max))
    }

    /// The nearest step to `fraction` (0 to 1) along the slider.
    pub fn at_fraction(&self, fraction: f32) -> f32 {
        let steps = ((fraction.clamp(0.0, 1.0) * (self.max - self.min)) / self.step).round();
        (self.min + steps * self.step).clamp(self.min, self.max)
    }

    /// How far along the slider (0 to 1) `value` is.
    pub fn fraction(&self, value: f32) -> f32 {
        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    /// The number with its unit, as the slider and the labels show it.
    pub fn text(&self, value: f32) -> String {
        format!("{value:.*}{}", self.decimals, self.unit)
    }

    /// How a value is kept in the saved text.
    pub fn store(value: f32) -> String {
        format!("{value}")
    }
}

/// A setting a display offers: a short list of choices (the first is the default), or an amount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingDef {
    pub key: &'static str,
    /// The menu's heading for the setting.
    pub label: &'static str,
    /// An amount, set with a slider; `choices` is then empty.
    pub amount: Option<Amount>,
    pub choices: &'static [Choice],
}

/// A Panel's settings as a display reads them: every setting it offers, each with a valid value.
pub type Settings = BTreeMap<String, String>;

/// Every Panel's settings, by the display's name. Holds only what the user chose; a Panel that has never been
/// set up is on its defaults.
#[derive(Clone, Debug, Default)]
pub struct PanelSettings {
    panels: BTreeMap<String, BTreeMap<String, String>>,
    /// What a display remembers that the menu does not offer (a Loupe's position and zoom): free values the
    /// display checks itself (ADR 0015). Kept apart so that changing it never re-measures the frame.
    state: BTreeMap<String, BTreeMap<String, String>>,
    /// Counts the menu choices that changed, so a reader can tell cheaply whether anything did.
    version: u64,
    /// The same for remembered state, which labels may show (a Loupe's zoom) but which never re-measures anything.
    state_version: u64,
}

/// Equal when the menu's choices are: remembered state is not a setting.
impl PartialEq for PanelSettings {
    fn eq(&self, other: &PanelSettings) -> bool {
        self.panels == other.panels
    }
}
impl Eq for PanelSettings {}

/// How remembered state is marked in the saved text.
const STATE_PREFIX: &str = "state:";

impl PanelSettings {
    pub fn set(&mut self, panel: &str, key: &str, value: &str) {
        let old = self
            .panels
            .entry(panel.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
        if old.as_deref() != Some(value) {
            self.version += 1;
        }
    }

    /// Give Panel `to` a copy of Panel `from`'s settings and remembered state, replacing whatever it had (Duplicate,
    /// ADR 0039). The settings count as changed when there is anything to copy.
    pub fn copy_panel(&mut self, from: &str, to: &str) {
        self.forget_panel(to);
        if let Some(own) = self.panels.get(from).cloned() {
            self.panels.insert(to.to_string(), own);
            self.version += 1;
        }
        if let Some(own) = self.state.get(from).cloned() {
            self.state.insert(to.to_string(), own);
            self.state_version += 1;
        }
    }

    /// Put every setting of Panel `panel` back to its default (the Panel reset, #225). What the display remembers
    /// for itself (a Loupe's zoom and position) and where the Panel sits are not settings and stay.
    pub fn reset_settings(&mut self, panel: &str) {
        if self.panels.remove(panel).is_some() {
            self.version += 1;
        }
    }

    /// Forget everything kept for Panel `panel` (it was closed, or replaced, ADR 0039): it starts from the defaults
    /// if it is ever made again.
    pub fn forget_panel(&mut self, panel: &str) {
        if self.panels.remove(panel).is_some() {
            self.version += 1;
        }
        if self.state.remove(panel).is_some() {
            self.state_version += 1;
        }
    }

    /// Goes up whenever a menu choice changes (not remembered state): equal versions of the same settings are equal.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Remember a value a display keeps for itself (not a menu choice). Returns whether it changed.
    pub fn set_state(&mut self, panel: &str, key: &str, value: &str) -> bool {
        let old = self
            .state
            .entry(panel.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
        let changed = old.as_deref() != Some(value);
        if changed {
            self.state_version += 1;
        }
        changed
    }

    /// Goes up whenever remembered state changes.
    pub fn state_version(&self) -> u64 {
        self.state_version
    }

    /// `panel`'s remembered values for `keys`, as a display reads them; those never remembered are left out.
    pub fn state_of(&self, panel: &str, keys: &[&str]) -> Settings {
        let Some(own) = self.state.get(panel) else {
            return Settings::new();
        };
        keys.iter()
            .filter_map(|k| own.get(*k).map(|v| (k.to_string(), v.clone())))
            .collect()
    }

    /// `panel`'s settings for the settings it offers (`defs`): the user's choice, or the default when there is
    /// none or it is no longer a choice (a remembered value from another version never reaches a display).
    pub fn resolve(&self, panel: &str, defs: &[SettingDef]) -> Settings {
        let chosen = self.panels.get(panel);
        defs.iter()
            .filter_map(|def| {
                if let Some(amount) = &def.amount {
                    return Some((
                        def.key.to_string(),
                        Amount::store(
                            amount.read(chosen.and_then(|c| c.get(def.key)).map(String::as_str)),
                        ),
                    ));
                }
                let wanted = chosen.and_then(|c| c.get(def.key));
                let choice = def
                    .choices
                    .iter()
                    .find(|c| Some(c.value) == wanted.map(String::as_str))
                    .or(def.choices.first())?;
                Some((def.key.to_string(), choice.value.to_string()))
            })
            .collect()
    }

    /// The text kept between sessions: one `panel<TAB>key<TAB>value` line per choice.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for (panel, settings) in &self.panels {
            for (key, value) in settings {
                text += &format!("{panel}\t{key}\t{value}\n");
            }
        }
        for (panel, state) in &self.state {
            for (key, value) in state {
                text += &format!("{panel}\t{STATE_PREFIX}{key}\t{value}\n");
            }
        }
        text
    }

    /// Settings from [`PanelSettings::to_text`]. Lines that are not a setting are ignored.
    pub fn from_text(text: &str) -> PanelSettings {
        let mut settings = PanelSettings::default();
        // The clean picture's B/W toggle became its channel (#160): "on" was Luma, "off" was colour.
        let mut legacy_luma = false;
        for line in text.lines() {
            let mut parts = line.split('\t');
            if let (Some(panel), Some(key), Some(value), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            {
                // The milestone 2 vectorscope Panels became one (#92): the 100 % Panel's settings are the one
                // Panel's, and the 200 % Panel's are ignored.
                let panel = match panel {
                    "Vectorscope 100 %" => "Vectorscope",
                    "Vectorscope 200 %" => continue,
                    other => other,
                };
                let panel = modern_key(panel);
                let panel = panel.as_str();
                if (panel, key) == ("Channel Viewer", "bw") {
                    legacy_luma |= value == "on";
                    continue;
                }
                if !panel.is_empty() && !key.is_empty() && !value.is_empty() {
                    match key.strip_prefix(STATE_PREFIX) {
                        Some(key) => {
                            settings.set_state(panel, key, value);
                        }
                        None => settings.set(panel, key, value),
                    }
                }
            }
        }
        if legacy_luma
            && !settings
                .panels
                .get("Channel Viewer")
                .is_some_and(|p| p.contains_key("channel"))
        {
            settings.set("Channel Viewer", "channel", "luma");
        }
        settings
    }
}

/// The displays' former names (before the rename of spec #196's follow-up), and what each is called now. Saved files
/// written under a former name are still read: [`modern_key`] gives the name a key is kept under today, and files are
/// written with the new names from the next save on. See the Displays table in GLOSSARY.md.
pub const FORMER_NAMES: [(&str, &str); 9] = [
    ("Clean picture", "Channel Viewer"),
    ("Luma waveform", "Waveform"),
    ("ARRI False Colour", "False Colour - Arri"),
    ("Stops False Colour", "False Colour - Stops"),
    ("Skin tone", "Skin Tone"),
    ("Saturation heatmap", "Saturation"),
    ("Gamut/legal warning", "Gamut Warning"),
    ("Highlight neutrals", "Neutrals"),
    ("CIE chromaticity", "Chromaticity"),
];

/// The key `key` is kept under today: a display's former name ("Clean picture") or a further instance of it
/// ("Clean picture 2") becomes the new name ("Channel Viewer", "Channel Viewer 2"); any other key is returned as is.
pub fn modern_key(key: &str) -> String {
    for (old, new) in FORMER_NAMES {
        if key == old {
            return new.to_string();
        }
        if let Some(number) = key.strip_prefix(old).and_then(|r| r.strip_prefix(' '))
            && !number.is_empty()
            && number.bytes().all(|b| b.is_ascii_digit())
        {
            return format!("{new} {number}");
        }
    }
    key.to_string()
}

/// The setting every picture Panel with a legend calls it (ARRI False Colour, Stops False Colour, the saturation
/// heatmap), with the choices "on" and "off" (#191).
pub const LEGEND_KEY: &str = "legend";
/// The two values of the legend setting.
pub const LEGEND_ON: &str = "on";
pub const LEGEND_OFF: &str = "off";

/// Whether `def` is a Panel's legend on/off setting.
fn is_legend(def: &SettingDef) -> bool {
    def.key == LEGEND_KEY
        && def.amount.is_none()
        && def.choices.iter().any(|c| c.value == LEGEND_ON)
        && def.choices.iter().any(|c| c.value == LEGEND_OFF)
}

impl PanelSettings {
    /// Whether every legend of `panels` (each a display's name and the settings it offers) is on, defaults
    /// included. True when there is none.
    pub fn legends_on(&self, panels: &[(&str, &[SettingDef])]) -> bool {
        panels.iter().all(|(name, defs)| {
            !defs.iter().any(is_legend)
                || self.resolve(name, defs).get(LEGEND_KEY).map(String::as_str) == Some(LEGEND_ON)
        })
    }

    /// Set the legend of every Panel in `panels` that has one, on or off, as its own popover would.
    pub fn set_legends(&mut self, panels: &[(&str, &[SettingDef])], on: bool) {
        for (name, defs) in panels {
            if defs.iter().any(is_legend) {
                self.set(name, LEGEND_KEY, if on { LEGEND_ON } else { LEGEND_OFF });
            }
        }
    }
}

/// Logical pixels: the popover's size (#225).
pub const MENU_WIDTH: f32 = crate::style::SPACE_POPOVER_WIDTH;
pub const MENU_ROW: f32 = crate::style::SPACE_POPOVER_ROW;
pub const MENU_PAD: f32 = crate::style::SPACE_POPOVER_PAD;

/// The heading of the Panel section that ends every popover (ADR 0039); it carries the Panel-wide reset.
pub const PANEL_SECTION: &str = "Panel";

/// What one row of the menu says.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuRow {
    /// A setting's name; not clickable. A group starts with one.
    Heading(String),
    /// A choice; `on` when it is the setting's current value.
    Choice {
        key: &'static str,
        value: &'static str,
        text: String,
        on: bool,
    },
    /// An amount, all in one row (#225): its label, a slider, a value box with arrows and a reset icon.
    Slider {
        key: &'static str,
        label: &'static str,
        amount: Amount,
        value: f32,
    },
    /// Closes the Panel (ADR 0038, #206): the last row of every Panel's popover.
    Close,
    /// One of the Panel section's other entries (ADR 0039, #208).
    Panel(PanelAction),
}

/// What the Panel section of a popover does besides Close (ADR 0039, #208).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelAction {
    /// Opens the list of displays to replace this Panel's display with.
    ReplaceWith,
    /// A new Panel of the same display to the right of this one, with a copy of its settings.
    DuplicateRight,
    /// The same, below.
    DuplicateBelow,
    /// In the Replace with list: replace this Panel with the display called so.
    ReplaceBy(String),
    /// In the Replace with list: back to the popover.
    Back,
}

impl PanelAction {
    /// What the row says.
    pub fn text(&self) -> String {
        match self {
            PanelAction::ReplaceWith => "Replace with\u{2026}".to_string(),
            PanelAction::DuplicateRight => "Duplicate right".to_string(),
            PanelAction::DuplicateBelow => "Duplicate below".to_string(),
            PanelAction::ReplaceBy(name) => name.clone(),
            PanelAction::Back => "Back".to_string(),
        }
    }
}

/// An amount that changed: the setting and its new value (already clamped to the slider's range).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub key: &'static str,
    pub value: f32,
}

/// The parts of an amount row a pointer can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmountHit {
    /// The slider's track or thumb.
    Track,
    /// The value box's increase arrow.
    Up,
    /// The value box's decrease arrow.
    Down,
    /// The value box's number: a press types into it.
    Field,
    /// The circular reset icon.
    Reset,
}

/// A key the value box takes while it is being typed into (#225).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    /// A typed character (only digits, a decimal point and a minus sign are kept).
    Char(char),
    Backspace,
    /// One step up; `true` with Shift is ten steps.
    Up(bool),
    Down(bool),
    /// Accept the typed number.
    Enter,
    /// Cancel: the value goes back to what it was when typing began.
    Escape,
}

/// What a press in the popover did to the amounts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pressed {
    /// The press was on one of the amount controls or the Panel reset (so nothing else is to be done with it).
    pub consumed: bool,
    /// The amounts that changed, in order, each to be remembered as a setting.
    pub changes: Vec<Change>,
    /// The Panel-wide reset was pressed while it was enabled.
    pub reset_panel: bool,
}

/// The value box being typed into.
#[derive(Clone, Debug, PartialEq)]
struct Edit {
    key: &'static str,
    text: String,
    original: f32,
    /// Nothing typed yet: the first character replaces the whole number.
    fresh: bool,
}

/// The right-click menu of one Panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    /// The Panel (its index in the grid) the menu belongs to.
    pub panel: usize,
    pub rows: Vec<MenuRow>,
    /// Whether every one of the Panel's settings is at its default: the Panel reset is dim when so. The caller
    /// sets it (a Panel's chips are settings the popover does not list).
    pub panel_at_default: bool,
    /// The amount whose slider is being dragged.
    drag: Option<&'static str>,
    edit: Option<Edit>,
}

impl Menu {
    fn with_rows(panel: usize, rows: Vec<MenuRow>) -> Menu {
        Menu {
            panel,
            rows,
            panel_at_default: true,
            drag: None,
            edit: None,
        }
    }

    /// The menu for a display with `defs`, showing `current`. `None` when it has no settings: such a Panel shows
    /// no menu.
    pub fn new(panel: usize, defs: &[SettingDef], current: &Settings) -> Option<Menu> {
        if defs.is_empty() {
            return None;
        }
        let mut rows = Vec::new();
        for def in defs {
            if let Some(amount) = def.amount {
                let value = amount.read(current.get(def.key).map(String::as_str));
                rows.push(MenuRow::Slider {
                    key: def.key,
                    label: def.label,
                    amount,
                    value,
                });
                continue;
            }
            rows.push(MenuRow::Heading(def.label.to_string()));
            for choice in def.choices {
                rows.push(MenuRow::Choice {
                    key: def.key,
                    value: choice.value,
                    text: choice.text.to_string(),
                    on: current.get(def.key).map(String::as_str) == Some(choice.value),
                });
            }
        }
        let mut menu = Menu::with_rows(panel, rows);
        menu.panel_at_default = Menu::at_default(defs, current);
        Some(menu)
    }

    /// Whether every setting in `defs` reads its default in `current`.
    pub fn at_default(defs: &[SettingDef], current: &Settings) -> bool {
        defs.iter().all(|def| match def.amount {
            Some(amount) => amount.read(current.get(def.key).map(String::as_str)) == amount.default,
            None => def
                .choices
                .first()
                .is_none_or(|first| current.get(def.key).is_none_or(|v| v == first.value)),
        })
    }

    /// A Panel's popover: its settings (when it has any) and, last, the Panel section with Close (#206). A Panel
    /// with no settings still has a popover, so it can be closed.
    pub fn for_panel(panel: usize, defs: &[SettingDef], current: &Settings) -> Menu {
        let mut menu =
            Menu::new(panel, defs, current).unwrap_or_else(|| Menu::with_rows(panel, Vec::new()));
        menu.rows.push(MenuRow::Heading(PANEL_SECTION.to_string()));
        menu.rows.push(MenuRow::Panel(PanelAction::ReplaceWith));
        menu.rows.push(MenuRow::Panel(PanelAction::DuplicateRight));
        menu.rows.push(MenuRow::Panel(PanelAction::DuplicateBelow));
        menu.rows.push(MenuRow::Close);
        menu
    }

    /// The Replace with list of Panel `panel`'s popover: one row for each of `names` (the displays to choose from),
    /// then Back.
    pub fn replace_list(panel: usize, names: &[String]) -> Menu {
        let mut rows = vec![MenuRow::Heading("Replace with".to_string())];
        rows.extend(
            names
                .iter()
                .map(|n| MenuRow::Panel(PanelAction::ReplaceBy(n.clone()))),
        );
        rows.push(MenuRow::Panel(PanelAction::Back));
        Menu::with_rows(panel, rows)
    }

    /// Show `current` again: every slider and choice reads the settings, so a value that changed (a drag, a
    /// typed number, a reset) is what the popover shows.
    pub fn sync(&mut self, current: &Settings) {
        for row in &mut self.rows {
            match row {
                MenuRow::Slider {
                    key, amount, value, ..
                } => *value = amount.read(current.get(*key).map(String::as_str)),
                MenuRow::Choice { key, value, on, .. } => {
                    *on = current.get(*key).map(String::as_str) == Some(*value)
                }
                _ => {}
            }
        }
    }

    // ---- Geometry ----

    /// A row's height in logical pixels, a group heading's room above it included.
    fn row_height(&self, row: usize) -> f32 {
        match &self.rows[row] {
            MenuRow::Slider { .. } => crate::style::SPACE_SLIDER_HEIGHT,
            MenuRow::Heading(_) => MENU_ROW + self.heading_gap(row),
            _ => MENU_ROW,
        }
    }

    /// The room above a heading that separates its group from the one before: none for the first row, more
    /// before the Panel section.
    fn heading_gap(&self, row: usize) -> f32 {
        match &self.rows[row] {
            MenuRow::Heading(text) if row > 0 => {
                if text == PANEL_SECTION {
                    2.0 * crate::style::SPACE_POPOVER_GROUP_GAP
                } else {
                    crate::style::SPACE_POPOVER_GROUP_GAP
                }
            }
            _ => 0.0,
        }
    }

    fn content_height(&self) -> f32 {
        (0..self.rows.len()).map(|r| self.row_height(r)).sum()
    }

    /// Where the menu sits when opened at `at` in a window of `window` device pixels at `scale`: the whole menu
    /// stays inside the window. Left, top, width, height.
    pub fn rect(&self, at: [f32; 2], window: [f32; 2], scale: f32) -> [f32; 4] {
        let (w, h) = (
            MENU_WIDTH * scale,
            (self.content_height() + 2.0 * MENU_PAD) * scale,
        );
        [
            at[0].min(window[0] - w).max(0.0),
            at[1].min(window[1] - h).max(0.0),
            w,
            h,
        ]
    }

    /// The top of row `row` inside `rect`.
    pub fn row_top(&self, rect: [f32; 4], row: usize, scale: f32) -> f32 {
        let above: f32 = (0..row.min(self.rows.len()))
            .map(|r| self.row_height(r))
            .sum();
        rect[1] + (MENU_PAD + above) * scale
    }

    /// Row `row`'s rectangle inside `rect`: the whole width.
    pub fn row_rect(&self, rect: [f32; 4], row: usize, scale: f32) -> [f32; 4] {
        [
            rect[0],
            self.row_top(rect, row, scale),
            rect[2],
            self.row_height(row) * scale,
        ]
    }

    /// A heading's own row, below the room its group's gap takes.
    pub fn heading_rect(&self, rect: [f32; 4], row: usize, scale: f32) -> [f32; 4] {
        let gap = self.heading_gap(row) * scale;
        let full = self.row_rect(rect, row, scale);
        [full[0], full[1] + gap, full[2], full[3] - gap]
    }

    /// The row under `point`'s y, when `point` is inside the menu's width.
    pub fn row_at(&self, rect: [f32; 4], point: [f32; 2], scale: f32) -> Option<usize> {
        if point[0] < rect[0] || point[0] >= rect[0] + rect[2] || point[1] < rect[1] {
            return None;
        }
        let mut top = rect[1] + MENU_PAD * scale;
        if point[1] < top {
            return None;
        }
        for row in 0..self.rows.len() {
            top += self.row_height(row) * scale;
            if point[1] < top {
                return Some(row);
            }
        }
        None
    }

    /// The Panel section's entry under `point`, if it is on one.
    pub fn panel_action_at(
        &self,
        rect: [f32; 4],
        point: [f32; 2],
        scale: f32,
    ) -> Option<PanelAction> {
        match self.rows.get(self.row_at(rect, point, scale)?)? {
            MenuRow::Panel(action) => Some(action.clone()),
            _ => None,
        }
    }

    /// Whether `point` is on the Close row.
    pub fn close_at(&self, rect: [f32; 4], point: [f32; 2], scale: f32) -> bool {
        self.row_at(rect, point, scale)
            .is_some_and(|row| self.rows.get(row) == Some(&MenuRow::Close))
    }

    /// The choice (setting key and value) under `point`, if the point is on a choice row.
    pub fn choice_at(
        &self,
        rect: [f32; 4],
        point: [f32; 2],
        scale: f32,
    ) -> Option<(&'static str, &'static str)> {
        match self.rows.get(self.row_at(rect, point, scale)?)? {
            MenuRow::Choice { key, value, .. } => Some((key, value)),
            _ => None,
        }
    }

    /// Where an amount row's slider begins (after its label), for [`crate::components::Slider`] to place its parts.
    pub fn slider_origin(&self, rect: [f32; 4], row: usize, scale: f32) -> [f32; 2] {
        [
            rect[0]
                + (crate::style::SPACE_MENU_HEADING_LEFT + crate::style::SPACE_POPOVER_LABEL)
                    * scale,
            self.row_top(rect, row, scale),
        ]
    }

    /// The label's box in an amount row.
    pub fn label_rect(&self, rect: [f32; 4], row: usize, scale: f32) -> [f32; 4] {
        let r = self.row_rect(rect, row, scale);
        [
            rect[0] + crate::style::SPACE_MENU_HEADING_LEFT * scale,
            r[1],
            crate::style::SPACE_POPOVER_LABEL * scale,
            r[3],
        ]
    }

    /// The parts of amount row `row`.
    pub fn slider_layout(
        &self,
        rect: [f32; 4],
        row: usize,
        scale: f32,
    ) -> crate::components::SliderLayout {
        crate::components::Slider {
            value: 0.0,
            number: String::new(),
        }
        .layout(self.slider_origin(rect, row, scale), scale)
    }

    /// Where the Panel-wide reset icon is: the corner of the Panel section's heading. `None` when the menu has no
    /// Panel section.
    pub fn panel_reset_rect(&self, rect: [f32; 4], scale: f32) -> Option<[f32; 4]> {
        let row = self
            .rows
            .iter()
            .position(|r| matches!(r, MenuRow::Heading(t) if t == PANEL_SECTION))?;
        let h = self.heading_rect(rect, row, scale);
        let side = crate::style::SPACE_RESET_ICON * scale;
        Some([
            rect[0] + rect[2] - crate::style::SPACE_MENU_HEADING_LEFT * scale - side,
            h[1] + (h[3] - side) / 2.0,
            side,
            side,
        ])
    }

    /// Whether `point` is on the Panel reset icon.
    pub fn panel_reset_at(&self, rect: [f32; 4], point: [f32; 2], scale: f32) -> bool {
        self.panel_reset_rect(rect, scale)
            .is_some_and(|r| crate::components::contains(r, point))
    }

    /// The part of an amount row under `point`, with its setting.
    pub fn amount_hit(
        &self,
        rect: [f32; 4],
        point: [f32; 2],
        scale: f32,
    ) -> Option<(&'static str, AmountHit)> {
        let row = self.row_at(rect, point, scale)?;
        let MenuRow::Slider { key, .. } = self.rows.get(row)? else {
            return None;
        };
        let at = self.slider_origin(rect, row, scale);
        let slider = crate::components::Slider {
            value: 0.0,
            number: String::new(),
        };
        let (l, grab) = (slider.layout(at, scale), slider.grab(at, scale));
        let on = |r: [f32; 4]| crate::components::contains(r, point);
        let part = if on(l.reset) {
            AmountHit::Reset
        } else if on(l.up) {
            AmountHit::Up
        } else if on(l.down) {
            AmountHit::Down
        } else if on(l.number) {
            AmountHit::Field
        } else if on(grab) {
            AmountHit::Track
        } else {
            return None;
        };
        Some((key, part))
    }

    /// The setting and value a press at `point` means when it is on a slider's track or thumb.
    pub fn slider_at(
        &self,
        rect: [f32; 4],
        point: [f32; 2],
        scale: f32,
    ) -> Option<(&'static str, f32)> {
        match self.amount_hit(rect, point, scale)? {
            (key, AmountHit::Track) => self
                .slider_value(rect, key, point[0], scale)
                .map(|v| (key, v)),
            _ => None,
        }
    }

    /// The value of slider `key` with the pointer at `x`, wherever it is: a drag goes on past the ends of the
    /// track and out of the menu.
    pub fn slider_value(&self, rect: [f32; 4], key: &str, x: f32, scale: f32) -> Option<f32> {
        let (row, amount, _) = self.amount_row(key)?;
        let at = self.slider_origin(rect, row, scale);
        Some(
            amount.at_fraction(
                crate::components::Slider {
                    value: 0.0,
                    number: String::new(),
                }
                .value_at(at, x, scale),
            ),
        )
    }

    /// The setting and default value when `point` is on an amount's reset icon and it is not at its default.
    pub fn reset_at(
        &self,
        rect: [f32; 4],
        point: [f32; 2],
        scale: f32,
    ) -> Option<(&'static str, f32)> {
        let (key, AmountHit::Reset) = self.amount_hit(rect, point, scale)? else {
            return None;
        };
        let (_, amount, value) = self.amount_row(key)?;
        (value != amount.default).then_some((key, amount.default))
    }

    /// Whether `point` is inside the menu.
    pub fn contains(rect: [f32; 4], point: [f32; 2]) -> bool {
        point[0] >= rect[0]
            && point[0] < rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] < rect[1] + rect[3]
    }

    // ---- Amounts: dragging, the value box, the reset icon ----

    fn amount_row(&self, key: &str) -> Option<(usize, Amount, f32)> {
        self.rows.iter().enumerate().find_map(|(i, r)| match r {
            MenuRow::Slider {
                key: k,
                amount,
                value,
                ..
            } if *k == key => Some((i, *amount, *value)),
            _ => None,
        })
    }

    /// The value the popover shows for amount `key`.
    pub fn value_of(&self, key: &str) -> Option<f32> {
        self.amount_row(key).map(|(_, _, v)| v)
    }

    /// Show `value` (clamped to the range) for amount `key` and say what changed. Nothing is said when it is
    /// what the row already shows.
    fn set_row(&mut self, key: &'static str, value: f32) -> Option<Change> {
        let (row, amount, old) = self.amount_row(key)?;
        let value = ((value.clamp(amount.min, amount.max) * 1e4).round() / 1e4)
            .clamp(amount.min, amount.max);
        if value == old {
            return None;
        }
        if let MenuRow::Slider { value: v, .. } = &mut self.rows[row] {
            *v = value;
        }
        Some(Change { key, value })
    }

    /// A press at `point` (`big`: Shift held). On the track it grabs the slider and moves the value to the
    /// pointer; on an arrow it steps (Shift: ten steps); on the number it starts typing; on a reset icon it goes
    /// back to the default. A press anywhere else while typing accepts the typed number.
    pub fn press(&mut self, rect: [f32; 4], point: [f32; 2], scale: f32, big: bool) -> Pressed {
        let mut out = Pressed::default();
        let hit = self.amount_hit(rect, point, scale);
        if let Some(edit) = &self.edit
            && !matches!(hit, Some((k, AmountHit::Field)) if k == edit.key)
        {
            out.changes.extend(self.finish_edit(true));
        }
        match hit {
            Some((key, AmountHit::Track)) => {
                self.drag = Some(key);
                if let Some(v) = self.slider_value(rect, key, point[0], scale) {
                    out.changes.extend(self.set_row(key, v));
                }
                out.consumed = true;
            }
            Some((key, AmountHit::Up)) => {
                out.changes.extend(self.step(key, 1.0, big));
                out.consumed = true;
            }
            Some((key, AmountHit::Down)) => {
                out.changes.extend(self.step(key, -1.0, big));
                out.consumed = true;
            }
            Some((key, AmountHit::Field)) => {
                if self.edit.is_none()
                    && let Some((_, amount, value)) = self.amount_row(key)
                {
                    self.edit = Some(Edit {
                        key,
                        text: format!("{value:.*}", amount.decimals),
                        original: value,
                        fresh: true,
                    });
                }
                out.consumed = true;
            }
            Some((key, AmountHit::Reset)) => {
                if let Some((_, amount, _)) = self.amount_row(key) {
                    out.changes.extend(self.set_row(key, amount.default));
                }
                out.consumed = true;
            }
            None => {
                if self.panel_reset_at(rect, point, scale) {
                    out.reset_panel = !self.panel_at_default;
                    out.consumed = true;
                }
            }
        }
        out
    }

    /// One step of `key` up (`direction` 1) or down; `big` is ten steps.
    fn step(&mut self, key: &'static str, direction: f32, big: bool) -> Option<Change> {
        let (_, amount, value) = self.amount_row(key)?;
        // While typing, the step goes from the number typed.
        let base = match &self.edit {
            Some(e) if e.key == key => e.text.parse::<f32>().unwrap_or(value),
            _ => value,
        };
        let change = self.set_row(
            key,
            base + direction * amount.step * if big { 10.0 } else { 1.0 },
        );
        let shown = self.value_of(key).unwrap_or(value);
        if let Some(e) = self.edit.as_mut().filter(|e| e.key == key) {
            e.text = format!("{shown:.*}", amount.decimals);
            e.fresh = true;
        }
        change
    }

    /// The drag in progress with the pointer at `x`: the amount's new value, if it moved.
    pub fn drag_to(&mut self, rect: [f32; 4], x: f32, scale: f32) -> Option<Change> {
        let key = self.drag?;
        let v = self.slider_value(rect, key, x, scale)?;
        self.set_row(key, v)
    }

    /// Whether a slider is being dragged.
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The pointer was let go.
    pub fn release(&mut self) {
        self.drag = None;
    }

    /// The setting and text of the value box being typed into.
    pub fn editing(&self) -> Option<(&'static str, &str)> {
        self.edit.as_ref().map(|e| (e.key, e.text.as_str()))
    }

    /// A key for the value box being typed into (nothing happens when none is).
    pub fn key(&mut self, key: EditKey) -> Option<Change> {
        let target = self.edit.as_ref()?.key;
        match key {
            EditKey::Char(c) => {
                let edit = self.edit.as_mut()?;
                let c = if c == ',' { '.' } else { c };
                if c.is_ascii_digit() || c == '.' || c == '-' {
                    if edit.fresh {
                        edit.text.clear();
                        edit.fresh = false;
                    }
                    if edit.text.len() < 12 {
                        edit.text.push(c);
                    }
                }
                None
            }
            EditKey::Backspace => {
                let edit = self.edit.as_mut()?;
                if edit.fresh {
                    edit.text.clear();
                    edit.fresh = false;
                } else {
                    edit.text.pop();
                }
                None
            }
            EditKey::Up(big) => self.step(target, 1.0, big),
            EditKey::Down(big) => self.step(target, -1.0, big),
            EditKey::Enter => self.finish_edit(true),
            EditKey::Escape => self.finish_edit(false),
        }
    }

    /// End typing: accepting sets the typed number (clamped; unreadable text keeps what was there), cancelling
    /// puts back the number from before typing began.
    fn finish_edit(&mut self, accept: bool) -> Option<Change> {
        let edit = self.edit.take()?;
        let typed = accept
            .then(|| edit.text.parse::<f32>().ok().filter(|v| v.is_finite()))
            .flatten();
        self.set_row(edit.key, typed.unwrap_or(edit.original))
    }
}

/// Write `text` to `path` so that a reader (or a crash) never sees half of it: it goes to a temporary file next to
/// `path` and is renamed over it. The folder is made if it is missing.
pub fn save(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    let temporary = path.with_file_name(format!(".{name}.tmp"));
    let written = std::fs::write(&temporary, text).and_then(|()| std::fs::rename(&temporary, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}
