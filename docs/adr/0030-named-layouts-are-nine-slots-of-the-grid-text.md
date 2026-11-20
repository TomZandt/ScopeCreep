# 0030: Named layouts are nine slots holding the grid's own saved text

Date: 2026-11-18 (#98). See ADR 0011, ADR 0012.

## Decision

A named layout is one of nine slots. Each slot holds a name and the Panel grid exactly as `PanelGrid::to_text` writes it (the same text that remembers the current arrangement). Recalling a slot reads that text with `PanelGrid::from_text` for the displays the Window has now, so a layout gets ADR 0012's rules for free: a Panel it names that no longer exists is dropped, a Panel it does not name takes the first free cell (or a row of its own), and a layout that leaves a hole or is rubbish gives the default layout.

The slots live in `~/.config/scopecreep/layouts`, beside `panel-grid` and `panel-settings`. The current arrangement is still remembered in `panel-grid`; a layout is a copy kept on purpose, so a recalled layout becomes the current one and is remembered like any other change.

The layout menu is the settings menu's `Menu` with other rows (opened by right-clicking the status line), so it is drawn and hit-tested by the same popover. Ctrl+1 to 9 recall and Ctrl+Shift+1 to 9 save by key position (not by the character, which Shift changes). A plain click on an empty slot saves into it; Shift-click saves over a layout; right-click renames, with the name typed into the row.

## Why

One format and one set of recovery rules, instead of a second grid format that could drift from the first. A saved layout can never put a Panel somewhere that does not fit the current displays, because it goes through the same checks as the grid that is remembered on every drag.

## Consequences

- A layout saved when a Panel was in the Window and recalled after that Panel left (with no other Panel to fill its cells) shows the default layout, not a layout with a gap. A Panel added since the layout was saved (one of the three spec #94 hides, if it returns) gets a row of its own under it.
- Layouts are not shared between machines and have no import or export.

## Note 2026-11-20

Showing, saving into and the default layout are commands in the command table, and Ctrl+1 to 9 and Ctrl+Shift+1 to 9 are their default hotkeys, which the user can change (ADR 0036, #146).

## Note 2026-11-20 (#147)

The layout menu is no longer opened by right-clicking the status line, and its Shift-click and right-click are gone with its hint heading: the Layout menu on the menu bar lists Default layout and the nine slots (a click shows a saved one, or saves into an empty one), with "Save current layout into…" and "Rename layout…" switching the menu to the slots to save into or rename. Every entry runs a command and shows its hotkey.
