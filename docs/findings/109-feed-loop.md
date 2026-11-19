# #109: one Feed loop run by the app and the tests

What ticket #109 showed on 2026-11-19.

- The Feed thread's body (poll, upload once, convert once, hand the frame to every display, measure the frame on screen again after a setting or a Colour Setup change, remake the labels) is `app/src/feed_loop.rs`. The app's thread only loops, sleeps and wakes the window; the display tests step the same `FeedLoop` through `Rig::step` (`app/tests/common/mod.rs`).
- `bw_seam`, `gamut_warning_seam` and `stops_false_colour_seam` carried their own simplified copy of the loop; they now drive the real one, so a setting chosen in a test measures the frame on screen again as it does in the app. Their assertions are unchanged and pass.
- New seam tests (`feed_loop_seam`): a setting changed with Resolve stopped measures the frame on screen again through Output Conversion; a Colour Setup read from the project once Resolve stops converts and measures the frame on screen again with no new frame; an idle step changes nothing and no display measures anything.
- An idle step reads the Feed's status, compares it with the last one and compares two settings counters. It allocates nothing; the slower work (texts, configuring displays, converting again) runs only when something changed.
- Not moved: the single-display benches (`highlight_neutrals_seam`, `skin_tone_seam`, `output_conversion_seam`) keep their own wiring because they measure one display or a waveform directly rather than a `DisplaySet`.
