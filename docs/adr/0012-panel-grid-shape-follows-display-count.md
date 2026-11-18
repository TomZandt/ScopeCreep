# The Panel grid's shape follows the number of displays, and Panels fill their cells

## Decision

- The grid's rows and columns depend only on the number of displays (columns about the square root of the count times 16:9, rows as equal as possible, every row filling the width), never on the window's size.
- Remembered border positions are weights per row and per row's Panels, so they stay valid at any window size. Positions saved for a different number of displays are ignored.
- Each Panel fills its cell: the display above, its label under it. The Picture View still keeps the frame's shape inside its Panel, but the waveform fills its own Panel, so a picture column no longer sits exactly level with its trace column (milestone 1).
- Every display implements one shape (`Display`) and is registered in one place (`DisplaySet::standard`).

## Considered and rejected

- **Choosing columns from the window's shape:** a better fit for any one window, but a resize can change the shape and make saved border positions meaningless.
- **Keeping the picture and waveform columns level:** needs both Panels to share a width and the frame's shape, which cannot hold for 14 Panels the user can resize.

## Consequences

- At 14 Panels in a 1280 × 760 window the smallest Panel is about 240 × 225 pixels; a Panel cannot be dragged below 120 × 100 logical pixels.
- A later change can bring back column alignment for the two displays that need it only by giving up free resizing for them.

## Update 2026-11-18 (#96): the shape no longer follows the count

The first decision no longer holds. The Window is now a layout: a grid of columns and rows (the default is 5 × 4) with each Panel at a column and row and a span of columns and rows, and every cell used. The clean picture is 3 × 2 at the top left, by default. Column widths and row heights are still weights that add up to 1 and are remembered, so they hold at any window size; border drags move the weight between the two tracks beside the border, and no track goes under the minimum Panel size, so no Panel does. A border is draggable only between Panels, never through one that spans it. Panels are placed by display name, so a saved layout that names a Panel that has gone drops it, and a Panel missing from it takes the first free cell (or a row of its own). A grid saved by v0.2.0 (it starts "panels") is ignored and the default used. Displays other than the default set fall back to an automatic grid (rows as equal as possible, every row filling the width). The third decision changes too: each Panel has a thin border and a title bar with its label at its top and no label strip under it. The rest stands.

## Update 2026-11-18 (#92): a grid with a hole is ignored

The default layout has one Vectorscope Panel, two cells wide, in place of the two one-cell vectorscope Panels, so the Window has 10 Panels over the same 20 cells. A saved layout that leaves any cell empty once its Panels that no longer exist are dropped no longer fits the displays and is ignored: the default is used. (Before, a display missing from the saved layout took the first free cell and could leave a hole.)

## Update 2026-11-18 (#98): named layouts

Layouts are saved copies of this grid's text in nine named slots (ADR 0030). Recalling one uses the same rules as above: Panels that have gone are dropped, new ones take the first free cell, a layout with a hole shows the default layout.
