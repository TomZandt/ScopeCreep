# #87: trace brightness slider on the CIE plot

What ticket #87 showed on 2026-11-19, at the display seam (`cie_seam.rs`), headless.

- Brightness is 20 to 400 % in steps of 5, default 100 %, the same scale as the vectorscope. The gain is the number over 100, so the default draws exactly as the old Normal; saved `normal`, `bright` and `dim` are 100, 400 and 25 and give the old gains 1, 4 and 0.25. Out of range is clamped, junk is 100.
- Brightness does not change the label. The Timeline triangle stays a list.
- Not verified: how it looks and drags in the real Window and on HiDPI (#127).
