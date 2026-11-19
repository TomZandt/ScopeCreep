# #85: soft spread as a slider on the waveforms

What ticket #85 showed on 2026-11-19, at the display seam (`display_seam.rs`, `colour_waveform_seam.rs`, `waveform_seam.rs`), headless.

- The soft spread is 0 to 3 device pixels in steps of 0.25, default 1 pixel, on the luma waveform and the Colour Waveform. An old On is 1 and an old Off is 0 (the same pixels as before); a stored number out of range is clamped, an unreadable one is the default.
- The default draws the same pixels as the old On, and 0 the same as the old Off, checked on the waveform seam row by row and on a ramp at both displays.
- Weights: at 1 pixel the old 0.6 centre and 0.1 for each of four neighbours, exactly. Below 1 the neighbours' weight scales down; each pixel beyond 1 adds a ring of four neighbours one pixel further out at 0.05 each, scaled by the part of the pixel. At 3 pixels the centre keeps 0.2. Growing the amount never lights a row less and never brightens the centre.
- 0 returns the traced pixel before any neighbour is read. The counts and the label do not change (spread is drawing only).
- The vectorscope's soft spread is still On or Off; it is a separate ticket of spec #83.
- Not verified: how the spread looks at 2 or 3 pixels on a real picture beside Resolve, and on a HiDPI display (the last ticket of spec #83, #127).
