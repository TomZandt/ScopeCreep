# #108: Measured Space and luma weights in a measurement module

What ticket #108 showed on 2026-11-19.

- Measured Space, the luma weights (`LumaWeights`, `REC709_LUMA`, `DEFAULT_LUMA_WEIGHTS`), `Measurement` with its wording (ADR 0007), and the reading of the Luma waveform's `luma` setting (`luma_weights_of`) now live in `app/src/measurement.rs`. The waveform module no longer exports them; every display, the Colour Setup, Output Conversion, the CIE plot, the window text and the tests import them from `measurement`.
- The display context still resolves the luma weights from the Luma waveform's setting once per configure; no display reads the waveform's module for them.
- Only import lines changed in the tests; every assertion is unchanged and the whole suite passed before and after.
