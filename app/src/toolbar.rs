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

/// A setting shown as chips on the title bar. Single choice (the Channel view) stores the chip's `value`; any
/// combination (the luma waveform) stores the `value`s of the chips that are on, in chip order ("yrb"). The other
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
    /// Where a click on chip `i` leads: the setting's key and the value it sets.
    pub picks: Vec<(&'static str, String)>,
}

impl Toolbar {
    /// The toolbar of `defs` showing `current`. A setting with no chip matching its value shows none on.
    pub fn new(defs: &[ToolbarDef], current: &Settings) -> Toolbar {
        let mut toolbar = Toolbar::default();
        for def in defs {
            let stored = current.get(def.key).map(String::as_str);
            let first = toolbar.on.len();
            for chip in def.chips {
                toolbar.chips.push(match chip.channel {
                    Some(channel) => Chip::channel(chip.text, channel),
                    None => Chip::new(chip.text),
                });
                toolbar.on.push(match def.mode {
                    ChipMode::Multiple => stored.is_some_and(|v| v.contains(chip.value)),
                    _ => stored == Some(chip.value),
                });
                toolbar.tooltips.push(chip.tooltip.to_string());
                toolbar.picks.push((def.key, chip.value.to_string()));
            }
            if def.mode == ChipMode::Multiple {
                // A click on chip `i` sets the letters of the chips that would then be on.
                let on = toolbar.on[first..].to_vec();
                for i in 0..def.chips.len() {
                    let next = crate::components::clicked(def.mode, &on, i);
                    let value: String = def.chips.iter().zip(&next).filter(|(_, on)| **on).map(|(c, _)| c.value).collect();
                    toolbar.picks[first + i].1 = value;
                }
            }
        }
        toolbar
    }

    /// What clicking chip `index` sets: the setting and its new value. Clicking the chip already on keeps it on
    /// (a single choice is never empty), and the last chip on of a combination stays on.
    pub fn click(&self, index: usize) -> Option<(&'static str, &str)> {
        self.picks.get(index).map(|(key, value)| (*key, value.as_str()))
    }
}

/// The settings left for the popover: those the toolbar does not show.
pub fn popover_defs(defs: &[SettingDef], toolbar: &[ToolbarDef]) -> Vec<SettingDef> {
    defs.iter().filter(|d| !toolbar.iter().any(|t| t.key == d.key)).copied().collect()
}
