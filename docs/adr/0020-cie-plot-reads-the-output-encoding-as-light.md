# The CIE plot decodes the Output picture to light by the Output Colour Space's name

## Decision

- The displays see the picture after Output Conversion, in the Output Colour Space's code values. The CIE plot undoes that encoding to linear light before it works out a pixel's chromaticity: a pure power for Rec.709 and Rec.2020 (2.4) and P3 (2.6), ST 2084 for Rec.2100 PQ, the BT.2100 inverse OETF for HLG. Channels below zero keep their sign, so an out-of-gamut colour lands outside its triangle, not on its edge.
- The primaries used are those of the Measured Space (the Output Colour Space, the waveform's rule); when the app shows the picture as sent, Rec.709 is assumed and the label says "assumed, measured as sent, unconverted" (ADR 0007).
- The decoding is chosen by the space's name, so sRGB (piecewise, not a pure 2.4) is read as gamma 2.4 and HLG is read without its system gamma. The label states the decoding it used ("gamma 2.4 decoded").
- The plot counts into a fixed grid of 0.001-wide bins (900 by 1150, covering x from -0.05 to 0.85 and y from -0.15 to 1.0) so that every gamut drawn fits, including DaVinci Wide Gamut's blue below y = 0. The plot reads back as those counts, and a Patch is checked against them to within 0.002.
- Both triangles are drawn from published primaries: the Output triangle (Rec.709, P3-D65, P3-DCI, Rec.2020) always, the Timeline triangle (DaVinci Wide Gamut, ACES AP1, Rec.709) unless the user hides it or the space is not one the app knows, in which case the label says "gamut not known". The label always names both spaces.

## Considered and rejected

- **Using the encoded code values as if they were light:** puts every saturated colour in the wrong place (a gamma-encoded Rec.709 red-ish pixel lands well inside the triangle).
- **A standard 0 to 0.8 by 0 to 0.9 plot:** the usual size, but DaVinci Wide Gamut's primaries lie outside it, so the Timeline triangle would be cut off.
- **Coarser bins (0.0025):** half the memory, but the bin width alone uses most of the 0.002 accuracy the ticket asks for.

## Consequences

- An sRGB or HLG Output Colour Space is plotted with a small, known inexactness (sRGB darks, HLG system gamma). Naming the decoding in the label is how it is not mistaken for more.
- Clearing and counting 1 035 000 bins every frame is a small fixed cost per frame, plus one atomic add per pixel; whether that keeps up at UHD 50 is checked in a separate issue, not assumed.
