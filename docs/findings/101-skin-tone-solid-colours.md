# 101: Skin Tone colours strong enough to read at a glance

Proven at the Feed seam (`app/tests/skin_tone_seam.rs`), not at Resolve:

- The ColorChecker's dark skin (2 degrees below the line) is solid yellow; light skin (5 degrees below, the edge of the on-line band) is solid yellow or magenta; orange (9 degrees above) is solid green.
- Synthetic hues 8, 15 and 28 degrees either side are solid green and solid magenta; 35 degrees and more keep their own colour.
- Blue sky and the neutral patches are not painted; a near-neutral pixel on the line is not painted.
- The tolerance choices move the bands (yellow within t, green or magenta to 6 t) as before.

Decision: ADR 0025. Nothing here needs a person at Resolve.
