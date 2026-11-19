# #89: threshold sliders on the gamut/legal warning

What ticket #89 showed on 2026-11-19, at the display seam (`app/tests/gamut_warning_seam.rs`), headless.

- The blue threshold is a slider from 0 to 50 % and the red threshold from 50 to 100 %, both in steps of 1; the defaults, 10 % and 90 %, draw and label exactly as the old lists' first choices did. EBU R103 tolerance stays on or off.
- Blue can never be set above red: the two ranges meet at 50 % and do not overlap, so a dragged, saved or hand-edited value is clamped into its own range (a saved blue of 70 reads as 50, a red of 20 as 50).
- The lists an earlier version saved (blue 5, 10, 15, 20; red 80, 85, 90, 95) are plain numbers and read as the number they named; unreadable text is the default.
- Moving a slider moves where the fade starts (red at 80 % tints a 85 % grey a quarter of the way, blue at 20 % tints a 15 % grey the same). The label names both values and the codes they give for the project's Levels (video: 10 % is code 152, 90 % is 852; data: 102 and 921).
- Not verified: how the sliders feel in the real Window, covered by #91.
