//! The Panel toolbar (spec #113, #116): the settings a display marks to show on its title bar as chips, instead of
//! in the popover. Pure: no GPU, no window.
//!
//! A display lists its title-bar settings as [`ToolbarDef`]s. Each is one of its ordinary settings (ADR 0013, the
//! choices are stored and remembered as ever), shown as a row of chips, one per choice, one at a time. The
//! popover holds every other setting ([`popover_defs`]).
use crate::components::{Channel, Chip, ChipMode};
use crate::settings::{SettingDef, Settings};

/// One chip: the setting's choice it selects, the letter on it, what hovering it says, and the colour it takes
/// when on (neutral accent when `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolbarChip {
    pub value: &'static str,
    pub text: &'static str,
    pub tooltip: &'static str,
    pub channel: Option<Channel>,
}

/// A setting shown as chips on the title bar. Only single choice is built so far (the Channel view); the other
/// modes of [`ChipMode`] arrive with the displays that use them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolbarDef {
    /// The setting's key, one of the display's [`SettingDef`]s.
    pub key: &'static str,
    pub mode: ChipMode,
    pub chips: &'static [ToolbarChip],
}

/// A Panel's toolbar as the title bar draws it, under the Panel's current settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Toolbar {
    pub chips: Vec<Chip>,
    pub on: Vec<bool>,
    pub tooltips: Vec<String>,
    /// Where a click on chip `i` leads: the setting's key and the choice it picks.
    pub picks: Vec<(&'static str, &'static str)>,
}

impl Toolbar {
    /// The toolbar of `defs` showing `current`. A setting with no chip matching its value shows none on.
    pub fn new(defs: &[ToolbarDef], current: &Settings) -> Toolbar {
        let mut toolbar = Toolbar::default();
        for def in defs {
            for chip in def.chips {
                toolbar.chips.push(match chip.channel {
                    Some(channel) => Chip::channel(chip.text, channel),
                    None => Chip::new(chip.text),
                });
                toolbar.on.push(current.get(def.key).map(String::as_str) == Some(chip.value));
                toolbar.tooltips.push(chip.tooltip.to_string());
                toolbar.picks.push((def.key, chip.value));
            }
        }
        toolbar
    }

    /// What clicking chip `index` sets: the setting and its new value. Clicking the chip already on keeps it on
    /// (a single choice is never empty).
    pub fn click(&self, index: usize) -> Option<(&'static str, &'static str)> {
        self.picks.get(index).copied()
    }
}

/// The settings left for the popover: those the toolbar does not show.
pub fn popover_defs(defs: &[SettingDef], toolbar: &[ToolbarDef]) -> Vec<SettingDef> {
    defs.iter().filter(|d| !toolbar.iter().any(|t| t.key == d.key)).copied().collect()
}
