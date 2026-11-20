# #191: the View menu with legends on and off for every Panel

What ticket #191 showed on 2026-11-20 (spec #192), at the pure UI seam (`app/tests/menu_bar_seam.rs`), the command seam (`app/tests/commands_seam.rs`), the display seam (`app/tests/legends_seam.rs`, needs a GPU) and the Window's minimum-size test (`app/tests/min_window.rs`), headless.

- The menu bar now reads Layout, View, Settings. The View title sits between the two with no gap, is hit exactly where drawn, and every size doubles at 2x; its menu drops under its title.
- The View menu's first (and so far only) entry is "Show legends". It runs the command "Legends: show / hide" (`legends-toggle` in the hotkeys file), which has no default hotkey, is listed in Settings, then Hotkeys…, and can be assigned, cleared, reset and round-tripped through the hotkeys file like any command. The entry shows whatever hotkey the command has. It carries the popover's on mark while every legend is on.
- The entry sets the existing `legend` Panel setting (ADR 0013) of every Panel that offers one, on or off, through the same Panel settings the popover edits: ARRI False Colour, Stops False Colour and the saturation heatmap. Each is remembered per Panel and survives a save and reload of the settings text. Settings of other Panels, and other settings of those Panels, are untouched.
- A Panel's own Legend choice still works afterwards. "Every legend is on" counts defaults, so a fresh install shows the mark; one Panel off makes it off, and a click on the entry then turns all on. A click with every legend on turns all off.
- Any display whose settings include a two-choice `legend` setting (on/off) joins by itself; a new picture Panel with a legend needs nothing here.
- The Window's minimum size is unchanged at 469 x 645: the View menu, with the widest hotkey beside its entry, is in `largest_menus`, and every menu still fits under the bar from every title at 1x and 2x.
- Not yet looked at by a person at the app: #201.
