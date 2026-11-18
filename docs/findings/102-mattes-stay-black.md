# #102: mattes stay black

What ticket #102 showed on 2026-11-18, at the Feed seam (`app/tests/mattes_seam.rs`, a GPU, no Resolve).

- A letterboxed frame (12 rows black at the top, 6 at the bottom) and a pillarboxed one (20 and 30 columns) draw black bars and a coloured picture, up to the bar's edge, on ARRI False Colour, Stops False Colour, Skin Tone and the saturation heatmap.
- A crushed black band inside a letterboxed picture is still purple on ARRI False Colour (the one of the four that colours black); only the edge bars are black.
- A frame with no bars is drawn whole, and an all-black frame is not treated as a matte.
- Stops False Colour, Skin Tone and the heatmap leave black black anyway, so for them the bars are a guard rather than a change.
- The bars are exact: all of red, green and blue 0.0. Bars with noise are not found (see ADR 0029).

Not verified (a person at Resolve): that the GPU job costs playback nothing at UHD 50 with real letterboxed media.
