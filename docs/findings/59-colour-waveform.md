# 59: Colour Waveform

What the Colour Waveform ticket (#59) verified, headlessly (no Resolve involved: the Feed seam with synthetic frames, reading back what the Panel counted and drew; `app/tests/colour_waveform_seam.rs`).

- A 1920 × 1080 frame of one flat colour (0.2, 0.8, 0.3) draws a green trace (red about 25 % and blue about 37.5 % of green) at the colour's Luma code, and the rest of the plot stays dark.
- Two colours alternating by row, chosen to land on the same Luma code, draw the same colour (within 3 of 255 per channel) as a flat frame of their average.
- The Colour Waveform's counts equal the luma waveform's for the same busy frame, every column and code.
- Trace brightness (Dimmer, Normal, Brighter) draws increasing brightness; soft spread changes the drawing; neither changes the label.
- The Colour Waveform offers only those two settings and follows the luma waveform's luma weights.

Not verified (a person at Resolve): UHD 50 playback with the extra colour pass, https://github.com/TomZandt/scopecreep/issues/68; the look next to Resolve's own colour waveform by eye, https://github.com/TomZandt/scopecreep/issues/74
