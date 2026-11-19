# #90: scale slider on the saturation heatmap

What ticket #90 showed on 2026-11-19, at the Feed seam (`saturation_heatmap_seam.rs`, a synthetic orange frame and the ColorChecker) and the title seam (`window_text.rs`), headless.

- The scale is a slider, 25 to 200 % in steps of 5, number and unit " %". The default is 75 %, today's default (ADR 0023): it draws exactly as before, and the most saturated ColorChecker patch still uses 70 to 100 % of it. (Spec #83 says 100 % in one place; the code and ADR 0023 say 75 %, and "the default draws as today" decided it.)
- The lists earlier versions saved (25, 50, 75, 100, 150, 200) are plain numbers, so they are read as they are, with no mapping needed; unreadable is 75 %, out of the range is clamped (1000 reads as 200 %).
- A value between the old list's values (35, 85, 120 %) puts the orange at its share of that scale; a smaller scale is warmer, a larger one cooler.
- The label and the title tag say "hot N %" in the slider's own number; the tag is absent at the default. The legend stays on or off and its top number follows the slider (same state).
- Not verified: how dragging feels in the real Window (#91).
