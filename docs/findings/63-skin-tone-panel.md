# 63: Skin Tone Panel

What the Skin Tone Panel ticket (#63) verified, headlessly (no Resolve involved: the Feed seam, `app/tests/skin_tone_seam.rs`).

- ColorChecker (DaVinci Intermediate chart, Rec.709 Gamma 2.4 output): every pixel of dark skin (-13.8° from the assumed line) and light skin (-16.7°) is coloured, no longer yellow, on the magenta side (a salmon mid-fade). Orange (-2.8°) is yellow, orange yellow (+15.2°) is green-leaning. The six neutral patches are exactly grey, and every patch 30° or more from the line (blue sky, foliage, blue flower, green, cyan, red, and so on) is exactly grey.
- Synthetic hues: on the line is yellow; +8° and +15° are green with the 15° one greener, -8° and -15° are magenta with the 15° one stronger; the green side is never magenta and the other way round. ±35° and further, any hue at 4 thousandths saturation, stay grey. The same hue at luma 0.15 and 0.85 is lit the same, so brightness does not matter.
- The tolerance: a hue 12° off is part-way to green at ±5°, still yellow at ±10°, grey at ±2°.
- The label gives the bands in degrees (±5°, ±20°, ±30° at the default), the skin-tone line as "134.5° (assumed)", the Output picture state and the Colour Setup, and never says the angle matches Resolve.

## Notes

- Mid-fade colours are blends: between yellow and magenta the pixel passes through salmon. See ADR 0021.
- The line was first assumed at 134.5° (ADR 0018); Resolve draws it at 123° ([60](60-skin-tone-line.md)), which moved dark skin to −2.3° and light skin to −5.2° from the line. The figures above are from the first run.

Not verified (a person at the app and Resolve): the look on real footage and skin, the colour directions as a colorist expects, UHD 50 performance: https://github.com/TomZandt/scopecreep/issues/80
