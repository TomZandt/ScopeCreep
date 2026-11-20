//! The Panel toolbar (spec #113, #116): the settings a display marks to show on its title bar as chips, instead of
//! in the popover. Pure: no GPU, no window.
//!
//! A display lists its title-bar settings as [`ToolbarDef`]s. Each is one of its ordinary settings (ADR 0013, the
//! choices are stored and remembered as ever), shown as a row of chips, one per choice, one at a time. The
//! popover holds every other setting ([`popover_defs`]).
use crate::components::{Channel, Chip, ChipMode};
use crate::settings::{Choice, SettingDef, Settings};

/// The value a toggle setting has when off; its chip's `value` is what it has when on.
pub const TOGGLE_OFF: &str = "off";

/// The B/W toggle (#118): the setting and its chip, for the Panels that draw their unpainted picture in Luma grey
/// or in colour. The setting's first choice is its default, so each Panel picks the one that suits it.
pub const BW_SETTING: &str = "bw";
pub const BW_DEFAULT_COLOUR: SettingDef = SettingDef {
    key: BW_SETTING,
    label: "Black and white",
    amount: None,
    choices: &[
        Choice {
            value: TOGGLE_OFF,
            text: "Colour",
        },
        Choice {
            value: "on",
            text: "Black and white",
        },
    ],
};
pub const BW_DEFAULT_GREY: SettingDef = SettingDef {
    key: BW_SETTING,
    label: "Black and white",
    amount: None,
    choices: &[
        Choice {
            value: "on",
            text: "Black and white",
        },
        Choice {
            value: TOGGLE_OFF,
            text: "Colour",
        },
    ],
};
pub static BW_TOOLBAR: [ToolbarDef; 1] = [ToolbarDef {
    key: BW_SETTING,
    mode: ChipMode::Toggle,
    chips: &[ToolbarChip {
        value: "on",
        text: "B/W",
        tooltip: "Unpainted picture in black and white (Luma grey)",
        channel: None,
    }],
}];

/// The clean picture's channel (#160): colour (off), or one of Y R G B as grey, one or none lit. Colour is the
/// first choice, so the default.
pub const CHANNEL_SETTING: &str = "channel";
pub const CHANNEL_DEF: SettingDef = SettingDef {
    key: CHANNEL_SETTING,
    label: "Channel",
    amount: None,
    choices: &[
        Choice {
            value: TOGGLE_OFF,
            text: "Colour",
        },
        Choice {
            value: "luma",
            text: "Luma (Y)",
        },
        Choice {
            value: "red",
            text: "Red",
        },
        Choice {
            value: "green",
            text: "Green",
        },
        Choice {
            value: "blue",
            text: "Blue",
        },
    ],
};

/// One channel the clean picture can show as grey: the single table the chips, the shader and the title tag read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelChoice {
    /// The stored setting value.
    pub value: &'static str,
    /// The letter on its chip and its chip colour.
    pub letter: &'static str,
    pub colour: Channel,
    pub tooltip: &'static str,
    /// The name in the Panel's title tag.
    pub tag: &'static str,
    /// What the colour rule's `channel` reads (0 is colour, so these start at 1).
    pub shader: f32,
}

/// The channels in chip order.
pub const CHANNELS: [ChannelChoice; 4] = [
    ChannelChoice {
        value: "luma",
        letter: "Y",
        colour: Channel::Y,
        tooltip: "Luma (Y)",
        tag: "Luma",
        shader: 1.0,
    },
    ChannelChoice {
        value: "red",
        letter: "R",
        colour: Channel::R,
        tooltip: "Red",
        tag: "Red",
        shader: 2.0,
    },
    ChannelChoice {
        value: "green",
        letter: "G",
        colour: Channel::G,
        tooltip: "Green",
        tag: "Green",
        shader: 3.0,
    },
    ChannelChoice {
        value: "blue",
        letter: "B",
        colour: Channel::B,
        tooltip: "Blue",
        tag: "Blue",
        shader: 4.0,
    },
];

/// The channel `value` (a stored setting) names; `None` for colour or anything else.
pub fn channel_of(value: Option<&str>) -> Option<&'static ChannelChoice> {
    CHANNELS.iter().find(|c| Some(c.value) == value)
}

const fn channel_chip(i: usize) -> ToolbarChip {
    let c = &CHANNELS[i];
    ToolbarChip {
        value: c.value,
        text: c.letter,
        tooltip: c.tooltip,
        channel: Some(c.colour),
    }
}

pub static CHANNEL_TOOLBAR: [ToolbarDef; 1] = [ToolbarDef {
    key: CHANNEL_SETTING,
    mode: ChipMode::OneOrNone,
    chips: &[
        channel_chip(0),
        channel_chip(1),
        channel_chip(2),
        channel_chip(3),
    ],
}];

/// The clean picture's Loupe toggle (#161): off by default; its size is the popover's `size` choice.
pub const LOUPE_SETTING: &str = "loupe";
pub const LOUPE_DEF: SettingDef = SettingDef {
    key: LOUPE_SETTING,
    label: "Loupe",
    amount: None,
    choices: &[
        Choice {
            value: TOGGLE_OFF,
            text: "Off",
        },
        Choice {
            value: "on",
            text: "On",
        },
    ],
};
pub static LOUPE_TOOLBAR: [ToolbarDef; 1] = [ToolbarDef {
    key: LOUPE_SETTING,
    mode: ChipMode::Toggle,
    chips: &[ToolbarChip {
        value: "on",
        text: "L",
        tooltip: "Loupe: drag it, scroll over it to zoom",
        channel: None,
    }],
}];

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
/// combination (the luma waveform) stores the `value`s of the chips that are on, in chip order ("yrb"); a toggle (B/W)
/// stores its chip's `value` when on and [`TOGGLE_OFF`] when off. The other
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
    /// What a click on chip `i` does.
    pub picks: Vec<ChipAction>,
}

/// What a click on a title-bar chip does.
#[derive(Clone, Debug, PartialEq)]
pub enum ChipAction {
    /// Set a Panel setting: its key and the value it takes.
    Set(&'static str, String),
    /// Ask for the zoom popover. `short` is what the chip says when the bar has no room for the word Fit.
    Zoom { short: String },
    /// Switch the Window's Link to this state.
    Link(bool),
}

/// The tip on a picture Panel's zoom chip.
pub const ZOOM_TOOLTIP: &str = "Zoom: scroll, or click for Fit";

/// The link chip's text and tip.
pub const LINK_CHIP: &str = "Link";
pub const LINK_TOOLTIP: &str = "Link zoom across Panels";

impl Toolbar {
    /// The toolbar with the zoom chip first when the Panel zooms (`readout` is `Some`): lit while zoomed in, and a
    /// click on it asks for the zoom popover ([`ChipAction::Zoom`]).
    pub fn with_zoom(mut self, readout: Option<&crate::zoom::Readout>) -> Toolbar {
        let Some(r) = readout else { return self };
        self.chips.insert(0, Chip::new(&r.text));
        self.on.insert(0, r.percent > r.fit + 0.01);
        self.tooltips.insert(0, ZOOM_TOOLTIP.to_string());
        self.picks.insert(
            0,
            ChipAction::Zoom {
                short: r.short.clone(),
            },
        );
        self
    }

    /// The toolbar with the link chip beside the zoom chip (second) when the Panel zooms: lit on every picture Panel
    /// while Link is on, and a click on it asks to switch it ([`ChipAction::Link`], to the state it would
    /// then have).
    pub fn with_link(mut self, on: bool) -> Toolbar {
        if !matches!(self.picks.first(), Some(ChipAction::Zoom { .. })) {
            return self;
        }
        self.chips.insert(1, Chip::new(LINK_CHIP));
        self.on.insert(1, on);
        self.tooltips.insert(1, LINK_TOOLTIP.to_string());
        self.picks.insert(1, ChipAction::Link(!on));
        self
    }

    /// On a title bar `bar_width` wide (device pixels), shorten the zoom chip to its number ("23 %" for "Fit 23 %")
    /// when the chips and the settings button would not fit with it long. It stays first, so it stays on the bar.
    pub fn fit_to(&mut self, bar_width: f32, settings_button: bool, scale: f32) {
        let Some(ChipAction::Zoom { short }) = self.picks.first() else {
            return;
        };
        let button = if settings_button {
            (crate::style::SPACE_SETTINGS_BUTTON + crate::style::SPACE_CHIP_GAP) * scale
        } else {
            0.0
        };
        let needed = crate::components::chip_row_width(&self.chips, scale)
            + button
            + 2.0 * crate::style::SPACE_BAR_PAD * scale;
        if needed > bar_width {
            self.chips[0].text = short.clone();
        }
    }

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
                toolbar
                    .picks
                    .push(ChipAction::Set(def.key, chip.value.to_string()));
            }
            if matches!(def.mode, ChipMode::Toggle | ChipMode::OneOrNone) {
                // A click on a toggle (or a one-or-none chip) that is on turns it off.
                for i in first..toolbar.on.len() {
                    if toolbar.on[i] {
                        toolbar.picks[i] = ChipAction::Set(def.key, TOGGLE_OFF.to_string());
                    }
                }
            }
            if def.mode == ChipMode::Multiple {
                // A click on chip `i` sets the letters of the chips that would then be on.
                let on = toolbar.on[first..].to_vec();
                for i in 0..def.chips.len() {
                    let next = crate::components::clicked(def.mode, &on, i);
                    let value: String = def
                        .chips
                        .iter()
                        .zip(&next)
                        .filter(|(_, on)| **on)
                        .map(|(c, _)| c.value)
                        .collect();
                    toolbar.picks[first + i] = ChipAction::Set(def.key, value);
                }
            }
        }
        toolbar
    }

    /// What clicking chip `index` sets: the setting and its new value. Clicking the chip already on keeps it on
    /// (a single choice is never empty), and the last chip on of a combination stays on.
    pub fn click(&self, index: usize) -> Option<(&'static str, &str)> {
        self.picks.get(index).map(|action| match action {
            ChipAction::Set(key, value) => (*key, value.as_str()),
            ChipAction::Zoom { .. } => (crate::zoom::ZOOM_KEY, ""),
            ChipAction::Link(on) => (crate::zoom::LINK_KEY, if *on { "on" } else { TOGGLE_OFF }),
        })
    }

    /// What clicking chip `index` does, for the Window to act on.
    pub fn action(&self, index: usize) -> Option<&ChipAction> {
        self.picks.get(index)
    }
}

/// The settings left for the popover: those the toolbar does not show.
pub fn popover_defs(defs: &[SettingDef], toolbar: &[ToolbarDef]) -> Vec<SettingDef> {
    defs.iter()
        .filter(|d| !toolbar.iter().any(|t| t.key == d.key))
        .copied()
        .collect()
}
