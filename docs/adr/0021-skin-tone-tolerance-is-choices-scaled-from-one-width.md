# The Skin Tone Panel's tolerance is a list of choices that scale one width, and the tints fade through each other

_Tint and fade rules superseded by [ADR 0025](0025-skin-tone-panel-paints-solid-colours-on-the-natural-picture.md); the list of choices stands._

Spec #49 asks for "a tolerance slider" with about ±5° on the line, fading out to ±20° and gone beyond ±30°. ADR 0013 says every setting is a short list of named choices, never a free number.

_Update 2026-11-19 (#88): the tolerance is now a slider from 1 to 12 degrees (default 5) under ADR 0013; the bands still scale from it at the ratios below._

## Decision

- The setting is one list of choices for the on-line half-width `t`: ±5° (default), ±2°, ±3°, ±8°, ±10°. The other two edges follow from it at fixed ratios: the colour is fully green or magenta at 4 t and gone (grey) at 6 t. At the default they are the spec's 20° and 30°.
- Between `t` and 4 t the yellow fades into green (hue above the line, towards yellow) or magenta (below it, towards red), so strength grows with distance; between 4 t and 6 t that colour fades back to grey. A pixel with almost no saturation (Cb/Cr length under about 0.01, fully on from 0.03) is grey whatever its hue.
- Hue is the angle of Cb/Cr with the vectorscope's luma weights, measured from the shared `SKIN_TONE_LINE_DEGREES` (ADR 0018). The label gives the bands and says the angle is assumed, and never that it matches Resolve.
- The three tints are muted and of similar brightness, and take the grey's lightness so the picture stays readable.

## Considered and rejected

- **A slider:** ADR 0013 ruled out free numbers and the menu had no slider. Since 2026-11-19 (#88) ADR 0013 allows amounts and the tolerance is a slider (see the update above); this option was rejected under the earlier rule.
- **Three independent settings (on, fade, cut-off):** three menus for one idea; a user who wants a wider band wants the whole shape wider.
- **Ratios to the line only from the spec's numbers at ±5° (fixed 20° and 30°):** widening the on-line band to ±10° would leave only 10° of fade.

## Consequences

- Mid-fade colours are blends (a salmon between yellow and magenta), not the pure tints.
- The edges at other choices are multiples of `t` (for example ±8° gives 32° and 48°); the label states them.
