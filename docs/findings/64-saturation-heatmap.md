# 64: Saturation heatmap

What the saturation heatmap ticket (#64) verified, headlessly (no Resolve involved: the Feed seam, `app/tests/saturation_heatmap_seam.rs`).

- Every ColorChecker patch (Rec.709, BT.709-encoded, out-of-gamut cyan mirrored about zero as in the research) is drawn at its Cb/Cr distance from neutral, as a share of the vectorscope's 0.5 ring, to within 1 % of the scale. The distances come from the BT.709 constants (1.8556, 1.5748), not from the shader.
- The six grey patches are at the cool end, within 3 % of the scale (the white patch is 1 to 2 % off neutral after the research table's D50 to D65 adaptation and rounding).
- 100 % colour bars: red, cyan, yellow and blue read 100 to 103 %, green and magenta 119 % (they sit further out on a vectorscope too), so all six are at the warm end at the default scale; 75 % bars read 75 % of those (ADR 0023).
- The scale (25, 50, 75, 100, 150, 200 %) remaps the same pixel to the matching place on the palette; the label names the scale.
- The legend (a cool to warm bar under the picture, with 0 % and the scale's top as text) toggles; with it off the picture takes the whole Panel.
- The palette's stops share one Rec.709 luma (0.56, within 0.02 across the whole scale).
- The label says "Output picture", the Measured Space (or "Rec.709 assumed, unconverted") and the weights, and defines 100 %.

Not verified (a person at the app and Resolve): the look of the palette on real pictures, comparison with Resolve's vectorscope, the default scale on real footage, and UHD 50 performance: https://github.com/TomZandt/scopecreep/issues/81
