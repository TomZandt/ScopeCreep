# #143: a Colorise chip tints the luma waveform's Y trace

What ticket #143 showed on 2026-11-18, at the Feed seam (`app/tests/waveform_seam.rs`, headless GPU) and the pure toolbar seam (`app/tests/toolbar.rs`).

- On a flat colour frame (0.2, 0.8, 0.3) the Y trace at the colour's Luma code is that colour (red about 25 % and blue about 37.5 % of green) with Colorise on and plain white (channels within 3 of 255) with it off; the rest of the plot stays dark.
- On a ColorChecker frame (Rec.709 Gamma 2.4 chart, `accuracy` crate), the Y trace at the middle of the blue, green, red and yellow patches, at each patch's Luma code, is that patch's colour (proportions within 0.1) with Colorise on and neutral with it off.
- Counts (Luma, R, G and B) and the edge rows are identical either way: the same edge rows light for an out-of-range column. Colorise does not change the label.
- The Colorise chip is off by default, toggles each way, is remembered per Panel with the Y R G B choice, is never a tag, and is not a popover row. Hovering names it (the chip says "Col" because the full word does not fit beside Y R G B: it is 77 logical pixels wide, "Col" about 38). With the five chips, the settings button and the name, a 180 logical pixel wide Panel keeps every chip inside the title bar and cuts the name first.
- The tint pass runs only while Colorise is on and Y is showing; the 47 MB sums buffer exists only then and is freed when Colorise goes off.
- R, G and B traces are unchanged by this ticket (always their channel colours); #144 makes them follow Colorise. The readability of three overlapping neutral traces (ADR 0032, consequences) is therefore not yet shown.

Not verified: the UHD 50 cost of the tint pass with Resolve playing, the look beside Resolve, and the title bar at the smallest Panel (120 pixels): ready-for-human, https://github.com/TomZandt/scopecreep/issues/150
