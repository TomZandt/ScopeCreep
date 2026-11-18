# #96: Panels span cells, the clean picture is the hero

What ticket #96 showed on 2026-11-18, at the pure layout seam (`app/tests/panel_grid.rs`, no GPU, no Resolve).

- The default layout is 5 columns by 4 rows with row weights 14:14:11:11. The clean picture is 3 x 2 at the top left; the luma waveform spans two columns to its right, the two vectorscopes (one cell each until #92 merges them) below it; the Colour Waveform is 2 x 2 under the picture, and the other six Picture Views fill the last three columns of the bottom two rows. Every cell is used.
- At 1280 x 760 (Panel area 1256 x 716) the smallest Panel is a single cell of about 239 x 150 device pixels; the minimum Panel size is 120 x 105 logical pixels (a display of 80 under a 22 px title bar and the border), so every Panel is above it.
- The hero is about 740 x 390 pixels at that size, against about 240 x 225 for the biggest Panel in the old 14-Panel grid.
- The title bar is one line high and the label is clipped to it, so a long label (today's labels are long until #99) cuts off instead of taking room from the display. The label is not yet shortened.
- The border is drawn as thin lines over the Panels' edge, with a line under the title bar; the flat-rectangle buffer grew from 64 to 256 rectangles to hold them with the menu.
- The layout is saved as `layout 1`, the column and row weights, and one `panel <column> <row> <columns> <rows> <name>` line per display. A v0.2.0 grid starts "panels" and is ignored.

Not verified (a person at the app and Resolve): the look of the layout at 1280 x 760 and on a real screen, dragging borders with a real mouse, UHD 50 playback with the new layout: filed as ready-for-human: https://github.com/TomZandt/scopecreep/issues/132
