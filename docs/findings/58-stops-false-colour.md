# 58: Stops False Colour

What the Stops False Colour ticket (#58) verified, headlessly (the Feed seam with the synthetic writer, `app/tests/stops_false_colour_seam.rs`). Nothing was tested in Resolve: the frames are the ColorChecker test frames of #52, whose expected values come from the research table.

- Every one of the 24 patches reads its published stops to 0.01 in ACEScct, ACEScg, DaVinci Intermediate and Rec.709 Gamma 2.4; neutral 5 reads +0.08 and neutral 6.5 reads +1.02 in all four.
- Luma is the Y of the colour adapted to D65 (Bradford), as the research table prints it. With AP1's own luma weights, coloured patches in the ACES spaces missed the table by up to 0.022 stops (neutrals were exact), so ACES uses D65-adapted weights (ADR 0019).
- The Rec.709 Gamma 2.4 test chart used to clip cyan's negative red at 0 when encoding; it now mirrors it about zero, as Resolve's pure power curve does and as the cyan luma research convention does, so the view decodes cyan to its published stops.
- Grey and +1 stop: neutral 5 is green, neutral 6.5 pink, and the other grey patches (white 9.5, neutral 8, 3.5, black 2) are grey.
- Full zones: a ramp with a column just either side of every one of the 14 zone boundaries, and one in the middle of each of the 15 zones, shows 15 different colours that change exactly at the boundaries. The zone of middle grey is grey.
- Labels: Rec.709 Gamma 2.4 says "approximated"; the scene-referred spaces say the scene light was decoded from the space; an unknown space or no Colour Setup says "not measured" and draws nothing.
- The licensed scheme's name appears nowhere in `app/src` or in the display's label and menu.
- ACEScc is decoded (published formula) but has no test frame: no ColorChecker chart exists for it yet.
- Unverified, needing Resolve: that the colours read well on a real monitor, and that Resolve's Timeline frames in each space match these curves. #52 and #10 prove the curves against Resolve renders for DaVinci Intermediate and ACEScct only.
