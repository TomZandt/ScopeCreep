# 57: ARRI False Colour

What the ARRI False Colour ticket (#57) verified, headlessly (no Resolve involved: the Feed seam, with the ColorChecker frames of #52).

- Every one of the 24 ColorChecker patches, given as its linear Rec.709 light through the BT.709 OETF (mirrored about zero for cyan's negative red), is drawn in the band the research table gives, or in the grey of its Luma where the table says none. Blue sky is green; neutral 5 (42.23 %) and neutral 6.5 (59.92 %) are grey (`app/tests/arri_false_colour.rs`).
- Band edges, on a synthetic ramp: a hundredth of a percent either side of each edge lands in the band on that side. An exact edge cannot be hit on the GPU in 32-bit float, so the inclusive lower and exclusive upper edge are tested on the band table's own rule (`band_at`), which the shader's code is generated from.
- The legend toggles from the Panel's right-click menu (on by default): six swatches under the picture with the band edges as text, or none. A Panel too short for it (under 110 logical pixels) shows the whole picture without it.
- The label reads "ARRI-style False Colour", names the Luma weights and Measured Space (with "assumed" until the Colour Setup is known), and says the bands are Rec.709 percentages published by third parties, not ARRI's specification and not affiliated with ARRI.

## Decisions it settled

- **Cyan's Luma (the open question from #52).** The view takes the picture's channel values as they are (ADR 0016): with the research's mirrored picture cyan is 34.56 % and grey. `accuracy/src/colorchecker.rs` is unchanged. Whether Resolve's own Output for an out-of-gamut colour matches that convention is not tested.
- **Edges.** Lower edge in, upper edge out; the first band is open below and the last open above (ADR 0016).

Not verified (a person at the app and Resolve): the look in the real app, the ColorChecker clip in the product owner's own project, and what an out-of-gamut channel looks like after the Output Conversion: https://github.com/TomZandt/scopecreep/issues/71
