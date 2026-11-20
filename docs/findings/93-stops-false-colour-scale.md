# 93: Stops False Colour scale and never black

What the ticket (#93) verified, headlessly (`app/tests/stops_false_colour_seam.rs`, the Feed seam with the ColorChecker test frames of #52). Nothing was tested in Resolve or on a real monitor.

- 17 zones with edges at the midpoints between the stop marks. Every one of the 24 patches draws in the colour of the zone its published stops fall in, in ACEScct, ACEScg, DaVinci Intermediate and Rec.709 Gamma 2.4. Neutral 5 (+0.08) is on the 18 % grey zone (a plain grey) and neutral 6.5 (+1.02) in the +1 zone.
- A ramp with a column either side of all 16 edges changes colour exactly at the edges and shows 17 different colours; black below -6 1/2 stops and white above +6 1/2.
- The legend is a bar of 17 swatches in the colours the picture uses (read back from a render), black first and white last. Numbers sit centred under their own swatches (-6 ... +6, -1/2, 18 %, +1/2), captions over them.
- The ColorChecker's dark skin (-0.89 stops) falls in the -1 zone and light skin (+0.89) in the +1 zone; the skin captions are centred over those swatches.
- Narrow Panels: at 300 px every second number from middle grey stays, below about 140 px none, and captions drop one by one rather than overlap. Text widths are estimated (0.6 of the text size per character), not measured from the font.
- A Timeline Colour Space with no curve (or no Colour Setup) no longer draws black: the Output picture is read through Rec.709 Gamma 2.4 (assumed) and every ColorChecker patch measures its published stops to 0.02; the label says approximated, display-referred and assumed. A converted picture is read through the Output Colour Space's curve (P3 as gamma 2.6 checked). Only an Output Colour Space with no curve (HLG, PQ) draws the message, over a neutral background, and it is gone when the Colour Setup changes to one that can be measured.
- That message is reachable only when a Feed is converted without a decodable Timeline Colour Space (DaVinci YRGB tags) and an Output Colour Space with no curve; any other undecodable timeline is shown as sent and read as assumed Rec.709.
- The licensed scheme's name appears nowhere in the app.

Unverified, needing a person (#131): the colours by eye against the reference scale and on a real monitor and HiDPI, and that the text widths estimate fits the real font.

**Checked by the product owner, 2026-11-20 (#131):** the 17 swatch colours, the horizontal scale, numbers, captions and message on a real monitor and HiDPI, and the approximated/assumed labelling on a space the Panel cannot decode all pass; no colour changes needed.
