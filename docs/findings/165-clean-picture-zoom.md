# #165: clean picture zoom

What ticket #165 showed on 2026-11-19, on the pure zoom seam (`app/tests/zoom.rs`) and at the Feed seam (`app/tests/clean_zoom_seam.rs`, `app/tests/picture_stage_seam.rs`; a GPU, no Resolve).

- Wheel steps from Fit are free below 100 % (ratio 1.25 per notch), land exactly on 100 %, then go 200, 300, 400 % and stop at 400 %; wheeling out retraces them and stops at Fit. A part of a notch waits for the rest.
- The picture pixel under the pointer stays under it (within a device pixel or two of rounding) except where the picture's edge stops at the Panel's edge or an axis is smaller than the Panel and stays centred.
- Frames whose neighbouring pixels all differ draw in exact 2, 3 and 4 device-pixel squares at 200, 300 and 400 % (the stage reads the exact texel above 100 %), and at Fit the picture is drawn byte for byte as before.
- Zoomed in, nothing is drawn outside the Panel's picture area; black mattes stay black and the legend strip and bar are where they were.
- The chip reads "Fit 75 %" by default and "200 %" zoomed, lit only while zoomed. The popover slider runs Fit to 400 %, is free below 100 %, snaps to 100 and then whole steps above (a value of 150 to 249 sets 200 %) and never goes past 400 %. Zoom is never in the saved settings text and a new session opens at Fit.
- A frame already past 100 % at Fit (160 x 90 in 400 x 300 is Fit 250 %) zooms on to 300 and 400 %; at Fit 400 % or more there is nothing beyond Fit.
- On a title bar too narrow for its chips the zoom chip shortens to its number and stays first on the bar. The clean picture's six chips still do not all fit the narrowest Panels (#171).

Not verified (a person at the app beside Resolve, #170): the wheel, chip and popover in the running window with the mouse, and that zoom costs playback nothing at UHD 50.
