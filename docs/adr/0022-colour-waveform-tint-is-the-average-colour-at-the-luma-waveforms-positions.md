# The Colour Waveform is the luma waveform's counts with a tint: the average colour of the pixels counted at each spot

## Decision

- The Colour Waveform is the same display as the luma waveform (same measurement, plot, scale, Levels lines, phosphor response and soft spread), made with a flag. Its positions and counts are the luma waveform's by construction: one `row_of` function in the shader decides a pixel's row for both the count pass and the colour pass.
- A second compute pass adds each pixel's colour (clamped to 0 to 1, in 1/1023ths, 32-bit integers) into three sums per count, after clearing them. A spot's tint is its colour sum divided by its count, averaged over the picture columns and codes under the screen pixel.
- The tint is brought to the neutral trace's strength: its strongest channel equals the neutral trace level. A grey scene therefore draws exactly the luma trace, and a dark green is still visible (its darkness is already shown by where it sits). The brightness still comes from density.
- The tint is drawn into the same trace texture as the brightness (RGBA16F: colour times brightness, and brightness), so the soft spread blurs both together.
- The Colour Waveform has no luma weights setting. It uses the luma waveform's choice, as the other Luma displays do (ADR 0013), so a trace never sits at a different code from the luma waveform's.
- Right-click settings: trace brightness and soft spread, the same as the luma waveform.
- The label says "Colour waveform" and names the Levels, Measured Space and weights like the luma waveform's.

## Considered and rejected

- **Plotting by a colour-space axis (a parade or RGB overlay):** that is a different Scope.
- **Showing the average colour at its own brightness:** dark traces would nearly vanish against the plot.
- **Sums in the single count pass:** the count pass already fills workgroup memory with the counts; adding three more sums per count would not fit the 16 KB limit.

## Consequences

- The colour pass uses global atomics, so a frame with very many pixels on one code may cost more than the count pass; UHD 50 with every Panel is checked by hand (#68).
- Pixels outside 0 to 1 tint as if clamped: their position is still honest (edge rows), their colour is not extrapolated.

## Note, 2026-11-18

The tint rule now also serves the luma waveform's Colorise chip (#143, ADR 0032): the Y trace takes the same average colour at each position. The Colour Waveform Panel keeps its code.

## Note, 2026-11-18 (#145)

The Colour Waveform Panel is out of the Window (ADR 0032): Colorise on the luma waveform gives the same picture on the one Scope. The module, its tests and its saved settings are kept, and a saved layout that names it still recalls. The tint rule above is unchanged.
