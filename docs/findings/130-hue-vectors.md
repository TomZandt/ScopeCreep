# #130: Hue vectors graticule on the Vectorscope

What ticket #130 (spec #192) showed on 2026-11-20, at the Feed seam (`vectorscope_seam.rs`, `toolbar.rs`), headless.

- The Vectorscope's popover has a "Graticule" choice, Standard (the default) and Hue vectors, stored per Panel under the key `graticule-style` (`standard`, `hue-vectors`). It sits after the Graticule brightness slider. An unknown or missing value is Standard, so a file from before the choice draws as it did (the Standard render is byte for byte the render before the choice).
- Hue vectors adds a spoke from the middle to each of the six colour targets (red, yellow, green, cyan, magenta, blue), along the angle of the target's boxes, ending at the further box of the pair (the 100 % one by default), so each runs through its own box. Neutral grey, the same grey and width as the axes and ring, scaled by Graticule brightness (the spoke brightens and dims with the slider).
- Both plots of the joined 100 % and 200 % pair draw the spokes (the choice is one setting), at the same angles.
- Nothing else changes: the trace pixels at a saturated patch are identical with and without spokes, and the skin-tone line is unchanged. It is a drawing choice, so the label and the tags do not change.
- Not verified: how the spokes look on a real monitor and at HiDPI, and beside Resolve's own hue-vector graticule: #199. The skin-tone line (123 degrees) lies about 19 degrees from the red spoke (about 104 degrees), so the two read as separate lines but are close; #199 asks a person to check.
