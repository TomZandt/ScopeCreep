# 61: Vectorscopes

What the vectorscope ticket (#61) verified, headlessly (no Resolve involved: the Feed seam, `app/tests/vectorscope_seam.rs`).

- Rec.709 75 % and 100 % colour bars land on their targets: every pixel of each of the six stripes is counted within one bin of the Cb/Cr the BT.709 constants (1.8556, 1.5748) give.
- Each of the first 18 ColorChecker patches (BT.709-encoded, shown as sent) lands within 1 degree of the research table's hue angle. The 1024 x 1024 bins are fine enough for that at the least saturated patches.
- The 200 % Panel shows the same data at twice the radius: the peak bin sits twice as far from the middle (within one bin). A full-strength bar is off the 200 % plot and is not counted.
- The skin-tone line is drawn from the middle at the shared constant's angle and nowhere else, and is gone when turned off. The 75 % or 100 % targets move the boxes to where those bars land.
- Trace brightness (dimmer, normal, brighter) changes how bright a single sparse colour is drawn.
- The label names the Measured Space (or "assumed, measured as sent, unconverted"), the weights (P3-D65 gives 0.2290 / 0.6917 / 0.0793, the same as the Luma waveform), the targets, and the skin-tone angle as assumed. It never says the angle matches Resolve.

## The skin-tone angle is assumed

The line was first drawn at an assumed 134.5 degrees (the traditional I-line rescaled to BT.709 Cb/Cr). Resolve's own vectorscope draws it at 123.3 degrees on screen ([60](60-skin-tone-line.md)), so it is now 123. See ADR 0018.

Not verified (a person at the app and Resolve): the look, comparison with Resolve's vectorscope, UHD 50 performance, very saturated pictures: https://github.com/TomZandt/scopecreep/issues/76

Clipping: pixels beyond the furthest a full-strength Rec.709 bar reaches (0.6 in Cb/Cr; green and magenta bars reach 0.596) are counted by hue angle (720 bins, after the plot's bins) and mark the outer ring, thicker and brighter in their tint at that hue, at either zoom. It is the same at 100 % and 200 %: the 200 % Panel only magnifies, so a colour falling off its edge is not clipping (added after v0.2.0; tests in app/tests/vectorscope_seam.rs).

The outer ring is at 0.6 in Cb/Cr and the plot's edge at 0.66 (first 0.5 and 0.55): the six full-strength bars reach 0.50 to 0.596, so a ring at 0.5 left four of them outside it. The ring is also where clipping starts.
