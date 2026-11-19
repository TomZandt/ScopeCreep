# #162: the Channel view out of the Window, 7 Panels

What ticket #162 showed on 2026-11-19, at the pure layout seam (`app/tests/panel_grid.rs`, `layouts_seam.rs`) and the display seam (`display_seam.rs`). Tests only; nothing was checked in Resolve.

- The Window registers 7 Panels (the Channel view is behind `IN_WINDOW`, like the other Panels that left). `channel_view_seam.rs` builds the displays it needs itself and its tests are otherwise unchanged and pass; its saved settings stay in the settings text.
- The default layout is still 5 x 4 with every cell used and every Panel at least the minimum size at 1280 x 760: ARRI False Colour (3 wide) and Stops False Colour (2) on the third row, the Saturation heatmap (3) and Skin tone (2) on the fourth.
- A layout saved with 8 Panels (naming the Channel view) recalls with the other Panels where they were and the hole closed up (the #98 rule), not as the default.
- The app sets no minimum Window size, so "every Panel is at least the minimum size" is only tested at 1280 x 760, the default size: issue #172.
- Not verified: how the wider Panels look beside Resolve, and the clean picture's channel chips and Loupe against Resolve 21: #75 now covers the clean picture.
