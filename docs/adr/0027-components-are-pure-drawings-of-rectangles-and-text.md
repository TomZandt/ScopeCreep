# Style components are pure descriptions that draw as rectangles and text; legends are horizontal only

Spec #113 needs one set of parts (chip, segmented control, slider, popover, tooltip, title bar, top and status bars, legend strip) that every Panel and menu uses, testable without a GPU and reviewable in one image.

## Decision

- `app/src/components.rs` holds each component as a pure description: a size and hit area in device pixels at a scale, and a drawing as a list of `Prim`s (flat rectangle with rgba, or text in a type-scale face, colour and alignment inside a box). Each has the states Normal, Hover, Pressed and Disabled, plus on or off where it has one. Colours and sizes come only from `style.rs`; the file is in the style check's list.
- The drawing is renderer-neutral. `main.rs` turns prims into the existing flat rectangles and glyphon text (the right-click menu is now drawn by the popover component); the showcase example (`app/examples/style_showcase.rs`) turns the same prims into PNGs at 1x and 2x outside the repo. `app/src/showcase.rs` lays out the sheet and is tested for every component in every state.
- Pure layout cannot shape text, so widths come from a per-character estimate (`SPACE_CHAR_WIDTH`). Real text is clipped to its box, so a wrong estimate truncates, never overlaps.
- The channel colours (R, G, B, Y) are style tokens and the one place chrome is not neutral: a chip is in its channel's colour when on and grey when off.
- **Legends are horizontal only** (product owner, 2026-11-18): the legend strip component has no vertical form, and the Stops False Colour legend (a strip since #93) was the last vertical one to go, so none remains.
- **Toggles are chips, amounts are sliders with a value box** (product owner, 2026-11-20, #225). An amount is one popover row modelled on Resolve's own: label, slider, a value box (a number to type into, in the slider's unit, with increase and decrease arrows, Up and Down, Shift for big steps, Enter to accept, Escape to cancel, clamped to the range), and a circular reset icon that replaces the old "Reset to default" text line (tooltip "Reset to 100 %", dim at the default). The slider and the box stay in step, and a drag updates both live. A popover also has one Panel reset icon in the Panel section's corner (tooltip "Reset this Panel's settings") that puts every setting of that Panel back to default, settings only. The value box, arrows and reset are parts of the slider component, so they are pure descriptions drawn from tokens like the rest.

## Considered and rejected

- **Components drawn straight into the GPU by each one:** untestable without a GPU and the showcase would need the app's window.
- **Measuring text for layout:** needs the font system in the pure seam; the estimate plus clipping is enough for chrome.

## Consequences

- Wiring the title bar, chips and sliders into the Panels is the later tickets' work (#116 onward); this ticket delivers the parts and the popover.

## Update (ADR 0044, #226)

Chips that are on are accent text with a thin accent line on a lighter grey (not a filled accent), and hover is a neutral lift. The accent is the user's colour (default #d2ff00).

- **Fonts and icons (#238):** the text in a prim is DM Mono (or Anton for the wordmark face) and an icon is a text prim in the icon face (`Icon::draw`), so the drawing stays rectangles and text. The layout estimate of ADR 0027 still holds: `app/tests/fonts_fit.rs` checks it against the real font (ADR 0045, ADR 0046).
