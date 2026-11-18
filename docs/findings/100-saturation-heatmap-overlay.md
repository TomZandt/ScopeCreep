# 100: Saturation heatmap keeps the picture's shape

What ticket #100 verified headlessly (the Feed seam, `app/tests/saturation_heatmap_seam.rs`).

- The heatmap draws each pixel as its Luma in grey with the saturation colour laid over it, so the picture stays readable. The six ColorChecker greys draw as their Luma grey (within 8/255; the white patch is 1 to 2 % off neutral and gets a barely visible tint, which the eased strength curve keeps small).
- Every ColorChecker patch draws as `overlay(Luma, share of scale)`, with the share from the BT.709 constants; 100 % and 75 % colour bars the same (strong colour).
- Default scale: the 18 colour patches as the Output picture sit at 14 to 67 % of a 100 % scale, so 100 % left a third of it unused. At 75 % they sit at 19 to 90 % (median about 50 %, the most saturated patch, orange, 90 %), so 75 % is the default. Recorded in ADR 0023.
- The palette is 6 stops for contrast (blue, cyan, green, yellow, orange, red); the even-brightness rule is dropped. Strength is 0 at 0 % and full from 30 % of the scale, eased in.
- The legend is the same overlay over mid grey, so its colours match the picture's.

Not verified (a person at the app and Resolve): how the look reads on real graded footage.
