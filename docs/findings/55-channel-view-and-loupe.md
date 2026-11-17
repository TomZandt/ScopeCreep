# 55: Channel view and Loupe

What the Channel view ticket (#55) verified, headlessly (no Resolve involved: the Feed seam, `app/tests/channel_view_seam.rs`).

- A frame with different R, G and B (0.4, 0.6, 0.8) draws exactly the chosen channel as grey (102, 153, 204 of 255). Blue is the default. The label says which channel ("Blue channel as grey ...") and the Loupe's zoom.
- Inside the Loupe at 200 %, every complete run of equal pixels across and down is exactly 2 device pixels long (a frame in which every neighbouring pixel differs); at 400 % it is 4, at 100 % 1.
- Scroll steps the zoom in whole 100 % steps and stops at 100 % and 800 %; it acts only over the Loupe.
- Dragging moves the Loupe exactly as far as the pointer moves; it is held wholly inside the Panel however far the pointer goes; the magnified area follows (dark on the left of a split frame, bright on the right).
- Small, medium and large Loupes are 100, 180 and 300 logical pixels, never more than the Panel.
- Size (a menu choice) and position and zoom (state, ADR 0015) come back in the next session; rubbish in the file falls back to the defaults.

Not verified (a person at the app and Resolve): dragging and scrolling with a real mouse and trackpad (the window code only forwards pointer events), the look at HiDPI, and the noise view on real Resolve footage: https://github.com/TomZandt/scopecreep/issues/70
