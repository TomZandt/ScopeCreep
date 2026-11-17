# 54: Black and white Picture View

What the black-and-white ticket (#54) verified, headlessly (no Resolve involved: the Feed seam; ColorChecker frames from #52 through the synthetic Feed writer and the app's Output Conversion) (`app/tests/black_and_white.rs`).

- Every one of the 24 ColorChecker patches (DaVinci Wide Gamut / Intermediate timeline, Rec.709 Gamma 2.4 output, video levels) is drawn as a grey (R = G = B) within one 8-bit step of the Luma the waveform traced it at: grey = (code - 64) / 876 of full scale.
- With a P3-D65 output, "match Resolve" on the waveform changes the greys of the black-and-white Panel too (the chart's green moves, as Rec.709's 0.7152 replaces P3-D65's 0.6917), the Panel stays within one 8-bit step of its trace, and both labels name the same weights ("to match Resolve", 0.7152 G).
- The Panel has no settings of its own (no menu): it takes the luma weights from the waveform's setting through the display context, so the two cannot disagree.
- The label is "Black and white" until a frame arrives, then names Luma, the Measured Space and the weights, like the waveform's.

Not verified (a person at the app and Resolve): the look in the real window, HiDPI, Resolve's own view and UHD 50 performance: https://github.com/TomZandt/scopecreep/issues/75
