# #151: the luma waveform keeps the footage's aspect ratio

What ticket #151 showed on 2026-11-19, at the pure layout seam (`app/tests/layout.rs`, `PlotLayout::fitted`) and the drawing seam (`app/tests/waveform_seam.rs`).

- For 16:9, 2.39:1, 4:3 and 9:16 frames, in wide, tall and offset Panels, the plot's width and height have the frame's ratio within a device pixel; the gutter and the plot are centred as one block and the plot and its two edge rows as one stack, all inside the Panel.
- A different resolution gives a different layout on the next draw, with nothing drawn outside the fitted plot and its edge rows.
- Learned: the seam tests that read the plot by code row had assumed it filled its Panel; a plot of exactly 1024 pixels (one row per code) now needs a frame and Panel chosen to fit (a 400 x 1024 frame in a 500 x 1046 area). Counts, traces, edge rows and Colorise tests pass unchanged apart from taking the frame size in the layout.
- Not verified: the look on small Panels, HiDPI and the shorter plot at 16:9 and 2.39:1 at Resolve: #152.
