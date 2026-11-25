//! Commands and hotkeys (#146, ADR 0036): every app command in one table with a default hotkey, and the pure model
//! of the hotkeys the user assigns, clears and resets. Pure: no GPU, no window; the window turns a key press into a
//! [`Hotkey`] with [`Key::from_code`] and asks [`Hotkeys::command_for`] what to do.
//!
//! A hotkey names a key by its position on the keyboard (winit's physical key), not by the character it types, so
//! Ctrl+Shift+1 stays Ctrl+Shift+1 although Shift turns the 1 into ! (ADR 0030).
use std::fmt;
use winit::keyboard::KeyCode;

use crate::pages::NUMBERED;

/// Something the app does that a menu entry offers and a hotkey can trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    /// Show the default layout on the current page.
    DefaultLayout,
    /// Go to page 1 to 9 (0 to 8): Ctrl+1 to 9 (ADR 0041, #212).
    GoToPage(usize),
    /// A new page with no Panel (ADR 0041). No default hotkey; the Page bar's **+** and the Layout menu offer the
    /// other two templates.
    NewPage,
    /// A new page that is a copy of the current one (ADR 0041). No default hotkey.
    DuplicatePage,
    /// Delete the current page; one page always stays (ADR 0041). No default hotkey.
    DeletePage,
    /// Resize the current page's splits so picture cells match the footage's aspect (ADR 0041, #213). No default hotkey.
    AutoResize,
    /// Re-arrange the current page once: the best cutting of its Panels into splits for the footage and the Window
    /// (ADR 0041, #214). No default hotkey.
    AutoLayout,
    /// The picture Panel under the pointer to Fit and back to its last zoom and spot (every picture Panel while
    /// Link is on), like Resolve's Shift+Z (#169).
    ZoomFit,
    /// The Panel under the pointer fills the Window, or the grid comes back (#180).
    FillWindow,
    /// Every Panel's legend on, or off when they all are (the View menu, #191). No default hotkey.
    ToggleLegends,
    /// Open the list of displays to add as a new Panel beside the focused one (ADR 0038, #206). No default hotkey.
    AddPanel,
    /// Close the focused Panel: its neighbour takes its space (ADR 0038, #206). No default hotkey.
    ClosePanel,
    /// Step back through the last 20 layout changes (ADR 0038, #207). Ctrl+Z.
    UndoLayout,
    /// Put every Panel's settings, on every page, back to its display's defaults, after asking (the Settings menu,
    /// #228). Leaves the arrangement, zoom, Link and the app's preferences alone. No default hotkey.
    ResetAllPanels,
    /// Open the plain-English licence page in the browser (the Help menu, #231). No default hotkey.
    Licence,
    /// Open the changelog page in the browser (the Help menu, #231). No default hotkey.
    Changelog,
    /// Open the author's website in the browser (the Help menu, #231). No default hotkey.
    Website,
    /// Open a new GitHub issue with the version and Feed protocol filled in (the Help menu, #231). No default hotkey.
    ReportProblem,
    /// Open the Releases page in the browser (the Help menu, #231). No default hotkey.
    CheckForUpdates,
    /// Open the Buy Me a Coffee page in the browser (the Help menu, #231). No default hotkey.
    BuyMeACoffee,
}

impl Command {
    /// Every command, in the order the Hotkeys panel lists them. A new command is one more entry here and one arm
    /// in each match below.
    pub fn all() -> Vec<Command> {
        let mut all = vec![Command::DefaultLayout];
        all.extend((0..NUMBERED).map(Command::GoToPage));
        all.push(Command::NewPage);
        all.push(Command::DuplicatePage);
        all.push(Command::DeletePage);
        all.push(Command::AutoLayout);
        all.push(Command::AutoResize);
        all.push(Command::ZoomFit);
        all.push(Command::FillWindow);
        all.push(Command::ToggleLegends);
        all.push(Command::AddPanel);
        all.push(Command::ClosePanel);
        all.push(Command::UndoLayout);
        all.push(Command::ResetAllPanels);
        all.push(Command::Licence);
        all.push(Command::Changelog);
        all.push(Command::Website);
        all.push(Command::ReportProblem);
        all.push(Command::CheckForUpdates);
        all.push(Command::BuyMeACoffee);
        all
    }

    /// The name kept in the hotkeys file: stable, never shown.
    pub fn id(self) -> String {
        match self {
            Command::DefaultLayout => "layout-default".to_string(),
            Command::GoToPage(page) => format!("page-go-{}", page + 1),
            Command::NewPage => "page-new".to_string(),
            Command::DuplicatePage => "page-duplicate".to_string(),
            Command::DeletePage => "page-delete".to_string(),
            Command::AutoLayout => "page-auto-layout".to_string(),
            Command::AutoResize => "page-auto-resize".to_string(),
            Command::ZoomFit => "zoom-fit".to_string(),
            Command::FillWindow => "panel-fill".to_string(),
            Command::ToggleLegends => "legends-toggle".to_string(),
            Command::AddPanel => "panel-add".to_string(),
            Command::ClosePanel => "panel-close".to_string(),
            Command::UndoLayout => "layout-undo".to_string(),
            Command::ResetAllPanels => "panels-reset-all".to_string(),
            Command::Licence => "help-licence".to_string(),
            Command::Changelog => "help-changelog".to_string(),
            Command::Website => "help-website".to_string(),
            Command::ReportProblem => "help-report-problem".to_string(),
            Command::CheckForUpdates => "help-check-for-updates".to_string(),
            Command::BuyMeACoffee => "help-buy-me-a-coffee".to_string(),
        }
    }

    /// What the Hotkeys panel calls the command.
    pub fn label(self) -> String {
        match self {
            Command::DefaultLayout => "Default layout".to_string(),
            Command::GoToPage(page) => format!("Go to page {}", page + 1),
            Command::NewPage => "Page: new".to_string(),
            Command::DuplicatePage => "Page: duplicate".to_string(),
            Command::DeletePage => "Page: delete".to_string(),
            Command::AutoLayout => "Page: auto layout".to_string(),
            Command::AutoResize => "Page: auto resize".to_string(),
            Command::ZoomFit => "Zoom: Fit / last zoom".to_string(),
            Command::FillWindow => "Panel: fill the Window".to_string(),
            Command::ToggleLegends => "Legends: show / hide".to_string(),
            Command::AddPanel => "Panel: add".to_string(),
            Command::ClosePanel => "Panel: close".to_string(),
            Command::UndoLayout => "Layout: undo change".to_string(),
            Command::ResetAllPanels => "Panels: reset all to defaults".to_string(),
            Command::Licence => "Help: Licence".to_string(),
            Command::Changelog => "Help: Changelog".to_string(),
            Command::Website => "Help: Website".to_string(),
            Command::ReportProblem => "Help: Report a problem".to_string(),
            Command::CheckForUpdates => "Help: Check for updates".to_string(),
            Command::BuyMeACoffee => "Help: Buy Me a Coffee".to_string(),
        }
    }

    /// The hotkey the command has until the user changes it: Ctrl+1 to 9 to go to a page, Z for the Fit toggle, Ctrl+F to fill the Window with a Panel, Ctrl+Z to undo a layout change. Default layout, the legends,
    /// adding and closing a Panel, the page commands that make or remove a page and the Help links have none.
    pub fn default_hotkey(self) -> Option<Hotkey> {
        let digit = |slot: usize| Key::named(&(slot + 1).to_string());
        match self {
            Command::DefaultLayout
            | Command::ToggleLegends
            | Command::AddPanel
            | Command::ClosePanel
            | Command::NewPage
            | Command::DuplicatePage
            | Command::DeletePage
            | Command::AutoLayout
            | Command::AutoResize
            | Command::ResetAllPanels
            | Command::Licence
            | Command::Changelog
            | Command::Website
            | Command::ReportProblem
            | Command::CheckForUpdates
            | Command::BuyMeACoffee => None,
            Command::GoToPage(page) => digit(page).map(Hotkey::ctrl),
            Command::ZoomFit => Key::named("Z").map(Hotkey::new),
            Command::FillWindow => Key::named("F").map(Hotkey::ctrl),
            Command::UndoLayout => Key::named("Z").map(Hotkey::ctrl),
        }
    }

    fn from_id(id: &str) -> Option<Command> {
        Command::all().into_iter().find(|c| c.id() == id)
    }
}

/// The keys a hotkey can use, by the name kept in the hotkeys file. The digits are the row above the letters.
const KEYS: &[&str] = &[
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "Space",
    "Tab",
    "Enter",
    "Escape",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Up",
    "Down",
    "Left",
    "Right",
    "Minus",
    "Equal",
    "BracketLeft",
    "BracketRight",
    "Semicolon",
    "Quote",
    "Comma",
    "Period",
    "Slash",
    "Backslash",
    "Backquote",
];

/// A key on the keyboard, by position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key(&'static str);

impl Key {
    /// The key called `name` in the hotkeys file ("1", "Z", "F5", "PageUp"); `None` for a key hotkeys don't use.
    pub fn named(name: &str) -> Option<Key> {
        KEYS.iter().find(|k| **k == name).map(|k| Key(k))
    }

    /// The name kept in the hotkeys file.
    pub fn name(self) -> &'static str {
        self.0
    }

    /// The key winit reports by position; `None` for a modifier or a key hotkeys don't use.
    pub fn from_code(code: KeyCode) -> Option<Key> {
        use KeyCode as C;
        let name = match code {
            C::Digit0 => "0",
            C::Digit1 => "1",
            C::Digit2 => "2",
            C::Digit3 => "3",
            C::Digit4 => "4",
            C::Digit5 => "5",
            C::Digit6 => "6",
            C::Digit7 => "7",
            C::Digit8 => "8",
            C::Digit9 => "9",
            C::KeyA => "A",
            C::KeyB => "B",
            C::KeyC => "C",
            C::KeyD => "D",
            C::KeyE => "E",
            C::KeyF => "F",
            C::KeyG => "G",
            C::KeyH => "H",
            C::KeyI => "I",
            C::KeyJ => "J",
            C::KeyK => "K",
            C::KeyL => "L",
            C::KeyM => "M",
            C::KeyN => "N",
            C::KeyO => "O",
            C::KeyP => "P",
            C::KeyQ => "Q",
            C::KeyR => "R",
            C::KeyS => "S",
            C::KeyT => "T",
            C::KeyU => "U",
            C::KeyV => "V",
            C::KeyW => "W",
            C::KeyX => "X",
            C::KeyY => "Y",
            C::KeyZ => "Z",
            C::F1 => "F1",
            C::F2 => "F2",
            C::F3 => "F3",
            C::F4 => "F4",
            C::F5 => "F5",
            C::F6 => "F6",
            C::F7 => "F7",
            C::F8 => "F8",
            C::F9 => "F9",
            C::F10 => "F10",
            C::F11 => "F11",
            C::F12 => "F12",
            C::Space => "Space",
            C::Tab => "Tab",
            C::Enter | C::NumpadEnter => "Enter",
            C::Escape => "Escape",
            C::Backspace => "Backspace",
            C::Delete => "Delete",
            C::Insert => "Insert",
            C::Home => "Home",
            C::End => "End",
            C::PageUp => "PageUp",
            C::PageDown => "PageDown",
            C::ArrowUp => "Up",
            C::ArrowDown => "Down",
            C::ArrowLeft => "Left",
            C::ArrowRight => "Right",
            C::Minus => "Minus",
            C::Equal => "Equal",
            C::BracketLeft => "BracketLeft",
            C::BracketRight => "BracketRight",
            C::Semicolon => "Semicolon",
            C::Quote => "Quote",
            C::Comma => "Comma",
            C::Period => "Period",
            C::Slash => "Slash",
            C::Backslash => "Backslash",
            C::Backquote => "Backquote",
            _ => return None,
        };
        Key::named(name)
    }

    /// How a menu shows the key: punctuation as its character, the rest by name.
    fn shown(self) -> &'static str {
        match self.0 {
            "PageUp" => "Page Up",
            "PageDown" => "Page Down",
            "Minus" => "-",
            "Equal" => "=",
            "BracketLeft" => "[",
            "BracketRight" => "]",
            "Semicolon" => ";",
            "Quote" => "'",
            "Comma" => ",",
            "Period" => ".",
            "Slash" => "/",
            "Backslash" => "\\",
            "Backquote" => "`",
            name => name,
        }
    }
}

/// A key with the modifiers held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Hotkey {
    pub fn new(key: Key) -> Hotkey {
        Hotkey {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    pub fn ctrl(key: Key) -> Hotkey {
        Hotkey {
            ctrl: true,
            ..Hotkey::new(key)
        }
    }

    pub fn with_shift(self) -> Hotkey {
        Hotkey {
            shift: true,
            ..self
        }
    }

    pub fn with_alt(self) -> Hotkey {
        Hotkey { alt: true, ..self }
    }

    /// Whether the app keeps this combination for itself, so it can't be assigned: Escape (it closes menus and
    /// cancels), and plain Enter, Backspace and Delete (they accept, and clear in the key-capture field).
    pub fn is_reserved(self) -> bool {
        let plain = !self.ctrl && !self.shift && !self.alt;
        self.key.0 == "Escape" || (plain && matches!(self.key.0, "Enter" | "Backspace" | "Delete"))
    }

    /// The hotkey written as the hotkeys file keeps it: "Ctrl+Shift+1".
    pub fn to_text(self) -> String {
        self.join(self.key.name())
    }

    /// The hotkey `text` names, as [`Hotkey::to_text`] writes it; `None` for anything else.
    pub fn from_text(text: &str) -> Option<Hotkey> {
        let mut parts: Vec<&str> = text.split('+').collect();
        let mut hotkey = Hotkey::new(Key::named(parts.pop()?)?);
        for part in parts {
            let held = match part {
                "Ctrl" => &mut hotkey.ctrl,
                "Shift" => &mut hotkey.shift,
                "Alt" => &mut hotkey.alt,
                _ => return None,
            };
            if *held {
                return None;
            }
            *held = true;
        }
        Some(hotkey)
    }

    fn join(self, key: &str) -> String {
        let mut text = String::new();
        for (held, name) in [
            (self.ctrl, "Ctrl+"),
            (self.shift, "Shift+"),
            (self.alt, "Alt+"),
        ] {
            if held {
                text += name;
            }
        }
        text + key
    }
}

/// How a menu shows the hotkey: "Ctrl+Shift+1", "Ctrl+-".
impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.join(self.key.shown()))
    }
}

/// Why a hotkey was not assigned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The app keeps the combination for itself.
    Reserved,
    /// Another command has it: [`Hotkeys::replace`] takes it from that command, or the user cancels.
    Conflict(Command),
}

/// The first line of the hotkeys file.
const HEADER: &str = "hotkeys 1";

/// The hotkey each command has: its default unless the user assigned another or cleared it. No two commands share
/// a hotkey.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hotkeys {
    /// Commands whose hotkey differs from the default, with what it is now (`None`: cleared).
    changed: Vec<(Command, Option<Hotkey>)>,
}

impl Hotkeys {
    /// The hotkey `command` has now.
    pub fn get(&self, command: Command) -> Option<Hotkey> {
        match self.changed.iter().find(|(c, _)| *c == command) {
            Some((_, hotkey)) => *hotkey,
            None => command.default_hotkey(),
        }
    }

    /// The command `hotkey` triggers, if any.
    pub fn command_for(&self, hotkey: Hotkey) -> Option<Command> {
        Command::all()
            .into_iter()
            .find(|c| self.get(*c) == Some(hotkey))
    }

    /// Give `command` the hotkey, unless the app keeps it or another command has it (that command is named, so the
    /// user can replace or cancel). Assigning a command the hotkey it has already changes nothing.
    pub fn assign(&mut self, command: Command, hotkey: Hotkey) -> Result<(), Refused> {
        if hotkey.is_reserved() {
            return Err(Refused::Reserved);
        }
        match self.command_for(hotkey) {
            Some(other) if other != command => Err(Refused::Conflict(other)),
            _ => {
                self.set(command, Some(hotkey));
                Ok(())
            }
        }
    }

    /// Give `command` the hotkey, taking it from the command that had it (which is left with none). A reserved
    /// hotkey is still refused.
    pub fn replace(&mut self, command: Command, hotkey: Hotkey) -> Result<(), Refused> {
        if hotkey.is_reserved() {
            return Err(Refused::Reserved);
        }
        if let Some(other) = self.command_for(hotkey).filter(|o| *o != command) {
            self.set(other, None);
        }
        self.set(command, Some(hotkey));
        Ok(())
    }

    /// Leave `command` with no hotkey; it still works from its menu.
    pub fn clear(&mut self, command: Command) {
        self.set(command, None);
    }

    /// Put `command` back to its default hotkey. A command now holding that hotkey loses it, so no two share one.
    pub fn reset(&mut self, command: Command) {
        if let Some(default) = command.default_hotkey()
            && let Some(other) = self.command_for(default).filter(|o| *o != command)
        {
            self.set(other, None);
        }
        self.set(command, command.default_hotkey());
    }

    /// Put every command back to its default.
    pub fn reset_all(&mut self) {
        self.changed.clear();
    }

    fn set(&mut self, command: Command, hotkey: Option<Hotkey>) {
        self.changed.retain(|(c, _)| *c != command);
        if hotkey != command.default_hotkey() {
            self.changed.push((command, hotkey));
            let all = Command::all();
            self.changed
                .sort_by_key(|(c, _)| all.iter().position(|a| a == c));
        }
    }

    /// The hotkeys, to remember between sessions: one line per command whose hotkey is not its default,
    /// `<command> <hotkey>` or `<command> none`.
    pub fn to_text(&self) -> String {
        let mut text = format!("{HEADER}\n");
        for command in Command::all() {
            if let Some((_, hotkey)) = self.changed.iter().find(|(c, _)| *c == command) {
                let value = hotkey.map_or_else(|| "none".to_string(), Hotkey::to_text);
                text += &format!("{} {value}\n", command.id());
            }
        }
        text
    }

    /// The hotkeys `text` holds. A file of another version is ignored; a line that names no command, no hotkey or a
    /// reserved one is ignored, so that command keeps its default. Where the lines leave two commands sharing a
    /// hotkey, every changed one of them goes back to its default, until none do.
    pub fn from_text(text: &str) -> Hotkeys {
        let mut hotkeys = Hotkeys::default();
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(HEADER) {
            return hotkeys;
        }
        for line in lines {
            let mut parts = line.split_whitespace();
            let (Some(id), Some(value), None) = (parts.next(), parts.next(), parts.next()) else {
                continue;
            };
            let Some(command) = Command::from_id(id) else {
                continue;
            };
            let hotkey = match value {
                "none" => None,
                value => match Hotkey::from_text(value).filter(|h| !h.is_reserved()) {
                    Some(hotkey) => Some(hotkey),
                    None => continue,
                },
            };
            hotkeys.set(command, hotkey);
        }
        while let Some(shared) = hotkeys.shared_hotkey() {
            let holders: Vec<Command> = Command::all()
                .into_iter()
                .filter(|c| hotkeys.get(*c) == Some(shared))
                .collect();
            let before = hotkeys.changed.len();
            hotkeys.changed.retain(|(c, _)| !holders.contains(c));
            if hotkeys.changed.len() == before {
                break;
            }
        }
        hotkeys
    }

    /// A hotkey two commands have, at least one of them by a change (the defaults never share one).
    fn shared_hotkey(&self) -> Option<Hotkey> {
        let held: Vec<Hotkey> = Command::all()
            .into_iter()
            .filter_map(|c| self.get(c))
            .collect();
        held.iter()
            .enumerate()
            .find(|(i, h)| held[i + 1..].contains(h))
            .map(|(_, h)| *h)
    }
}
