# ColorChecker test frames (ticket #52)

Tests can now feed a ColorChecker Classic frame through the synthetic Feed writer in ACEScct, ACEScg, DaVinci Wide Gamut / Intermediate or Rec.709 Gamma 2.4, and know what every patch should read. Nothing here was tested in Resolve: the frames are built from published data and checked against the desk research and the app's own Feed seam.

## What was built

- `accuracy/src/colorchecker.rs`: `ColorChecker::new(TimelineSpace)` gives the 24 patches (linear Y, stops from 18 %, linear and encoded RGB, Rec.709 luma, ARRI-style band, hue, out-of-gamut channels) and `frame(width, height)` paints the 6 x 4 chart as a Feed-ready picture (RGBA f32, bottom row first), with `patch_rect(n)` for the layout. Data: X-Rite ColorChecker24 after November 2014 (D50 L\*a\*b\*), Bradford adaptation.
- `accuracy/tests/colorchecker.rs`: every patch against the research table (`docs/research/colorchecker-reference-values.md`) to its printed precision.
- `app/tests/colorchecker_seam.rs`: an ACEScct chart (ACES 2.0, Rec.709 BT.1886) and a DaVinci Intermediate chart (Rec.709 Gamma 2.4) through the Feed, the app's Output Conversion and the waveform. Every pixel of each of the six grey patches is traced within one code of where the f64 reference puts it.

## What the check showed

- **Y is the colour after adaptation to D65,** not the raw Y of the L\*a\*b\* value: the two agree for neutrals and differ for coloured patches (dark skin is 0.0971, not 0.0983). The research table prints the adapted Y.
- **The D50 white is the ICC one (0.9642, 1, 0.8249).** With CIE's 0.96422 / 0.82521 a few printed values (ACEScct of patches 8, 15 and 18, the hue of patch 16) fall a hair outside the table.
- **Cyan's luma (34.56 %) needs the BT.709 OETF mirrored about zero** for its negative red (linear -0.044). Clamping the red at 0 gives 38.18 %, a straight extension of the linear segment 33.98 %. This is the research's convention, not proven against Resolve; it is noted on ticket #57 (ARRI False Colour) so it is settled when out-of-gamut channels meet the bands.
- The grey patches trace at the Output Conversion's expected code in both spaces, so the Feed seam, the Output Conversion and the waveform agree with the chart code.
- The "Rec.709 Gamma 2.4" chart encodes with a pure 2.4 power, as Resolve's space of that name does (see the accuracy suite).
