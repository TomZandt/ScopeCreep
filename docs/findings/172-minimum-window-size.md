# #172: the Window's minimum size

What ticket #172 showed on 2026-11-20, at the pure seam (`app/tests/min_window.rs`), headless.

- The minimum is worked out, not chosen: the least Panel area in which every Panel of the default layout is at least its minimum (120 x 105) is 318 x 513.3 (the 2-column Panels set the width, the 11/50-weight rows the height). With the margins, the menu bar, the Colour Setup line and a two-line status row that gives 342 x 602.
- The menus need more: the Hotkeys panel with a conflict showing is 468.4 wide (its "… is used by …" line with the widest hotkey) and 621 tall, under the 24-pixel menu bar. So the Window's minimum is 469 x 645 logical pixels, set with winit's minimum inner size once the displays are known. It grows by itself as commands are added.
- At the minimum, at 1x, 1.5x and 2x, every Panel of the default layout is at least its minimum and its display at least 80 tall, and every menu in its largest form (each Layout mode with nine 24-character names, Settings, the Hotkeys panel capturing and in conflict) fits across and under the bar from every title. One pixel less of Panel area either way leaves a Panel under its minimum, so the grid part is tight.
- Only the default layout is guaranteed: a layout dragged in a big Window scales with the Window, so its smallest Panel can go under the minimum in a smaller one. A minimum that followed the dragged layout would stop the Window shrinking after one drag in a large Window.
- Checked at the app by the product owner on 2026-11-20: the Window stops at its minimum and nothing is cut off (#182).
