# 62: CIE chromaticity plot

What the CIE plot ticket (#62) verified, headlessly (no Resolve involved: the Feed seam, the Output Conversion and the GPU, in `app/tests/cie_seam.rs`).

- **Accuracy.** A ColorChecker chart in DaVinci Intermediate goes through the synthetic Feed writer, the app's Output Conversion (to Rec.709 Gamma 2.4 and to P3-D65) and the plot. Every pixel of each of the 24 Patches is counted in bins within 0.002 of the xy computed from the reference data (the chart's linear Rec.709 RGB through the Rec.709 matrix). The same light through a P3 output lands at the same xy, so the decoding and primaries of both outputs agree. Every pixel is accounted for: black (no light) is counted apart, none fall outside the plot.
- **Triangles.** The Output triangle follows the Colour Setup (Rec.709, P3-D65, Rec.2020 checked against their published primaries); the Timeline triangle is DaVinci Wide Gamut or ACES AP1 and turns off through the right-click setting. Trace brightness has three choices.
- **Label.** Names the Output and the Timeline Colour Space, the decoding used, and says "assumed, measured as sent, unconverted" when the Colour Setup is not known, "gamut not known" for a Timeline space the plot has no primaries for, and keeps naming the Timeline space when its triangle is hidden.
- **Drawing.** Rendered at 600 x 400, the white point is lit by the grey patches, a blank area is dark, the 520 nm point of the spectral locus and the corners of both triangles are lit, and the Timeline corner goes dark when the triangle is hidden.
- **Spectral locus.** The 33 points (380 to 700 nm every 10 nm) are the CIE 1931 2 degree standard observer's chromaticities, typed from the published table. They could not be re-read against the CVRL files from the machine that built this, and nothing in the tests proves them beyond the plot drawing through the 520 nm point: https://github.com/TomZandt/scopecreep/issues/77

Not verified: how the plot looks to a person at the app (brightness, line weights, HiDPI), and whether counting every pixel of a UHD frame keeps up at 50 fps (ADR 0003): https://github.com/TomZandt/scopecreep/issues/78
