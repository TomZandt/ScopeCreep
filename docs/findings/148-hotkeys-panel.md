# #148: the Hotkeys panel

What ticket #148 showed on 2026-11-20, through the panel's pure description and the command seam (`app/tests/hotkeys_panel_seam.rs`), headless.

- Settings, then Hotkeys…, shows the panel in place of the Settings menu: a heading, all 19 commands with their hotkeys (Default layout has none), and "Reset all to defaults" (greyed while nothing has changed).
- A click on a command makes its row the key-capture field ("Press a key", marked). The next combination is assigned and the row shows it; Escape cancels with nothing changed; Backspace or Delete clears. Pressing the key the command already has changes nothing. A click on the same row stops capturing; a click on another moves the field there.
- A combination another command has gives "Ctrl+2 is used by Show layout 2" with Replace and Cancel; while it asks, keys go to the panel, never to the other command. Cancel or Escape keeps both; Replace moves the key and leaves the other command with none.
- Plain Enter is refused with "Escape, Enter, Backspace and Delete are kept by the app" and capture goes on.
- "Reset to default (Ctrl+1)" shows while a command is captured, greyed at its default; reset all equals a fresh start.
- A change shows in the Layout menu at once, and an entry with no hotkey still runs its command. Changes round-trip through the hotkeys file (the window writes it on every change). With a conflict showing, the panel fits under the bar in the default Window at 1x and 2x.
- Review fixes to #147 in the same change: a bar menu always drops below the bar (a short Window cuts its foot off; #172 is the missing minimum size), a popover over the bar keeps its own clicks, the pointer leaving the Window unlights everything, a press shows on the entry, and a middle-button drag keeps moving when a menu opens. The press state, leaving and the drag are not covered by a test (window code).
- Checked at the app by the product owner on 2026-11-20: the Hotkeys panel works at a real keyboard, and the app takes no keys while Resolve has focus (#149).
