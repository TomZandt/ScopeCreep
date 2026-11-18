# #145: the Colour Waveform out of the Window, 8 Panels

What ticket #145 showed on 2026-11-18, at the pure layout seam (`app/tests/panel_grid.rs`, `layouts_seam.rs`) and the display seam (`display_seam.rs`).

- The Window registers 8 Panels (Colour Waveform, like the gamut warning, Highlight Neutrals, CIE plot and Black and white, is behind `IN_WINDOW`). Its seam tests (`colour_waveform_seam.rs`) build the displays they need themselves and pass unchanged otherwise; its saved settings stay in the settings text.
- The default layout is still 5 x 4 with every cell used and every Panel at least the minimum size at 1280 x 760: the Channel view takes the 2 x 2 the Colour Waveform left and ARRI False Colour spans three columns beside it.
- A layout saved with 9 Panels (naming the Colour Waveform) recalls with the other Panels where they were and the hole closed up (the #98 rule), not as the default.
- Not verified: how the 2 x 2 Channel view and the wide ARRI False Colour Panel look beside Resolve: part of #150.
