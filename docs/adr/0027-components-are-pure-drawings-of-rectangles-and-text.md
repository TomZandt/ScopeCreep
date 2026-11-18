# Style components are pure descriptions that draw as rectangles and text; legends are horizontal only

Spec #113 needs one set of parts (chip, segmented control, slider, popover, tooltip, title bar, top and status bars, legend strip) that every Panel and menu uses, testable without a GPU and reviewable in one image.

## Decision

- `app/src/components.rs` holds each component as a pure description: a size and hit area in device pixels at a scale, and a drawing as a list of `Prim`s (flat rectangle with rgba, or text in a type-scale face, colour and alignment inside a box). Each has the states Normal, Hover, Pressed and Disabled, plus on or off where it has one. Colours and sizes come only from `style.rs`; the file is in the style check's list.
- The drawing is renderer-neutral. `main.rs` turns prims into the existing flat rectangles and glyphon text (the right-click menu is now drawn by the popover component); the showcase example (`app/examples/style_showcase.rs`) turns the same prims into PNGs at 1x and 2x outside the repo. `app/src/showcase.rs` lays out the sheet and is tested for every component in every state.
- Pure layout cannot shape text, so widths come from a per-character estimate (`SPACE_CHAR_WIDTH`). Real text is clipped to its box, so a wrong estimate truncates, never overlaps.
- The channel colours (R, G, B, Y) are style tokens and the one place chrome is not neutral: a chip is in its channel's colour when on and grey when off.
- **Legends are horizontal only** (product owner, 2026-11-18): the legend strip component has no vertical form, and the Stops False Colour legend (a strip since #93) was the last vertical one to go, so none remains.

## Considered and rejected

- **Components drawn straight into the GPU by each one:** untestable without a GPU and the showcase would need the app's window.
- **Measuring text for layout:** needs the font system in the pure seam; the estimate plus clipping is enough for chrome.

## Consequences

- Wiring the title bar, chips and sliders into the Panels is the later tickets' work (#116 onward); this ticket delivers the parts and the popover.
