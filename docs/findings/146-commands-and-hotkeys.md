# #146: one command table and a hotkey model

What ticket #146 showed on 2026-11-20, at the pure command and hotkey seam (`app/tests/commands_seam.rs`) and the layouts seam (`app/tests/layouts_seam.rs`), headless.

- With no hotkeys file, Ctrl+1 to 9 are Show layout 1 to 9 and Ctrl+Shift+1 to 9 Save layout into slot 1 to 9, exactly the shortcuts #98 had; a digit alone, Shift+digit, Ctrl+0 and Ctrl+Alt+digit do nothing, and Default layout has no hotkey. The window turns winit's physical key into the hotkey, so Shift never changes the digit.
- Assigning moves a command to the new hotkey and frees the old one; a hotkey another command holds is refused with that command named, and cancelling leaves everything as it was. Replace takes it and leaves the other command with none. Reset takes the default back from whoever holds it; reset all equals a fresh start.
- Escape (with any modifier) and plain Enter, Backspace and Delete are refused; Ctrl+Enter is free.
- The file round-trips whatever the line order. A line with no such command, an unreadable or reserved hotkey, or a missing value keeps that command's default; another version or rubbish gives the defaults. A hand edit that puts a changed hotkey on a default one drops the edit; two edits on one hotkey drop both.
- The layout tests from #98 pass unchanged apart from the shortcut tests, which moved to the hotkey seam.
- Checked at the app by the product owner on 2026-11-20: the hotkeys work at a real keyboard and the app takes none while Resolve has focus (#149).
