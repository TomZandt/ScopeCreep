//! The menu bar's behaviour (#147, spec #142): which menu is open, and what the Layout menu lists and does. Pure:
//! no GPU, no window. The drawing is the style components' [`MenuBar`] and [`DropMenu`]; every entry that does
//! something runs a [`Command`] from the command table, so it does what its hotkey does, and shows that hotkey.
use crate::commands::{Command, Hotkeys};
use crate::components::{DropMenu, MenuBar, MenuEntry, MenuItem};

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

/// What the Layout menu is showing: the pages and what to do with them, or the templates a new page can start from
/// (the menu's own entries switch between them, so no click needs a modifier), or the Add list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutMode {
    #[default]
    Show,
    /// "New page from": Empty page, Default layout and Copy of this page (ADR 0041, #212).
    NewFrom,
    /// The list of displays to add as a Panel (ADR 0038, #206).
    Add,
}

/// What a new page starts from (ADR 0041).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    /// No Panel at all.
    Empty,
    /// The default layout: the 7 Panels.
    Default,
    /// A copy of the current page, with each Panel's settings.
    Copy,
    /// A page template, arranged for the footage (#215).
    From(crate::page_templates::PageTemplate),
}

/// What a click on a menu entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Run a command, as its hotkey does, and close the menu.
    Run(Command),
    /// Switch the Layout menu to other entries; it stays open.
    Mode(LayoutMode),
    /// Go to page `.0` and close the menu.
    Page(usize),
    /// A new page from a template.
    NewPage(Template),
    /// Start typing a new name for the current page in its tab, and close the menu.
    RenamePage,
    /// Add Panel `.0` to the page.
    Add(usize),
    /// Show the Hotkeys panel in place of the Settings menu.
    OpenHotkeys,
    /// Something in the Hotkeys panel.
    Hotkeys(crate::hotkeys_panel::PanelAction),
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

/// The Layout menu (ADR 0041, #212).
///
/// - Show: every page (the current one marked, the first nine with their Ctrl+number beside them), then New page
///   from, Duplicate page, Rename page and Delete page (greyed with one page), then Add panel, Close panel,
///   Undo layout change and Default layout.
/// - NewFrom: Empty page, Default layout (the 7 Panels), Copy of this page and each page template, then "Back".
pub fn layout_menu(names: &[String], current: usize, hotkeys: &Hotkeys, mode: LayoutMode) -> Menu {
    layout_menu_with_undo(names, current, hotkeys, mode, true)
}

/// [`layout_menu`] that knows whether there is a layout change to undo: "Undo layout change" is greyed when not.
/// ([`layout_menu`] itself shows it enabled, for the menus that only measure sizes.)
pub fn layout_menu_with_undo(
    names: &[String],
    current: usize,
    hotkeys: &Hotkeys,
    mode: LayoutMode,
    can_undo: bool,
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
    match mode {
        // The Add list is [`add_menu`], which needs to know what is on the page; as a mode it opens as Show.
        LayoutMode::Show | LayoutMode::Add => {
            for (i, name) in names.iter().enumerate() {
                let command = (i < crate::pages::NUMBERED).then_some(Command::GoToPage(i));
                menu.push(
                    item(format!("{}  {}", i + 1, name), command, true, i == current),
                    Some(MenuAction::Page(i)),
                );
            }
            menu.push(MenuEntry::Separator, None);
            menu.push(
                item("New page from…".to_string(), None, true, false),
                Some(MenuAction::Mode(LayoutMode::NewFrom)),
            );
            menu.push(
                item(
                    "Duplicate page".to_string(),
                    Some(Command::DuplicatePage),
                    true,
                    false,
                ),
                Some(MenuAction::Run(Command::DuplicatePage)),
            );
            menu.push(
                item("Rename page".to_string(), None, true, false),
                Some(MenuAction::RenamePage),
            );
            menu.push(
                item(
                    "Delete page".to_string(),
                    Some(Command::DeletePage),
                    names.len() > 1,
                    false,
                ),
                Some(MenuAction::Run(Command::DeletePage)),
            );
            menu.push(MenuEntry::Separator, None);
            for (label, command, enabled) in [
                ("Add panel…", Command::AddPanel, true),
                ("Close panel", Command::ClosePanel, true),
                ("Auto layout", Command::AutoLayout, true),
                ("Auto resize", Command::AutoResize, true),
                ("Undo layout change", Command::UndoLayout, can_undo),
                ("Default layout", Command::DefaultLayout, true),
            ] {
                menu.push(
                    item(label.to_string(), Some(command), enabled, false),
                    Some(MenuAction::Run(command)),
                );
            }
        }
        LayoutMode::NewFrom => {
            menu.push(MenuEntry::Heading("New page from".to_string()), None);
            let default = format!(
                "Default layout ({} Panels)",
                crate::layout_tree::default_names().len()
            );
            for (label, template, command) in [
                (
                    "Empty page".to_string(),
                    Template::Empty,
                    Some(Command::NewPage),
                ),
                (default, Template::Default, None),
                (
                    "Copy of this page".to_string(),
                    Template::Copy,
                    Some(Command::DuplicatePage),
                ),
            ] {
                menu.push(
                    item(label, command, true, false),
                    Some(MenuAction::NewPage(template)),
                );
            }
            for t in crate::page_templates::PageTemplate::ALL {
                menu.push(
                    item(t.name().to_string(), None, true, false),
                    Some(MenuAction::NewPage(Template::From(t))),
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
    let long = "W".repeat(crate::layouts::NAME_LENGTH);
    let names: Vec<String> = (0..crate::pages::MAX_PAGES).map(|_| long.clone()).collect();
    let mut hotkeys = Hotkeys::default();
    let widest = Key::named("PageDown").map(|k| Hotkey::ctrl(k).with_shift().with_alt());
    let mut menus: Vec<DropMenu> = [LayoutMode::Show, LayoutMode::NewFrom]
        .into_iter()
        .map(|mode| layout_menu(&names, 0, &hotkeys, mode).drop)
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
