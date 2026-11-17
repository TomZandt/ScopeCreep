# The waveform's brightness is density relative to what the Panel and frame lead us to expect, with a phosphor response

## Decision

- A spot's brightness is `1 − exp(−gain · d)`, where `d` is the mean pixels per picture column in that screen row divided by what an even spread over all 1024 codes would put there (picture height ÷ 1024 per code, times the codes under the screen row). So `d` does not depend on the Panel's size or the picture's width, and `d = 1` is an ordinary, evenly spread trace.
- The gain is the "Trace brightness" setting: Normal 0.25 (default), Dimmer 0.12, Brighter 0.6. With Normal, one stray pixel in an HD frame lands near 11 % before the floor, an even spread near 22 %, and anything a few times denser is nearly full.
- Any non-zero count shows at no less than 8 % inside the plot; the out-of-range edge rows keep their 80 % floor.
- The soft spread is a separate pass: the plot's trace brightness is drawn into a texture (one texel per device pixel of the plot), and the Panel's own pass reads it, spreading it by at most one pixel (centre 0.6, four neighbours 0.1 each, never dimmer than the centre) or not, as set. The trace pass is submitted by the waveform itself just ahead of the window's commands, so the display shape (`Display::draw` into the caller's pass) is unchanged.
- The trace stays neutral grey; the edge rows are not spread.

## Considered and rejected

- **Keeping picture height ÷ 8 with a different curve:** it ties brightness to the frame's size alone, so a small Panel and a large Panel (more codes per row) disagree.
- **Normalising to the busiest spot of the frame (auto-exposure):** a single dense patch would dim the rest, and the same frame in two Panels would still agree but a changing picture would pump.
- **Blurring inside the one pass by sampling neighbours' counts:** repeats the expensive count sum for every neighbour.

## Consequences

- The gain numbers were chosen to give the intended ratios between a stray pixel, an even spread and a dense trace; how they look next to Resolve's waveform is judged by eye (see the findings).
- Other Scopes with the same look (Colour Waveform, vectorscopes, CIE plot) can reuse the response and the trace-texture pass.
