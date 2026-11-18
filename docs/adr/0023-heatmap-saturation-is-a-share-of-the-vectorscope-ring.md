# The saturation heatmap's 100 % is the vectorscope's ring (Cb/Cr 0.5), not each colour's own bar

Spec #49 asks for 100 % on the heatmap to be "the vectorscope's 100 % target". A vectorscope has six 100 % targets, and they are not at one distance from the middle: BT.709 100 % bars sit at Cb/Cr distances of 0.502 (yellow, blue), 0.513 (red, cyan) and 0.596 (green, magenta).

## Decision

- Saturation is `sqrt(Cb² + Cr²)` from the Measured Space's luma weights (the same Cb/Cr as the vectorscope), as a percentage of the vectorscope's outer ring, `RING` = 0.5, so 100 % is the ring the vectorscope draws.
- 100 % colour bars therefore read 100 to 103 % (red, cyan, yellow, blue) and 119 % (green, magenta). At the default scale everything from the ring outwards is the warm end, so all six bars are at the "100 % point".
- The scale setting is the percentage at the warm end (100 % by default; 25, 50, 75, 150, 200 %). Pixels past it stay at the warm end.
- The palette lives in `app/src/muted_palette.rs` (see the 2026-11-18 update: grey picture with a contrast palette laid over it).

## Considered and rejected

- **Each bar's own distance as 100 %** (a hue-dependent scale): the same saturation would read differently at different hues, and no longer match the vectorscope's rings.
- **HSV or HSL saturation:** a different measure from the vectorscope, so the two Panels would disagree about the same pixel.

## Consequences

- A pixel's percentage is the vectorscope's radius for it; it is read the same way in both Panels.
- Pixels in the Output picture past the ring (green and magenta bars, wide-gamut colours) are all drawn at the warm end at the default scale; the 150 % and 200 % scales separate them.

## Update 2026-11-17

The vectorscope ring moved from 0.5 to 0.6 Cb/Cr, the furthest a full-strength Rec.709 bar reaches, so that every legal bar is inside it. The heatmap follows the ring, so 100 % is now 0.6: green and magenta full-strength bars read 99 %, the other four 84 to 86 %, and nothing legal is over 100 %.

## Update 2026-11-18 (#100)

The heatmap now keeps the picture's shape: each pixel is its Luma in grey (the Measured Space's weights) with the saturation colour laid over it, the colour's strength rising smoothly (smoothstep) from none at 0 % to full at 30 % of the scale. Neutral areas show the grey picture; saturated areas a strong colour. The even-brightness palette rule (spec #49 story 78) is dropped: the palette is 6 stops chosen for contrast, blue, cyan, green, yellow, orange, red, cool to hot.

The default scale is 75 % (was 100 %). Measured on the ColorChecker as the Output picture, the 18 colour patches sit at 19 to 90 % of a 75 % scale (the most saturated, orange, at 90 %; the median near 50 %), so a normal graded picture uses most of the scale. At 100 % the most saturated patch used only 67 %. The measure itself (share of the vectorscope's ring) is unchanged, so 100 % is still the ring. The legend is the same overlay laid over mid grey, so its colours match the picture's.
