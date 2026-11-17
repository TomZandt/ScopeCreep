# 50: Panel grid

What the grid ticket (#50) verified, headlessly (no Resolve involved: the Feed seam and the pure layout seam).

- The grid fills the Window for 1 to 20 displays; 2 displays sit side by side, 14 sit in rows of 5, 5 and 4 (`app/tests/panel_grid.rs`).
- Dragging a border moves only the two Panels beside it, never below the minimum size; positions round-trip through the saved text, and positions for another display count or rubbish are ignored.
- Labels keep their full height at the smallest Panel size at scales 1 and 2.
- Two displays registered beside the built-in ones get the same frames (one texture, uploaded into again), the Colour Setup and a label slot; the waveform counted every pixel of that one frame (`app/tests/display_seam.rs`).
- Each display draws only inside its own Panel; the waveform draws at a scale of 2 with its gutter kept clear.
- The app starts and runs with the grid without error.

Not verified (a person at the app and Resolve): dragging with a real mouse, HiDPI look, UHD 50 performance: https://github.com/TomZandt/scopecreep/issues/68
