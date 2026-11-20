//! The menu bar's behaviour (#147, spec #142): which menu is open, and what the Layout menu lists and does. Pure:
//! no GPU, no window. The drawing is the style components' [`MenuBar`] and [`DropMenu`]; every entry that does
//! something runs a [`Command`] from the command table, so it does what its hotkey does, and shows that hotkey.
use crate::commands::{Command, Hotkeys};
use crate::components::{DropMenu, MenuBar, MenuEntry, MenuItem};
use crate::layouts::{Layouts, Rename, SLOTS};

/// The menus on the bar, left to right.
pub const LAYOUT: usize = 0;
pub const VIEW: usize = 1;
pub const SETTINGS: usize = 2;

/// The menu bar's titles.
pub fn bar() -> MenuBar {
    MenuBar {
        titles: vec![
            "Layout".to_string(),
            "View".to_string(),
            "Settings".to_string(),
        ],
    }
}

/// Which menu is open. Menus open on a click on their title and close on a second click, a click elsewhere or
/// Escape; while one is open, resting on another title opens that one instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BarState {
    pub open: Option<usize>,
}

impl BarState {
    /// A click on title `title`: opens its menu, or closes it when it is the open one.
    pub fn click_title(&mut self, title: usize) {
        self.open = if self.open == Some(title) {
            None
        } else {
            Some(title)
        };
    }

    /// The pointer moved onto title `title`: with a menu open, that menu opens instead.
    pub fn hover_title(&mut self, title: usize) {
        if self.open.is_some() {
            self.open = Some(title);
        }
    }

    pub fn close(&mut self) {
        self.open = None;
    }
}

/// What the Layout menu is showing: the layouts to show, or the slots to save into or rename (the menu's own
/// entries switch between them, so no click needs a modifier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutMode {
    #[default]
    Show,
    Save,
    Rename,
    /// The list of displays to add as a Panel (ADR 0038, #206).
    Add,
}

/// What a click on a menu entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Run a command, as its hotkey does, and close the menu.
    Run(Command),
    /// Switch the Layout menu to other entries; it stays open.
    Mode(LayoutMode),
    /// Start typing a new name for the layout in a slot; the menu stays open.
    Rename(usize),
    /// Add Panel `.0` to the page.
    Add(usize),
    /// Show the Hotkeys panel in place of the Settings menu.
    OpenHotkeys,
    /// Something in the Hotkeys panel.
    Hotkeys(crate::hotkeys_panel::PanelAction),
}

/// What is on screen now: the default layout, or the arrangement saved in a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Current {
    Default,
    Slot(usize),
}

/// A menu with what each entry does (`None` for a separator, a heading or a disabled entry).
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub drop: DropMenu,
    pub actions: Vec<Option<MenuAction>>,
}

impl Menu {
    pub(crate) fn new() -> Menu {
        Menu {
            drop: DropMenu {
                entries: Vec::new(),
            },
            actions: Vec::new(),
        }
    }

    /// Add an entry; its action is dropped when it is not an enabled item.
    pub(crate) fn push(&mut self, entry: MenuEntry, action: Option<MenuAction>) {
        let usable = matches!(&entry, MenuEntry::Item(item) if item.enabled);
        self.drop.entries.push(entry);
        self.actions.push(action.filter(|_| usable));
    }

    /// What clicking entry `entry` does.
    pub fn action(&self, entry: usize) -> Option<MenuAction> {
        self.actions.get(entry).copied().flatten()
    }
}

fn hotkey_text(hotkeys: &Hotkeys, command: Command) -> String {
    hotkeys
        .get(command)
        .map_or_else(String::new, |h| h.to_string())
}

fn slot_text(layouts: &Layouts, slot: usize, renaming: Option<&Rename>) -> String {
    match renaming.filter(|r| r.slot == slot) {
        Some(r) => format!("{}  {}|", slot + 1, r.text),
        None => format!("{}  {}", slot + 1, layouts.name(slot).unwrap_or("empty")),
    }
}

/// The Layout menu.
///
/// - Show: Default layout, then the nine slots. A saved slot shows its layout (with Ctrl+digit beside it); an
///   empty one says so and saves the arrangement on screen into it (with Ctrl+Shift+digit beside it), so a slot
///   is filled with one click. The layout on screen is marked. Under them, "Save current layout into…" and
///   "Rename layout…" (greyed while no slot is saved).
/// - Save: the nine slots, each saving into itself; a saved one is replaced but keeps its name.
/// - Rename: the saved slots; a click starts typing the name into the row.
///
/// Save and Rename end with "Back".
pub fn layout_menu(
    layouts: &Layouts,
    hotkeys: &Hotkeys,
    current: Option<Current>,
    mode: LayoutMode,
    renaming: Option<&Rename>,
) -> Menu {
    let mut menu = Menu::new();
    let item = |label: String, command: Option<Command>, enabled: bool, on: bool| {
        MenuEntry::Item(MenuItem {
            label,
            hotkey: command.map_or_else(String::new, |c| hotkey_text(hotkeys, c)),
            enabled,
            on,
        })
    };
    let filled = |slot: usize| layouts.name(slot).is_some();
    match mode {
        // The Add list is [`add_menu`], which needs to know what is on the page; as a mode it opens as Show.
        LayoutMode::Show | LayoutMode::Add => {
            let default = Command::DefaultLayout;
            menu.push(
                item(
                    "Default layout".to_string(),
                    Some(default),
                    true,
                    current == Some(Current::Default),
                ),
                Some(MenuAction::Run(default)),
            );
            menu.push(MenuEntry::Separator, None);
            for slot in 0..SLOTS {
                let command = if filled(slot) {
                    Command::RecallLayout(slot)
                } else {
                    Command::SaveLayout(slot)
                };
                menu.push(
                    item(
                        slot_text(layouts, slot, None),
                        Some(command),
                        true,
                        current == Some(Current::Slot(slot)),
                    ),
                    Some(MenuAction::Run(command)),
                );
            }
            menu.push(MenuEntry::Separator, None);
            menu.push(
                item("Save current layout into…".to_string(), None, true, false),
                Some(MenuAction::Mode(LayoutMode::Save)),
            );
            let any = (0..SLOTS).any(filled);
            menu.push(
                item("Rename layout…".to_string(), None, any, false),
                Some(MenuAction::Mode(LayoutMode::Rename)),
            );
            menu.push(MenuEntry::Separator, None);
            for (label, command) in [
                ("Add panel…", Command::AddPanel),
                ("Close panel", Command::ClosePanel),
            ] {
                menu.push(
                    item(label.to_string(), Some(command), true, false),
                    Some(MenuAction::Run(command)),
                );
            }
        }
        LayoutMode::Save => {
            menu.push(
                MenuEntry::Heading("Save current layout into".to_string()),
                None,
            );
            for slot in 0..SLOTS {
                let command = Command::SaveLayout(slot);
                menu.push(
                    item(slot_text(layouts, slot, None), Some(command), true, false),
                    Some(MenuAction::Run(command)),
                );
            }
            menu.push(MenuEntry::Separator, None);
            menu.push(
                item("Back".to_string(), None, true, false),
                Some(MenuAction::Mode(LayoutMode::Show)),
            );
        }
        LayoutMode::Rename => {
            menu.push(MenuEntry::Heading("Rename layout".to_string()), None);
            for slot in 0..SLOTS {
                menu.push(
                    item(
                        slot_text(layouts, slot, renaming),
                        None,
                        filled(slot),
                        false,
                    ),
                    Some(MenuAction::Rename(slot)),
                );
            }
            menu.push(MenuEntry::Separator, None);
            menu.push(
                item("Back".to_string(), None, true, false),
                Some(MenuAction::Mode(LayoutMode::Show)),
            );
        }
    }
    menu
}

/// A display the Add list offers: the Panel it would bring onto the page, its name, and whether it can be added now
/// (a display already on the page cannot be added again until Panels can be copied, ADR 0039).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddEntry {
    pub panel: usize,
    pub name: String,
    pub enabled: bool,
}

/// The Add list (ADR 0038, #206): each display, then "Back" to the Layout menu.
pub fn add_menu(entries: &[AddEntry]) -> Menu {
    let mut menu = Menu::new();
    for e in entries {
        menu.push(
            MenuEntry::Item(MenuItem {
                label: e.name.clone(),
                hotkey: String::new(),
                enabled: e.enabled,
                on: false,
            }),
            Some(MenuAction::Add(e.panel)),
        );
    }
    menu.push(MenuEntry::Separator, None);
    menu.push(
        MenuEntry::Item(MenuItem::new("Back", "")),
        Some(MenuAction::Mode(LayoutMode::Show)),
    );
    menu
}

/// The View menu (#191): window-wide display choices. Its one entry shows or hides the legend strips of every
/// Panel at once; it is marked when every legend is on (`legends_on`).
pub fn view_menu(hotkeys: &Hotkeys, legends_on: bool) -> Menu {
    let command = Command::ToggleLegends;
    let mut menu = Menu::new();
    menu.push(
        MenuEntry::Item(MenuItem {
            label: "Show legends".to_string(),
            hotkey: hotkey_text(hotkeys, command),
            enabled: true,
            on: legends_on,
        }),
        Some(MenuAction::Run(command)),
    );
    menu
}

/// The Settings menu: Hotkeys, which opens the Hotkeys panel in its place.
pub fn settings_menu() -> Menu {
    let mut menu = Menu::new();
    menu.push(
        MenuEntry::Item(MenuItem::new("Hotkeys…", "")),
        Some(MenuAction::OpenHotkeys),
    );
    menu
}

/// Every menu at its largest, for the Window's minimum size (#172): the Layout menu in each mode with nine names of
/// the longest length and one being typed, the View menu with the widest hotkey, the Settings menu, and the Hotkeys panel asking about the longest
/// conflict.
pub fn largest_menus() -> Vec<DropMenu> {
    use crate::commands::{Hotkey, Key};
    use crate::hotkeys_panel::{HotkeysPanel, State};
    let mut layouts = Layouts::default();
    let grid = crate::layout_tree::LayoutTree::automatic(1);
    let long = "W".repeat(crate::layouts::NAME_LENGTH);
    for slot in 0..SLOTS {
        layouts.save(slot, &grid, &["x"]);
        layouts.rename(slot, &long);
    }
    let typing = Rename::start(&layouts, 0);
    let mut hotkeys = Hotkeys::default();
    let widest = Key::named("PageDown").map(|k| Hotkey::ctrl(k).with_shift().with_alt());
    let mut menus: Vec<DropMenu> = [LayoutMode::Show, LayoutMode::Save, LayoutMode::Rename]
        .into_iter()
        .map(|mode| layout_menu(&layouts, &hotkeys, None, mode, typing.as_ref()).drop)
        .collect();
    menus.push(settings_menu().drop);
    let names = crate::layout_tree::default_names();
    let all: Vec<AddEntry> = names
        .iter()
        .enumerate()
        .map(|(panel, name)| AddEntry {
            panel,
            name: name.to_string(),
            enabled: true,
        })
        .collect();
    menus.push(add_menu(&all).drop);
    // The longest name with the widest hotkey beside it is the widest row.
    let longest = Command::all()
        .into_iter()
        .max_by_key(|c| c.label().chars().count())
        .unwrap_or(Command::DefaultLayout);
    // The View menu with the widest hotkey beside its entry (and without one, if there is no widest).
    let mut view = Hotkeys::default();
    if let Some(hotkey) = widest {
        let _ = hotkeys.replace(longest, hotkey);
        let _ = view.replace(Command::ToggleLegends, hotkey);
    }
    menus.push(view_menu(&view, true).drop);
    if let Some(hotkey) = widest {
        let panel = HotkeysPanel {
            state: State::Conflict {
                command: Command::DefaultLayout,
                hotkey,
                other: longest,
            },
        };
        menus.push(panel.menu(&hotkeys).drop);
        let panel = HotkeysPanel {
            state: State::Capturing {
                command: longest,
                refused: true,
            },
        };
        menus.push(panel.menu(&hotkeys).drop);
    }
    menus
}
