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

The first decision no longer holds. The Window is now a layout: a grid of columns and rows (the default is 5 × 4) with each Panel at a column and row and a span of columns and rows, and every cell used. The clean picture is 3 × 2 at the top left, by default. Column widths and row heights are still weights that add up to 1 and are remembered, so they hold at any window size; border drags move the weight between the two tracks beside the border, and no track goes under the minimum Panel size, so no Panel does. A border is draggable only between Panels, never through one that spans it. Panels are placed by display name, so a saved layout that names a Panel that has gone drops it, a neighbour fills the cell it left, and a Panel missing from it takes the first free cell (or a row of its own). A grid saved by v0.2.0 (it starts "panels") is ignored and the default used. Displays other than the default set fall back to an automatic grid (rows as equal as possible, every row filling the width). The third decision changes too: each Panel has a thin border and a title bar with its label at its top and no label strip under it. The rest stands.

## Update 2026-11-18 (#92): one Vectorscope Panel

The default layout has one Vectorscope Panel, two cells wide, in place of the two one-cell vectorscope Panels, so the Window has 9 Panels over the same 20 cells. A layout saved with the two old vectorscope Panels is ignored and the default used: its vectorscope is missing, and a layout that has lost Panels and is also missing a display does not fit.

## Update 2026-11-18 (#98): named layouts

Layouts are saved copies of this grid's text in nine named slots (ADR 0030). Recalling one uses the same rules as above: Panels that have gone are dropped and a neighbour widens or deepens into each cell they leave; new ones take the first free cell; a layout that leaves a cell empty after a display was also missing from it shows the default layout.

## Update 2026-11-19 (#151): the luma waveform follows the footage's aspect again

The luma waveform's plot no longer fills its whole Panel (#50 dropped the aspect on purpose). It takes the Feed frame's aspect (16:9 gives a 16:9 plot, 2.39:1 a wide short one, 9:16 a tall narrow one), scaled to fit and centred in the Panel's display area, the way the Picture stage fits a picture (ADR 0028). The gutter for the scale's numbers travels with the plot, so the numbers stay beside the axis, and the edge rows stay directly above and below it; the rest of the Panel is empty. A change of resolution re-lays it out on the next frame. Counts, traces, edge rows and Colorise are unchanged. The Vectorscope stays square and the Picture Views are untouched. No new trade-off: the plot is smaller on a Panel whose shape differs from the footage's, which is what the product owner asked for.

## Note 2026-11-20 (#179): saved weights are repaired, not refused

Saved column and row weights that leave a track under the minimum Panel size in the reference area (the default window) are no longer rubbish: each such track is raised to the minimum and the others scaled down, keeping their proportions, so the layout recalls with its places intact. A border dragged to the minimum in a much larger window therefore comes back a little further out there. Weights that are not finite and positive, do not add up to 1, do not fit the places, or cannot all reach the minimum are still refused and the default used.

## Note, 2026-11-20 (spec #196, ADR 0038)

The Panel grid (columns and rows of weights, spans, every cell used) is replaced by a layout tree of tab groups and splits. A split's ratio replaces the column and row weights, the tree can never leave a cell empty, and the minimum Panel sizes, the minimum Window rule and the repair of saved sizes (#179) carry over to it. A grid saved in the old text converts when it is a sequence of cuts (the default is), else the default page is used.
