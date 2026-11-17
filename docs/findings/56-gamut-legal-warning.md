# 56: Gamut/legal warning

What the gamut/legal warning ticket (#56) verified, headlessly at the Feed seam (`app/tests/gamut_warning_seam.rs`; no Resolve involved).

- On a ColorChecker in DaVinci Wide Gamut / Intermediate converted to Rec.709 Gamma 2.4 by the app, cyan (negative red in Rec.709) is solid blue, and none of the other 23 patches is solid red or blue at true exposure.
- On one-colour frames, grey at 92.5 %, 95 % and 97.5 % is tinted 25 %, 50 % and 75 % of the way to red, and 7.5 %, 5 % and 2.5 % the same way to blue; 102 % and above is solid red, below 0 % solid blue. The result is the same at video and data Levels (percentages are of the range; values are full range in the Feed).
- One channel past a limit makes the pixel solid whatever the luma is.
- Choosing the red threshold 80 % or the blue threshold 20 % moves where the tint starts. With EBU R103 on, 103 % is a tinted pixel, 106 % solid red, -3 % not solid blue and -6 % solid blue; a channel at 103 % is tolerated too.
- The label names the Levels with the codes of the thresholds (video: 152 and 852; data: 102 and 921), the tolerance in use, the Output Colour Space (or that it is assumed and judged as sent), and that every channel is checked.

Not verified (a person at the app and Resolve): the tints on a real picture and monitor, and a real project's out-of-gamut colour against Resolve's own scopes: https://github.com/TomZandt/scopecreep/issues/72
