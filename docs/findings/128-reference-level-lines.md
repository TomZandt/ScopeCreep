# #128: reference-level lines on the waveforms

What ticket #128 (spec #192) showed on 2026-11-20, at the pure seam (`app/tests/reference_levels.rs`) and the display seam (`app/tests/waveform_seam.rs`), headless.

- The luma and Colour Waveforms' popover has two sliders, Low and High reference level, in the Panel's scale unit. Both lines are off by default; the waveform looks as it did until a slider moves. Low is off at the bottom of its range, High at the top.
- One Low and High pair per Panel (`reference_low`, `reference_high`), kept as a signal level: a percent of the Levels' range, -10 (Low off) to 110 (High off). The sliders show it in the Panel's scale unit and convert what they are set to back, so a 1000 nit line stays on 1000 nits whether the scale reads nits, codes or percent, moves when the unit changes, and is still there when a PQ or HLG output returns after one without light. Light does not depend on the Levels, so a change of Levels keeps the nits (and moves the code). A code is a few nits wide near 1000, so a line set in codes reads the nearest nit when read in nits.
- Chosen sliders (written here because the spec left them to the ticket). Low and High each reach the whole plot, so Low can mark a real floor and High a deliberate peak; the end of each slider is "off" (Low at the bottom, High at the top), and a line that is on is never read as off:

  | Unit | Low | High |
  |---|---|---|
  | 10-bit codes | 0 to 1023, step 1, default 0 | 0 to 1023, step 1, default 1023 |
  | Percent of the Levels' range | -10 to 110, step 1, default -10 | -10 to 110, step 1, default 110 |
  | Nits | 0 to 100, step 0.1, default 0 | 0 to 10000, step 1, default 10000 |

  Low never goes above High: a slider stops at the other line, and a hand-edited file is clamped. Percent runs from -10 to 110 because video levels' footroom ends at -7.3 % and headroom at 109.5 %, so black (0 %) and white (100 %) are both real lines. Nits Low stops at 100 nits (a floor, to a tenth of a nit); nits High has steps of 1 so 203 and 1000 are on a step. Dragging with the pointer over a 10000 nit track cannot land on every nit: that feel is part of #200.
- A line is drawn only when its slider is off the "off" end and its code lands strictly inside the plot (codes 1 to 1022): never on the edge rows, never off the plot.
- Lines are dashed (4 device pixels on, 4 off) in a neutral grey (0.36) and follow the graticule brightness slider; the test lights only the two rows of the lines at any gain, and a dimmer to brighter gain gives a dimmer to brighter line. They change no count (the counts after setting the lines equal the counts before).
- The popover offers the one pair in the unit of the scale. Where nits are not offered (an Output Colour Space without light, or an assumed one), a Panel set to nits reads codes on its scale (ADR 0033), so its sliders read codes too: the slider and the scale agree. The pair keeps its light meanwhile and is on the same nits when a PQ or HLG output returns. On an HLG output a High above 1000 nits clamps to the top and is not drawn.
- Not verified: how the lines and sliders look and feel in the Window, on a real monitor and on HiDPI, covered by #200.
