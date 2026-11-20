# #176: zoom to 800 %, Link on by default, click the zoom chip for Fit

What ticket #176 showed on 2026-11-20, at the zoom model seam (`app/tests/zoom.rs`), the Feed seam (`clean_zoom_seam.rs`, `link_seam.rs`), the Toolbar seam and the Settings seam; a GPU, no Resolve.

- The wheel steps 100, 200 ... 800 % and stops at 800 %; coming back it retraces to 100 % and then the free steps to Fit. A frame already at Fit 250 % goes 300 ... 800 %; a frame whose Fit is 800 % or more has nothing beyond Fit. The picture pixel under the pointer stays put.
- At the Feed seam every picture pixel at 800 % is an exact run of 8 device pixels.
- No saved Link opens on; a saved "off" stays off; "rubbish", "ON", "1", "true" read as on. The link chip is lit on every picture Panel by default and goes out on all of them at once.
- The zoom chip's click asks for Fit, and `DisplaySet::zoom_to_fit` returns the Panel to Fit and the picture to what it drew at Fit; while Link is on every picture Panel goes to Fit.
- The slider popover is gone (`Menu::zoom` and its tests removed).

Not verified (a person at the app beside Resolve, #170): clicking the chip with a real mouse.
