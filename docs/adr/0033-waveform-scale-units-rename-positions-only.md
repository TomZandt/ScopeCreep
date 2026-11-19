# Waveform scale units rename positions; nits are for PQ and HLG and say what they assume

Spec #113 gives the luma and Colour Waveforms a Scale units setting: 10-bit codes (default), percent, or nits. Colourists reading HDR think in nits, graders of broadcast work in percent, and Resolve's own waveform offers both.

## Decision

- A unit only **renames positions**. The plot is always the 1024 codes of the 10-bit scale, so the counts, the edge rows and where a trace lands never change. The scale's numbers, the reference lines and the words of the edge rows follow the unit. A Feed-seam test holds the counts equal across units.
- **Percent** is of the Levels' range: black 0 %, white 100 % (video: codes 64 and 940; full: 0 and 1023). The edge rows say what codes 1023 and 0 are in that unit (video levels: `> 109.5` and `< -7.3`), so headroom and footroom are visible, not hidden.
- **Nits** are offered only when the Output Colour Space is known and is PQ or HLG. PQ is the ST 2084 EOTF from the accuracy crate (white of the Levels = 10000 nits). HLG is BT.2100 display light for a neutral on a **1000 nit display** (system gamma 1.2); the tag and hover label say so, because HLG has no absolute nits without a display. The top edge row reads `> 10000` (PQ) or `> 1000` (HLG), because anything above code 1023 is above white; nothing past the top of the scale is given a number.
- A remembered "nits" for an output without light is not an error: the scale falls back to codes (no tag) and the choice comes back when a PQ or HLG output does (ADR 0013's rule that a value that is not a choice never reaches a display, kept in the display rather than by forgetting the user's choice).
- The tag names the unit when it is not 10-bit codes (`scale in %`, `scale in nits`); the unit changes how the scale is read, so it is a tag, unlike drawing-only settings.
- The reference lines move with the unit (percent: 0, 25, 50, 75, 100; nits: 0, 10, 100, 1000 and, for PQ, 10000), up to eight, passed to the shader.

## Considered and rejected

- **Nits for every output**, using a nominal peak: the colourist would read light that the picture does not carry (ADR 0007).
- **Changing the plot's axis to the unit** (a log nits axis): the same trace would then land in a different place per unit, and the counts would no longer be the 10-bit counts every other test and Panel agree on.
- **A scene-referred HLG scale** (signal to scene light): not what a colourist asks of a waveform in nits.
