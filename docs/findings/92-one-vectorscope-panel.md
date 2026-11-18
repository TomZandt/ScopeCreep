# #92: one Vectorscope Panel

What ticket #92 showed on 2026-11-18, at the Feed seam (`app/tests/vectorscope_seam.rs`, headless GPU) and the pure layout seam.

- The 100 % and 200 % plots sit side by side in one Panel, each square and centred in its half. One setting (targets, skin-tone line, trace brightness, soft spread) changes both plots, and the rings and skin-tone line fall in the same place on both.
- Clipping is the same on both plots: a colour beyond the furthest full-strength bar marks the ring at either zoom, and the zoom alone never does.
- Settings saved for the 100 % vectorscope carry over; those saved for the 200 % one are ignored.
- The Window has 9 Panels over the same 20 cells, the Vectorscope two cells wide. A layout saved with the two old vectorscope Panels is ignored and the default used, because its vectorscope is missing.
