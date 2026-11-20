# #32: out-of-range values from feed-synth, and the edge rows at scale 1.5 and 2

What ticket #32 showed on 2026-11-20, headless: pure tests of the test card (`feed/src/synthetic.rs`) and the Feed seam with an offscreen draw (`app/tests/out_of_range_hidpi_seam.rs`). No window was opened and Resolve was not used.

## feed-synth --out-of-range

- With `--out-of-range` the bottom twelfth of the moving test card (at least one row) is a grey ramp from −0.25 at the left to 1.25 at the right; the rest of the card is unchanged and the marker never runs through the strip. At 1920 × 1080, 320 columns on each side are below 0 and above 1 in all 90 rows of the strip, on every frame feed-synth writes (tested on frames 0 to 500). The plain card stays within 0 to 1.
- On the video levels the app assumes when no Colour Setup comes (which is always the case with feed-synth), a value lands on code value × 876 + 64, so −0.07 to 0 and 1 to 1.095 are still on the scale. Only 226 columns on the left (below code −0.5) and 198 on the right (at or above 1023.5) count in the edge rows, each with all 90 pixels of the strip; every other column counts nothing there. On full levels the 320 columns each side would fall off.

## The edge rows on the drawn Panel

- The feed-synth frame went in through the Feed, the app's Feed monitor and the standard display set, and the luma waveform Panel (a 720 × 420 logical cell, about its size in the default Window) was drawn offscreen at scale 1.0, 1.5 and 2.0: the display, the title bar (name, tags, Y R G B Col chips, settings button), the scale's numbers and the border, in the order `main.rs` draws them, cleared to the Window's surface.
- At every scale the top edge row is lit (at least 128 of 255) over columns 1760 and 1900 and the bottom edge row over columns 20 and 180; the other edge row there, and both edge rows at the in-range columns 700 and 1200, stay dark (under 64).
- This is the stand-in for the "screenshot of the real app" the issue asked for: the display drawing, layout, toolbar and title code are the app's own, but the text is set by the test, copying `main.rs` (same faces, colours, places and clip rectangles), not by `main.rs` itself. A person looks at the real app on HiDPI under #152 (comment added there) and the Window as a whole under #105.
- The captures are in `~/scopecreep-testdata/screenshots/32-out-of-range/` (not in git); `SCOPECREEP_CAPTURE=<folder> cargo test -p scopecreep --test out_of_range_hidpi_seam` writes them again.

## Legibility by eye (the captures)

- At 1.5 and 2.0 the title ("Luma waveform"), its tags ("unconverted, Rec.709 assumed, video levels assumed"), the chips' letters and the scale numbers 940, 512 and 64 are sharp and easy to read; the muted grey of the scale numbers and tags is quiet but clear on the dark surface. At 1.0 they are small but readable.
- The lit edge rows read at a glance as bright bars at the top right and bottom left of the plot, set apart from the plot by the dark gap above and below it.
- The edge-row labels "> 1023" and "< 0" are clipped at every scale: the top of "> 1023" by the title bar's line and the bottom of "< 0" by the Panel's border, because they are centred on edge rows at the very edge of the content and scale text is clipped to the content. Still readable, but cut: #183.
