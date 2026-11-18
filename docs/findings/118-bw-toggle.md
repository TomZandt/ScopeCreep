# #118: B/W toggle, and the Black and white Panel out of the Window

What ticket #118 showed on 2026-11-18, at the Feed seam (`app/tests/bw_seam.rs`, headless GPU) and the pure UI seam (`app/tests/toolbar.rs`).

- The clean picture is in colour until B/W is on, then Luma grey. ARRI False Colour and Stops False Colour (grey and +1 stop mode) are grey until toggled to colour; their bands and marks do not change either way.
- The toggle is a chip on the title bar, remembered per Panel; the tag says "B/W" or "colour" only when it is off its default. The saturation heatmap has none.
- The Black and white Panel is out of the Window (the Window has 9 Panels); its code and tests stay.
