//! Named layouts (#98): the nine slots the user saves arrangements of the Panel grid into, recalls and renames.
//! The Layout menu that lists them is in `menu_bar`. Pure: no GPU, no window, no fonts.
//!
//! A slot holds the grid as [`PanelGrid::to_text`] writes it, so recalling one goes through [`PanelGrid::from_text`]:
//! a Panel the layout names that no longer exists is dropped, and a Panel it does not name is placed in the first
//! free cells. Nothing here is on screen until the user opens the menu or presses a hotkey (the hotkeys are commands, in
//! `commands`).
use crate::panel_grid::PanelGrid;

/// How many layouts can be saved.
pub const SLOTS: usize = 9;
/// The longest a layout's name can be, in characters.
pub const NAME_LENGTH: usize = 24;
/// The first line of the saved layouts.
const HEADER: &str = "layouts 1";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Slot {
    name: String,
    grid: String,
}

/// The saved layouts, nine slots, each empty or holding a name and an arrangement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layouts {
    slots: [Option<Slot>; SLOTS],
}

/// A name the user typed, made safe to keep: no control characters, at most [`NAME_LENGTH`] characters, trimmed.
fn clean(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(NAME_LENGTH)
        .collect::<String>()
        .trim()
        .to_string()
}

fn default_name(slot: usize) -> String {
    format!("Layout {}", slot + 1)
}

impl Layouts {
    /// Save `grid` (for the displays called `names`) into `slot`. A slot keeps its name when it has one.
    pub fn save(&mut self, slot: usize, grid: &PanelGrid, names: &[&str]) {
        if slot >= SLOTS {
            return;
        }
        let name = self.slots[slot]
            .as_ref()
            .map_or_else(|| default_name(slot), |s| s.name.clone());
        self.slots[slot] = Some(Slot {
            name,
            grid: grid.to_text(names),
        });
    }

    /// The arrangement saved in `slot` for the displays called `names`; `None` when the slot is empty.
    pub fn recall(&self, slot: usize, names: &[&str]) -> Option<PanelGrid> {
        let saved = self.slots.get(slot)?.as_ref()?;
        Some(PanelGrid::from_text(names, &saved.grid))
    }

    /// The name of the layout in `slot`, if it holds one.
    pub fn name(&self, slot: usize) -> Option<&str> {
        self.slots.get(slot)?.as_ref().map(|s| s.name.as_str())
    }

    /// Rename the layout in `slot`. An empty name, or an empty slot, changes nothing. Returns whether it changed.
    pub fn rename(&mut self, slot: usize, name: &str) -> bool {
        let name = clean(name);
        match self.slots.get_mut(slot).and_then(Option::as_mut) {
            Some(saved) if !name.is_empty() && saved.name != name => {
                saved.name = name;
                true
            }
            _ => false,
        }
    }

    /// The layouts, to remember between sessions: `slot <n> <name>` and the arrangement's lines under it.
    pub fn to_text(&self) -> String {
        let mut text = format!("{HEADER}\n");
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some(slot) = slot {
                text += &format!("slot {} {}\n{}", i + 1, slot.name, slot.grid);
            }
        }
        text
    }

    /// The layouts `text` holds. Anything that is not this version's, or does not make sense, is ignored (those
    /// slots stay empty); a saved arrangement is checked when it is recalled, not here.
    pub fn from_text(text: &str) -> Layouts {
        let mut layouts = Layouts::default();
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return layouts;
        }
        let mut current: Option<(usize, String, String)> = None;
        let finish = |current: &mut Option<(usize, String, String)>, layouts: &mut Layouts| {
            if let Some((slot, name, grid)) = current.take() {
                let name = clean(&name);
                if !name.is_empty() {
                    layouts.slots[slot] = Some(Slot { name, grid });
                }
            }
        };
        for line in lines {
            if let Some(rest) = line.strip_prefix("slot ") {
                finish(&mut current, &mut layouts);
                let mut parts = rest.splitn(2, ' ');
                let number = parts
                    .next()
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|n| (1..=SLOTS).contains(n));
                current =
                    number.map(|n| (n - 1, parts.next().unwrap_or("").to_string(), String::new()));
            } else if let Some((_, _, grid)) = current.as_mut() {
                *grid += line;
                *grid += "\n";
            }
        }
        finish(&mut current, &mut layouts);
        layouts
    }
}

/// A name being typed for a slot: the Layout menu shows it with a caret until it is accepted or dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    pub slot: usize,
    pub text: String,
}

impl Rename {
    /// Start renaming `slot`, from its current name; `None` for an empty slot (there is nothing to name yet).
    pub fn start(layouts: &Layouts, slot: usize) -> Option<Rename> {
        layouts.name(slot).map(|name| Rename {
            slot,
            text: name.to_string(),
        })
    }

    /// Typed text: control characters are ignored and the name stops at [`NAME_LENGTH`] characters.
    pub fn type_text(&mut self, typed: &str) {
        for c in typed.chars().filter(|c| !c.is_control()) {
            if self.text.chars().count() < NAME_LENGTH {
                self.text.push(c);
            }
        }
    }

    pub fn backspace(&mut self) {
        self.text.pop();
    }

    /// Accept the name. Returns whether the layout's name changed (an empty name keeps the old one).
    pub fn accept(&self, layouts: &mut Layouts) -> bool {
        layouts.rename(self.slot, &self.text)
    }
}
