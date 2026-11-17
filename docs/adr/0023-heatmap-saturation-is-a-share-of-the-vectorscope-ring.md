# The saturation heatmap's 100 % is the vectorscope's ring (Cb/Cr 0.5), not each colour's own bar

Spec #49 asks for 100 % on the heatmap to be "the vectorscope's 100 % target". A vectorscope has six 100 % targets, and they are not at one distance from the middle: BT.709 100 % bars sit at Cb/Cr distances of 0.502 (yellow, blue), 0.513 (red, cyan) and 0.596 (green, magenta).

## Decision

- Saturation is `sqrt(Cb² + Cr²)` from the Measured Space's luma weights (the same Cb/Cr as the vectorscope), as a percentage of the vectorscope's outer ring, `RING` = 0.5, so 100 % is the ring the vectorscope draws.
- 100 % colour bars therefore read 100 to 103 % (red, cyan, yellow, blue) and 119 % (green, magenta). At the default scale everything from the ring outwards is the warm end, so all six bars are at the "100 % point".
- The scale setting is the percentage at the warm end (100 % by default; 25, 50, 75, 150, 200 %). Pixels past it stay at the warm end.
- The palette (7 muted stops of one Rec.709 luma, cool blue to warm red) lives in `app/src/muted_palette.rs` so the Skin Tone Panel can share it.

## Considered and rejected

- **Each bar's own distance as 100 %** (a hue-dependent scale): the same saturation would read differently at different hues, and no longer match the vectorscope's rings.
- **HSV or HSL saturation:** a different measure from the vectorscope, so the two Panels would disagree about the same pixel.

## Consequences

- A pixel's percentage is the vectorscope's radius for it; it is read the same way in both Panels.
- Pixels in the Output picture past the ring (green and magenta bars, wide-gamut colours) are all drawn at the warm end at the default scale; the 150 % and 200 % scales separate them.
