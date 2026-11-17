# 53: Waveform look

What the waveform look ticket (#53) verified, headlessly (no Resolve involved: the Feed seam with synthetic frames, reading back the waveform as drawn into an 8-bit target; `app/tests/waveform_seam.rs`, `app/tests/display_seam.rs`).

- Counts are unchanged: the measurement tests from milestone 1 still pass.
- On a frame whose tones thin out towards white, the drawn trace is brighter where more pixels land and the sparse part is still above the 8 % floor, not a flat floor value.
- One stray pixel in an otherwise dense column shows in the plot (at least about 8 %) and is under half the brightness of the dense area.
- The out-of-range edge rows still light at 80 % or more (the milestone 1 test passes).
- Dimmer, Normal and Brighter draw increasing brightness on the same frame. Soft spread off lights only a code's own row (one row per code); on, the rows next to it are lit and rows two or more away are untouched.
- The same vertical-ramp frame draws within 15 % of the same mean brightness in a 400 × 300 Panel and an 800 × 600 Panel.
- Through the display set, the settings change the drawing but not the label or the counts.

Not verified (a person at Resolve): the look next to Resolve's own waveform on the UHD 50 test clip, whether the gain values feel right, and UHD 50 performance with the extra trace pass: UHD 50 with every Panel is https://github.com/TomZandt/scopecreep/issues/68;  the by-eye comparison: https://github.com/TomZandt/scopecreep/issues/74
