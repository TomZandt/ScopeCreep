# 88: Tolerance sliders on Skin Tone and Highlight Neutrals

What the ticket (#88, spec #83) verified, headlessly (the Feed seam, `app/tests/skin_tone_seam.rs`, `app/tests/highlight_neutrals_seam.rs`, `app/tests/window_text.rs`).

- Skin Tone: the tolerance is a slider from 1 to 12 degrees in steps of 1, default 5. The widest, 12, lights out to 72 degrees either side (bands run to 6 t), so the view cannot be set to mean everything. At 5 the ColorChecker and hue-frame tests are unchanged; a hue 20 degrees out goes from unlit to lit as the slider goes up, and never back to unlit on the way.
- Highlight Neutrals: the tolerance is a slider from 0.5 to 10 % in steps of 0.5, default 3. A grey with a 4 % channel offset is grey at 3.5 % and green from 4.5 %, monotonically; 3.0 / 100 is exactly today's 0.03.
- A list value an earlier version saved ("2", "3", "8", "10", "0.5", "1", "5") is the number it named; a value out of range is clamped; unreadable text is the default.
- The label shows the value in the slider's unit and rounding ("±7°", "within 3.0 %"); the Skin Tone bands in it are 6 times the slider's value. The Skin Tone title-bar tag follows the same rule when it is not the default.

Not verified (a person at the app): that dragging the sliders feels right on real footage, covered by the spec's ready-for-human check.
