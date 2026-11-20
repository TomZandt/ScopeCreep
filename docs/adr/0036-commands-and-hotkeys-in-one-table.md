# 0036: Every app command is in one table, and the user's hotkeys are kept as changes from its defaults

Date: 2026-11-20 (#146, spec #142). See ADR 0030.

## Decision

Every app command (Default layout, show a layout slot, save into a slot) is one entry of a command table (`app/src/commands.rs`): a stable id, a label and a default hotkey. The menus are built from it, the window dispatches keys through it, and the Hotkeys panel edits it. A new command is one more entry.

A hotkey is a key by its position on the keyboard (winit's physical key, as ADR 0030 already did for the digits) plus Ctrl, Shift and Alt. A key with Super held is never a hotkey, and winit gives keys only to the focused window, so the app never takes keys from Resolve.

The defaults are the shortcuts the app had: Ctrl+1 to 9 show a layout, Ctrl+Shift+1 to 9 save one; Z is the Fit toggle (#169). A hotkey may be a key with no modifier, like Z: while a name is typed or a hotkey captured, keys go there first. Escape, and plain Enter, Backspace and Delete are reserved: the app uses them to close, accept and clear, so they can't be assigned.

No two commands share a hotkey. Assigning one another command holds is refused and names that command; the user then replaces (the other command is left with none) or cancels. Resetting a command to its default takes that default from whoever holds it.

The hotkeys file is `~/.config/scopecreep/hotkeys`, beside `layouts` and `panel-settings`: a `hotkeys 1` header, then one line per command whose hotkey differs from its default, `<id> <hotkey>` or `<id> none`. A line that names no command, no readable hotkey or a reserved one is skipped, so that command keeps its default; where the lines leave two commands on one hotkey, every changed command on it goes back to its default. A file of another version is ignored.

## Why

One table is the only way the menu, the dispatch and the Hotkeys panel can't disagree about a command's key. Keeping only the changes means a default added or changed in a later version reaches users who never touched that command, and a hand-edited or damaged file costs at most the lines it got wrong, never the app starting. Refusing a shared hotkey (instead of letting the last one win) keeps every key's meaning obvious.

## Consequences

- A hotkey can't be a key alone if that key is reserved, and there is no global hotkey (spec #142, out of scope).
- Changing a default in a later version moves it for everyone who kept the default, and a user whose change clashes with the new default loses the change at the next start (both go back to their defaults).
