# Chrome draws only through style tokens named by role; measurement colours are data

The Window's chrome (the menu, labels, scale numbers, status line, borders and gaps) each picked its own grey and size in whatever file drew it, so the Window looked assembled and every new Panel would drift further (spec #113).

## Decision

- One module, `app/src/style.rs`, holds the tokens, named by **role** (`SURFACE_MENU`, `TEXT_SECONDARY`, `TYPE_LABEL`, `SPACE_MARGIN`, `LINE_BORDER`, `STATE_HOVER`) and not by value. Surfaces, lines, text, states, the type scale (notice, label, scale numbers), spacing, line weight and corner radius. Dark and neutral only: every chrome colour has equal red, green and blue, so nothing tints the colourist's eye.
- Token values start from what the Window already used, so the move changes nothing visible. Existing constants elsewhere (`layout::MARGIN`, `settings::MENU_ROW`, `panel_grid::LABEL_SIZE`, `waveform::SCALE_SIZE`) now alias a token, so call sites and tests stay.
- **Measurement colours are data, not style.** False colour bands, the heatmap palette, skin tints, trace colours and a legend's bars live with their displays and are exempt by rule: a band is never "corrected" to a theme colour.
- **The check** (`app/tests/style_check.rs`) scans the chrome files (`main.rs`, `settings.rs`, `panel_grid.rs`, `layout.rs`, `flat_rects.rs`) and fails on a raw colour (`Color::rgb(..)`, `wgpu::Color { .. }`, an array of three or four number literals) or a raw size (`const X: f32 = 12.0`, `size: 12.5`, `Metrics::new(12.0, ..)`). It was seen failing on planted cases. Display modules are not in the list; a new chrome file must be added to it.

## Considered and rejected

- **A lint over every file:** displays hold hundreds of measurement literals; the exemption would need a marker on each. A list of chrome files is short and explicit.
- **Tokens in a theme file loaded at run time:** there is one dark theme (spec #113, out of scope: user themes), so a constant is simpler and checked by the compiler.

## Consequences

- New components (title bar, chips, slider, popover, tooltip; #115 onward) take their colours and sizes from `style.rs` and are added to the check's list.
- Displays' own plot greys (grid, marks) are still local; moving the ones that are chrome rather than measurement is a follow-up.
