//! The Hotkeys panel (#148, spec #142, ADR 0036): every command and its hotkey, where the colourist presses the
//! combination they want instead of typing a key name. Pure: no GPU, no window. It is drawn as a menu dropped from
//! the Settings title ([`crate::components::DropMenu`]), so it is hit-tested and lit like the other menus.
//!
//! Click a command and its row becomes the key-capture field ("Press a key"): the next combination is assigned;
//! Escape cancels, Backspace or Delete clears. A combination another command has names that command and offers
//! Replace or Cancel; one the app keeps for itself is refused with a line saying so, and capture goes on. A
//! command can be reset to its default while it is being captured, and every command at once from the foot of
//! the panel.
use crate::commands::{Command, Hotkey, Hotkeys, Refused};
use crate::components::{MenuEntry, MenuItem};
use crate::menu_bar::{Menu, MenuAction};

/// What a click in the panel does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelAction {
    /// Start capturing a hotkey for the command (a click on the one being captured stops).
    Capture(Command),
    /// Put the command being captured back to its default.
    Reset(Command),
    ResetAll,
    /// Give the command the conflicting hotkey, taking it from the other command.
    Replace,
    /// Leave every hotkey as it was.
    Cancel,
}

/// What the panel is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// Listing the commands.
    #[default]
    Idle,
    /// Waiting for a key combination for the command. `refused` when the last one pressed is kept by the app.
    Capturing { command: Command, refused: bool },
    /// The combination pressed for `command` belongs to `other`: replace or cancel.
    Conflict {
        command: Command,
        hotkey: Hotkey,
        other: Command,
    },
}

/// The Hotkeys panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotkeysPanel {
    pub state: State,
}

/// What the key-capture field says while it waits.
pub const PRESS_A_KEY: &str = "Press a key";

impl HotkeysPanel {
    /// Whether the panel wants every key (capturing, or asking replace or cancel).
    pub fn takes_keys(&self) -> bool {
        self.state != State::Idle
    }

    /// The panel's rows: a heading, every command with its hotkey (the one being captured says so), then what the
    /// state offers.
    pub fn menu(&self, hotkeys: &Hotkeys) -> Menu {
        let mut menu = Menu::new();
        menu.push(MenuEntry::Heading("Hotkeys".to_string()), None);
        let capturing = match self.state {
            State::Capturing { command, .. } | State::Conflict { command, .. } => Some(command),
            State::Idle => None,
        };
        for command in Command::all() {
            let hotkey = if capturing == Some(command) {
                PRESS_A_KEY.to_string()
            } else {
                hotkeys
                    .get(command)
                    .map_or_else(String::new, |h| h.to_string())
            };
            menu.push(
                MenuEntry::Item(MenuItem {
                    label: command.label(),
                    hotkey,
                    enabled: true,
                    on: capturing == Some(command),
                }),
                Some(MenuAction::Hotkeys(PanelAction::Capture(command))),
            );
        }
        menu.push(MenuEntry::Separator, None);
        let reset_all = |menu: &mut Menu| {
            let mut item = MenuItem::new("Reset all to defaults", "");
            item.enabled = *hotkeys != Hotkeys::default();
            menu.push(
                MenuEntry::Item(item),
                Some(MenuAction::Hotkeys(PanelAction::ResetAll)),
            );
        };
        match self.state {
            State::Idle => reset_all(&mut menu),
            State::Capturing { command, refused } => {
                let line = if refused {
                    "Escape, Enter, Backspace and Delete are kept by the app"
                } else {
                    "Escape cancels, Backspace clears"
                };
                menu.push(MenuEntry::Heading(line.to_string()), None);
                let default = command
                    .default_hotkey()
                    .map_or_else(|| "none".to_string(), |h| h.to_string());
                let mut item = MenuItem::new(&format!("Reset to default ({default})"), "");
                item.enabled = hotkeys.get(command) != command.default_hotkey();
                menu.push(
                    MenuEntry::Item(item),
                    Some(MenuAction::Hotkeys(PanelAction::Reset(command))),
                );
                reset_all(&mut menu);
            }
            State::Conflict { hotkey, other, .. } => {
                menu.push(
                    MenuEntry::Heading(format!("{hotkey} is used by {}", other.label())),
                    None,
                );
                menu.push(
                    MenuEntry::Item(MenuItem::new("Replace", "")),
                    Some(MenuAction::Hotkeys(PanelAction::Replace)),
                );
                menu.push(
                    MenuEntry::Item(MenuItem::new("Cancel", "")),
                    Some(MenuAction::Hotkeys(PanelAction::Cancel)),
                );
            }
        }
        menu
    }

    /// A click on one of the panel's entries. Returns whether the hotkeys changed (and should be remembered).
    pub fn click(&mut self, action: PanelAction, hotkeys: &mut Hotkeys) -> bool {
        let before = hotkeys.clone();
        match action {
            PanelAction::Capture(command) => {
                let same = matches!(
                    self.state,
                    State::Capturing { command: c, .. } | State::Conflict { command: c, .. } if c == command
                );
                self.state = if same {
                    State::Idle
                } else {
                    State::Capturing {
                        command,
                        refused: false,
                    }
                };
            }
            PanelAction::Reset(command) => {
                hotkeys.reset(command);
                self.state = State::Idle;
            }
            PanelAction::ResetAll => {
                hotkeys.reset_all();
                self.state = State::Idle;
            }
            PanelAction::Replace => {
                if let State::Conflict {
                    command, hotkey, ..
                } = self.state
                {
                    // Never reserved: a reserved key never gets as far as a conflict.
                    let _ = hotkeys.replace(command, hotkey);
                }
                self.state = State::Idle;
            }
            PanelAction::Cancel => self.state = State::Idle,
        }
        *hotkeys != before
    }

    /// A key combination pressed while the panel [takes keys](Self::takes_keys) (a modifier alone never gets
    /// here). Returns whether the hotkeys changed.
    pub fn key(&mut self, pressed: Hotkey, hotkeys: &mut Hotkeys) -> bool {
        let plain = !pressed.ctrl && !pressed.shift && !pressed.alt;
        match self.state {
            State::Idle => false,
            State::Conflict { .. } => {
                if pressed.key.name() == "Escape" {
                    self.state = State::Idle;
                }
                false
            }
            State::Capturing { command, .. } => {
                let name = pressed.key.name();
                if name == "Escape" {
                    self.state = State::Idle;
                    return false;
                }
                if plain && matches!(name, "Backspace" | "Delete") {
                    let changed = hotkeys.get(command).is_some();
                    hotkeys.clear(command);
                    self.state = State::Idle;
                    return changed;
                }
                let before = hotkeys.get(command);
                match hotkeys.assign(command, pressed) {
                    Ok(()) => {
                        self.state = State::Idle;
                        before != Some(pressed)
                    }
                    Err(Refused::Reserved) => {
                        self.state = State::Capturing {
                            command,
                            refused: true,
                        };
                        false
                    }
                    Err(Refused::Conflict(other)) => {
                        self.state = State::Conflict {
                            command,
                            hotkey: pressed,
                            other,
                        };
                        false
                    }
                }
            }
        }
    }
}
