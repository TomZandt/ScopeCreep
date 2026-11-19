# #167: every picture Panel zooms and pans

What ticket #167 showed on 2026-11-19, at the Feed seam (`app/tests/picture_panels_zoom_seam.rs`) and the Toolbar seam; a GPU, no Resolve.

- ARRI False Colour, Stops False Colour, Skin Tone and the saturation heatmap each zoom with the wheel from Fit to 400 %. At 100, 200 and 400 % a frame of one-pixel stripes is drawn in runs of exactly 1, 2 and 4 device pixels, on every colour rule.
- Zooming one Panel leaves the others at Fit or at their own zoom.
- A middle-drag moves the picture on every one of them.
- Zoomed and panned to the frame's top edge, black mattes stay black and nothing is drawn outside the Panel.
- With the legend on, the strip under ARRI False Colour and the saturation heatmap is pixel-identical at Fit and at 200 %.
- Every picture Panel's title bar starts with the zoom chip ("Fit 50 %" for the test frame), a click asks for the slider, and at 190 x 200 logical pixels (scale 1 and 2) it shortens to its number and stays inside the bar.
- The strip is in device pixels, so `Display::zoom` and `set_zoom` and `DisplaySet::toolbar` now take the scale.
- While Stops False Colour shows its message (timeline not decodable), it offers no zoom.

Not verified (a person at the app beside Resolve, #170): the wheel and middle-drag with a real mouse on each Panel; the gamut warning, highlight neutrals, black and white and Channel view displays are out of the Window and do not zoom.
