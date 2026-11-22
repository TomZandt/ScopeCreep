//! The Appearance panel (spec #196, #226, ADR 0044): Settings > Appearance > App accent colour. Swatches, a hex box,
//! Reset and a notice that says when the accent reads poorly as text on a chip. Pure: no GPU, no window. It is drawn
//! as a menu dropped from the Settings title ([`crate::components::DropMenu`]), like the Hotkeys panel.
//!
//! The hex box behaves as the value box of ADR 0027: a click on it starts typing (the hex so far, a caret), Enter
//! accepts, Escape cancels, and anything that is not a hex colour is refused and the accent stays as it was.
use crate::accent::{self, Accent};
use crate::components::{MenuEntry, MenuItem};
use crate::menu_bar::{Menu, MenuAction};
use crate::style;

/// What a click in the panel does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppearanceAction {
    /// Take swatch `.0` of [`accent::SWATCHES`].
    Swatch(usize),
    /// Start typing a hex into the box (a click on it while typing accepts what is typed).
    EditHex,
    /// Back to the default accent.
    Reset,
}

/// The longest a hex is typed to: `#` and six digits.
pub const HEX_LENGTH: usize = 7;

/// The Appearance panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppearancePanel {
    /// The hex being typed into the box, if any.
    pub editing: Option<String>,
}

/// What the notice under the swatches says for `accent`.
pub fn contrast_notice(accent: Accent) -> String {
    let ratio = accent.contrast();
    if accent.low_contrast() {
        format!(
            "Contrast {ratio:.1}:1 on a chip: under {:.1}:1, hard to read",
            accent::MIN_CONTRAST
        )
    } else {
        format!("Contrast {ratio:.1}:1 on a chip")
    }
}

impl AppearancePanel {
    /// Whether the panel wants every key (a hex is being typed).
    pub fn takes_keys(&self) -> bool {
        self.editing.is_some()
    }

    /// The panel's rows for the accent `current`.
    pub fn menu(&self, current: Accent) -> Menu {
        let mut menu = Menu::new();
        menu.push(MenuEntry::Heading("App accent colour".to_string()), None);
        let swatches = accent::SWATCHES
            .iter()
            .map(|(_, hex)| {
                let rgb = accent::parse_hex(hex).unwrap_or(accent::DEFAULT);
                (style::rgb(rgb), Accent(rgb) == current)
            })
            .collect();
        menu.push(MenuEntry::Swatches(swatches), None);
        let shown = match &self.editing {
            Some(typed) => format!("{typed}|"),
            None => current.hex(),
        };
        let mut hex = MenuItem::new("Hex", &shown);
        hex.on = self.editing.is_some();
        menu.push(
            MenuEntry::Item(hex),
            Some(MenuAction::Appearance(AppearanceAction::EditHex)),
        );
        menu.push(MenuEntry::Heading(contrast_notice(current)), None);
        menu.push(MenuEntry::Separator, None);
        let mut reset = MenuItem::new(&format!("Reset to default ({})", accent::DEFAULT_HEX), "");
        reset.enabled = current != Accent::default();
        menu.push(
            MenuEntry::Item(reset),
            Some(MenuAction::Appearance(AppearanceAction::Reset)),
        );
        menu
    }

    /// A click on one of the panel's entries. Returns whether the accent changed (and should be remembered).
    pub fn click(&mut self, action: AppearanceAction, current: &mut Accent) -> bool {
        let before = *current;
        match action {
            AppearanceAction::Swatch(i) => {
                if let Some(rgb) = accent::SWATCHES
                    .get(i)
                    .and_then(|(_, hex)| accent::parse_hex(hex))
                {
                    *current = Accent(rgb);
                }
                self.editing = None;
            }
            AppearanceAction::EditHex => match self.editing.take() {
                Some(typed) => self.accept_text(&typed, current),
                None => self.editing = Some(current.hex()),
            },
            AppearanceAction::Reset => {
                *current = Accent::default();
                self.editing = None;
            }
        }
        *current != before
    }

    fn accept_text(&mut self, typed: &str, current: &mut Accent) {
        if let Some(rgb) = accent::parse_hex(typed) {
            *current = Accent(rgb);
        }
    }

    /// Typed text: only hex digits and one leading `#`, up to [`HEX_LENGTH`] characters.
    pub fn type_text(&mut self, typed: &str) {
        let Some(text) = self.editing.as_mut() else {
            return;
        };
        for c in typed.chars() {
            let allowed = c.is_ascii_hexdigit() || (c == '#' && text.is_empty());
            if allowed && text.chars().count() < HEX_LENGTH {
                text.push(c.to_ascii_lowercase());
            }
        }
    }

    pub fn backspace(&mut self) {
        if let Some(text) = self.editing.as_mut() {
            text.pop();
        }
    }

    /// Enter: accept what is typed (a bad hex is refused and the accent stays). Returns whether the accent changed.
    pub fn accept(&mut self, current: &mut Accent) -> bool {
        let before = *current;
        if let Some(typed) = self.editing.take() {
            self.accept_text(&typed, current);
        }
        *current != before
    }

    /// Escape: stop typing, nothing changes.
    pub fn cancel(&mut self) {
        self.editing = None;
    }
}
