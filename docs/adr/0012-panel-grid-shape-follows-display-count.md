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
