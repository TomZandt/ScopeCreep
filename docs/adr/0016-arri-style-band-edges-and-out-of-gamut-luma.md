# ARRI-style bands: Luma is taken as it is, lower edges are in, and the ends are open

Ticket #57 turned the band table of ADR 0010 into a view. Three details of that view are choices, not facts from a source, and they are hard to see afterwards.

## Decision

- **Luma is the weighted sum of the Output picture's own channel values**, unclamped, with the weights of the Measured Space (ADR 0004), as a percentage of the full-range signal. It is the number the luma waveform traces. A channel below 0 or above 1 counts as the number it is. The view neither clamps it nor mirrors it: what a negative channel holds is decided by the Output Conversion.
- **Each band includes its lower edge and excludes its upper edge**, as ARRI's own specification does. 38.0 % is green and 42.0 % is grey.
- **The two ends are open:** Luma below 0 % is purple (crushed past black) and 100 % or more is red (clipped). Read literally, the table would leave a pixel clipped at exactly 100 % grey, which is the one pixel the red band exists to flag.

## Why

- Using the picture's own values keeps this view, the waveform and the gamut warning agreeing about the same pixel, with no second rule for out-of-gamut colours.
- The research's cyan (34.56 %) mirrors the BT.709 OETF about zero for its negative red; clamping gives 38.18 %, which would put cyan in green. That mirrored value is a convention of the research (#52), not proven against Resolve. The view does not decide it either way: given the research's picture it shows cyan in grey, and given a clamped picture it would show green. If Resolve's own output for cyan is ever measured, only the Output Conversion would change.

## Considered and rejected

- **Clamping each channel to 0..1 first:** it hides out-of-gamut colours from Luma and, for cyan, moves it into the green band.
- **Closed upper edges (the band includes 100 %):** two bands would claim every shared edge.

## Consequences

- The view's percentages are of the full-range signal whatever the project's Levels, as the research table's are.
- Open ends are a small departure from the published table; the label does not claim the table is ARRI's (ADR 0010).
