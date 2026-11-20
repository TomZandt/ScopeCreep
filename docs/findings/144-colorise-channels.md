# #144: Colorise on the R, G and B traces, any combination

What ticket #144 showed on 2026-11-18, at the Feed seam (`app/tests/waveform_seam.rs`, headless GPU).

- All 15 Y R G B chip combinations, flat frame with R, G and B at codes 200, 500 and 800: with Colorise off each lit trace is neutral (channels within 3) at its channel's code and an off chip draws nothing; with it on R, G and B are their channel colours at the same codes. Y with Colorise on is the picture's colour, R and G likewise tinted, in one overlay.
- Edge rows: with Colorise on, green over white lights the top row green and blue under black the bottom row blue; with it off the same rows light, neutral. Counts and which rows light are unchanged (#143's test, now with all four chips).
- The tint is a per-channel choice of colour in the draw shader (a flag next to the chip mask); no extra pass, buffer or count. The Colour Waveform keeps its tint and channel colours regardless of the chip.
- **Readability of three overlapping neutral traces (ADR 0032, consequences):** on the ColorChecker frame (Rec.709 Gamma 2.4) with Y R G B all on and Colorise off, the blue, green, red and yellow patches draw each channel as a neutral line at its code, with dark plot between lines more than 30 codes apart, so they read by position. Where channels sit within about 12 codes (the red patch's G and B, 257 and 269) the two lines merge into one band and cannot be told apart without colour; at a coincident value (grey) all four traces stack and brighten, as Resolve's do. That is the expected trade of neutral traces, not unreadable: with Colorise on, the channels separate by colour. No issue to revisit ADR 0032 is filed; whether it reads well to a colourist is part of #150.
- The R, G and B traces' default look changes from coloured (#117) to white, because Colorise is off by default (ADR 0032).

Not verified: the UHD 50 cost with Resolve playing (the per-channel tint adds no pass, so the cost of #143's colour pass is the only open one) and the look beside Resolve: ready-for-human, https://github.com/TomZandt/scopecreep/issues/150

**Checked at Resolve 21 by the product owner, 2026-11-20 (#140):** Resolve's waveform has three exclusive modes (Y, CbCr, RGB). Colorize is one switch inside whichever mode is chosen, and it stays set across all three. This differs from ADR 0032's assumption of any combination of Y R G B; see #202.
