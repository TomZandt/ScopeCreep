# #117: the luma waveform draws any of Y R G B

What ticket #117 showed on 2026-11-18, at the Feed seam (`app/tests/waveform_seam.rs`, GPU) and the pure toolbar seam (`app/tests/toolbar.rs`).

- R, G and B are counted from the same frame as Luma, at the same Levels, in one extra compute pass (one workgroup row per channel). On synthetic frames each channel's counts land at that channel's code in every column, and every pixel is counted once per channel.
- Out of range is counted per channel: red over white, green under black and an in-range blue each show in their own channel's above and below counts. Drawn, the edge rows light in the channel's colour (red in red), and stay dark where nothing is out of range.
- The chips overlay on one scale, each trace in its channel colour at its own phosphor brightness, added together (overlapping R, G and B tend to white). Luma keeps its neutral trace colour. With only Y on, the drawing and the counts are unchanged.
- At least one chip is always on (a click on the last chip on does nothing); Luma is the default; the choice is stored as the letters of the chips that are on ("yrb") in the Panel's settings, so it is remembered per Panel. A stored value with no chip on falls back to Luma.
- The tags name the channels when not just Luma ("R G"), and the label drops the luma weights when Y is off. The Colour Waveform is unchanged: it follows Luma only.
- Cost: Luma is always measured (the Colour Waveform shares its counts). The R, G and B counts (47 MB at UHD) exist and are measured only while one of their chips is on, and are freed when all three go off.

Not verified: the UHD 50 cost of the extra pass with Resolve playing (the GPU was full): ready-for-human, https://github.com/TomZandt/scopecreep/issues/138
