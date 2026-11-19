# #127: graticule brightness slider on every trace display

What ticket #127 showed on 2026-11-19, at the Feed seam (`waveform_seam.rs`, `colour_waveform_seam.rs`, `vectorscope_seam.rs`, `cie_seam.rs`), headless.

- One slider, "Graticule brightness", with the same key (`graticule`), range (20 to 300 % in steps of 5) and default (100 %) on the luma waveform, the Colour Waveform, the Vectorscope and the CIE plot. It lives once, in the scope shader library (`scope_shader.rs`, `graticule()` in `scope_shader.wgsl`), so the four cannot drift apart.
- The default draws the same pixels as before (the 100 % render equals the unset render, byte for byte, on each display). It is a gain on how far a mark sits above the plot's own grey, not on its grey, so the lowest setting fades the marks towards the plot but never below it, and the highest never passes white.
- What it scales: the waveform's reference lines and legal lines; the vectorscope's axes, half ring, skin-tone line, and the ring's hue wheel and the target boxes (their muting, capped at 0.9 so a bright graticule never outshines the clipping marks); the CIE plot's 0.1 grid. What it never scales: the traces, the clipping marks on the vectorscope ring, the CIE locus and gamut triangles (the measurement), the waveform's edge rows (out of range).
- Drawing only: the labels do not change, and it is never a tag. Saved in the Panel's settings like the other sliders; an unreadable value is the default, out of the range is clamped.
- The CIE plot's brightness slider (#87) was not in this tip; the graticule row is a separate entry after the timeline triangle so the two merge without touching the same lines.
- Not verified: how the graticule looks at 20 % and 300 % beside Resolve, on a real monitor and HiDPI; the scale numbers drawn as text beside the waveform were left as they are (#91 covers the slider feel check).
