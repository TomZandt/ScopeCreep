# #124: Vectorscope tonal range

What ticket #124 showed on 2026-11-19, at the pure UI seam (`app/tests/toolbar.rs`) and the Feed seam (`app/tests/vectorscope_seam.rs`, headless GPU).

- **The ranges and their source.** Shadows are Luma below 0.30, Mids from 0.30 up to below 0.70, Highlights 0.70 and above (anything above white included; anything below black is Shadows). These are the defaults of Resolve's own vectorscope Low (0.30) and High (0.70) boundaries (Resolve 20 manual p. 3020, via `docs/research/panel-controls.md` section 4). The manual states them for its tonal range control, so they are taken from it rather than picked as thirds of the legal range. Luma is Y of the Feed's full-range values (0 black, 1 white) with the Measured Space's luma weights, the same Y the waveform uses, so the Levels do not move the boundaries. They are fixed: Resolve lets them be adjusted, but an adjustable boundary is one more way to make a reading look different without the label saying so (ADR 0013).
- A frame with plain and overdriven colours in each range: each chip plots only its own range's plain colour (every pixel of it, at 100 % and 200 %) and none of the other two ranges' colours; All plots all three. Back to All is the same measurement as before.
- Clipping on the ring counts only the plotted pixels: with each range chosen the clipped total is the overdriven pixels of that range alone; All counts all of them. A Luma that is not a number is in no range, so it is not plotted under any chip (under All it behaves as before).
- A change of range changes what is measured, so the Feed measures again: the test plays the frame again after each choice, as the app does when settings change.
- Tags: "shadows only", "mids only" or "highlights only" when not All; the title bar's hover label spells out the three ranges and says clipping counts only these pixels. All is not tagged. The choice round-trips through the saved settings per Panel; the chips are All, Shadows, Mids and Highlights, one at a time, after the plots and Mono chips.

Unverified against Resolve 21.2 itself: [#155](https://github.com/TomZandt/scopecreep/issues/155).

**Checked against Resolve 21.2 by the product owner, 2026-11-20 (#155):** Resolve's Low, Mid and High vectorscope channels plot differently from scopecreep's Shadows, Mids and Highlights chips (30 % and 70 %). The product owner prefers scopecreep's, so they are not made to match.
