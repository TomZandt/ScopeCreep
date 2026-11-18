# #98: named layouts

What ticket #98 showed on 2026-11-18, at the pure layout seam (`app/tests/layouts_seam.rs`, no GPU, no Resolve).

- A layout is saved into one of nine slots, recalled, renamed and remembered between sessions; the saved text is the grid's own (ADR 0030), so a layout keeps its border positions and spans.
- A layout naming a Panel that no longer exists drops it; a Panel the layout does not name goes in the first free cell, or a row of its own when there is none (the same rules as the remembered grid). A layout that would leave a cell empty, or is rubbish, recalls the default layout.
- Ctrl+1 to 9 and Ctrl+Shift+1 to 9 are decided by one pure function from the digit and the modifiers; Alt or no Ctrl is not a layout shortcut.
- The layout menu is the settings menu's rows (heading, "Default layout", nine slots, a line of how-to), so the popover draws it. Names are cleaned (no control characters, 24 characters, trimmed) and an empty name keeps the old one.

Not verified (a person at the app): key handling on a real keyboard (Ctrl+Shift+digit is read by key position), the menu opening on the status line, typing a name: https://github.com/TomZandt/scopecreep/issues/136
