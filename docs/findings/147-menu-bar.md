# #147: the menu bar with the Layout menu

What ticket #147 showed on 2026-11-20, at the pure UI seam (`app/tests/menu_bar_seam.rs`), the layouts seam and the style showcase (offscreen, 1x and 2x), headless.

- The bar is 24 logical pixels (thinner than the 32-pixel Colour Setup line under it), its titles side by side with no gap, hit exactly where drawn, and every size doubles at 2x. The open menu's title is pressed, the title under the pointer lit, nothing else.
- A dropped menu sits under its title, at least the popover's width and wider when a name and its hotkey need it; it moves left and up to stay in the window. Rows are the popover's height and separators 9 pixels; only enabled items are hit (never a separator, heading or greyed entry). Hover lights an item, a press darkens it, a greyed one never lights and its text is the disabled grey; the hotkey is right-aligned in the quieter text colour; the layout on screen has the popover's on mark.
- A click on a title opens its menu and a second click closes it; with one open, the pointer on another title opens that one. A click elsewhere, Escape or a right-click closes it.
- The Layout menu: Default layout, the nine slots ("1  empty", "2  Colour pass"), "Save current layout into…" and "Rename layout…" (greyed while no slot is saved). A saved slot runs Show layout n and shows Ctrl+n; an empty one runs Save layout into slot n and shows Ctrl+Shift+n, so a click does what the key beside it does. Save lists all nine slots with Ctrl+Shift+n; Rename lists only saved slots and shows the name being typed with a caret. Each ends with Back.
- Every entry that runs a command shows exactly the hotkey that runs the same command, with the default hotkeys and with changed, cleared and replaced ones; an entry that runs no command shows none.
- Every Layout menu mode, with nine long names, fits under the bar in a 480 x 400 window at 1x and 2x.
- The layout area starts under both bars; the saved-grid reference size (ADR 0012, #96) is unchanged, because the real area at the default window was already below it. Saved grids from a window much taller than the default can be refused at the next start; that is older than this ticket (#179).
- Checked at the app by the product owner on 2026-11-20: the bar, its menus and the Layout menu work with a real mouse, and the app takes no keys while Resolve has focus (#149).
