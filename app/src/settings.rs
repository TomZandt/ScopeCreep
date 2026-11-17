//! Panel settings and the right-click menu that edits them (spec #49, "Settings"). Pure: no GPU, no window.
//!
//! A display lists the settings it offers as [`SettingDef`]s, each a short list of choices (the first is the
//! default), so only sensible values can be picked. The user's choices are kept per Panel in [`PanelSettings`],
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

/// A setting a display offers. The first choice is the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingDef {
    pub key: &'static str,
    /// The menu's heading for the setting.
    pub label: &'static str,
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
        self.panels.entry(panel.to_string()).or_default().insert(key.to_string(), value.to_string());
    }

    /// Remember a value a display keeps for itself (not a menu choice).
    pub fn set_state(&mut self, panel: &str, key: &str, value: &str) {
        self.state.entry(panel.to_string()).or_default().insert(key.to_string(), value.to_string());
    }

    /// `panel`'s remembered values for `keys`, as a display reads them; those never remembered are left out.
    pub fn state_of(&self, panel: &str, keys: &[&str]) -> Settings {
        let Some(own) = self.state.get(panel) else { return Settings::new() };
        keys.iter().filter_map(|k| own.get(*k).map(|v| (k.to_string(), v.clone()))).collect()
    }

    /// `panel`'s settings for the settings it offers (`defs`): the user's choice, or the default when there is
    /// none or it is no longer a choice (a remembered value from another version never reaches a display).
    pub fn resolve(&self, panel: &str, defs: &[SettingDef]) -> Settings {
        let chosen = self.panels.get(panel);
        defs.iter()
            .filter_map(|def| {
                let wanted = chosen.and_then(|c| c.get(def.key));
                let choice = def.choices.iter().find(|c| Some(c.value) == wanted.map(String::as_str)).or(def.choices.first())?;
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
        for line in text.lines() {
            let mut parts = line.split('\t');
            if let (Some(panel), Some(key), Some(value), None) = (parts.next(), parts.next(), parts.next(), parts.next()) {
                if !panel.is_empty() && !key.is_empty() && !value.is_empty() {
                    match key.strip_prefix(STATE_PREFIX) {
                        Some(key) => settings.set_state(panel, key, value),
                        None => settings.set(panel, key, value),
                    }
                }
            }
        }
        settings
    }
}

/// Logical pixels: the menu's size.
pub const MENU_WIDTH: f32 = 230.0;
pub const MENU_ROW: f32 = 24.0;
pub const MENU_PAD: f32 = 6.0;

/// What one row of the menu says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuRow {
    /// A setting's name; not clickable.
    Heading(String),
    /// A choice; `on` when it is the setting's current value.
    Choice { key: &'static str, value: &'static str, text: String, on: bool },
}

/// The right-click menu of one Panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    /// The Panel (its index in the grid) the menu belongs to.
    pub panel: usize,
    pub rows: Vec<MenuRow>,
}

impl Menu {
    /// The menu for a display with `defs`, showing `current`. `None` when it has no settings: such a Panel shows
    /// no menu.
    pub fn new(panel: usize, defs: &[SettingDef], current: &Settings) -> Option<Menu> {
        if defs.is_empty() {
            return None;
        }
        let mut rows = Vec::new();
        for def in defs {
            rows.push(MenuRow::Heading(def.label.to_string()));
            for choice in def.choices {
                rows.push(MenuRow::Choice { key: def.key, value: choice.value, text: choice.text.to_string(), on: current.get(def.key).map(String::as_str) == Some(choice.value) });
            }
        }
        Some(Menu { panel, rows })
    }

    /// Where the menu sits when opened at `at` in a window of `window` device pixels at `scale`: the whole menu
    /// stays inside the window. Left, top, width, height.
    pub fn rect(&self, at: [f32; 2], window: [f32; 2], scale: f32) -> [f32; 4] {
        let (w, h) = (MENU_WIDTH * scale, (self.rows.len() as f32 * MENU_ROW + 2.0 * MENU_PAD) * scale);
        [at[0].min(window[0] - w).max(0.0), at[1].min(window[1] - h).max(0.0), w, h]
    }

    /// The top of row `row` inside `rect`.
    pub fn row_top(rect: [f32; 4], row: usize, scale: f32) -> f32 {
        rect[1] + (MENU_PAD + row as f32 * MENU_ROW) * scale
    }

    /// The choice (setting key and value) under `point`, if the point is on a choice row.
    pub fn choice_at(&self, rect: [f32; 4], point: [f32; 2], scale: f32) -> Option<(&'static str, &'static str)> {
        if point[0] < rect[0] || point[0] >= rect[0] + rect[2] {
            return None;
        }
        let offset = point[1] - rect[1] - MENU_PAD * scale;
        if offset < 0.0 {
            return None;
        }
        match self.rows.get((offset / (MENU_ROW * scale)) as usize)? {
            MenuRow::Choice { key, value, .. } => Some((key, value)),
            MenuRow::Heading(_) => None,
        }
    }

    /// Whether `point` is inside the menu.
    pub fn contains(rect: [f32; 4], point: [f32; 2]) -> bool {
        point[0] >= rect[0] && point[0] < rect[0] + rect[2] && point[1] >= rect[1] && point[1] < rect[1] + rect[3]
    }
}
