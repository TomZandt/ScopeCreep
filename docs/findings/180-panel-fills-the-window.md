# #180: Ctrl+F, the Panel under the pointer fills the Window

What ticket #180 showed on 2026-11-20, at the Panel grid's pure seam (`app/tests/panel_grid.rs`) and the command seam (`app/tests/commands_seam.rs`), headless.

- With a Panel filling the Window, its cell is the whole Panel area and every other Panel's cell is empty and far off screen: its title bar and display can never be hit in the window, and the window skips drawing it (display, title text, scale numbers, chips and border). With none filling, or an index that is not a Panel, the cells are the grid's own.
- "Panel: fill the Window" has Ctrl+F by default (F alone is not it) and is reassigned, cleared, reset and saved like any command.
- In the window: Ctrl+F over a Panel fills the Window with it, Ctrl+F anywhere brings the grid back, Escape too when no menu is open, and showing a layout or the default does as well. Border and Panel drags are off meanwhile. It is never saved.
- Not verified at the app: the toggle with a real keyboard and mouse beside Resolve, including the notice placement while the clean picture is hidden (#181).
