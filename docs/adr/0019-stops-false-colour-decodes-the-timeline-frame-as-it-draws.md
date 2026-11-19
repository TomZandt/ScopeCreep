# Stops False Colour takes the Timeline Colour Space frame and decodes it as it draws

Every display used to get one frame: the picture after Output Conversion. Stops False Colour must measure scene light (ADR 0009), which only the frame before Output Conversion holds. Think of a photographer's light meter: it must point at the scene, not at the print.

## Decision

- The Display trait gets `on_timeline_frame`, called with the Timeline Colour Space frame right after `on_frame`. It does nothing by default, so no other display changes.
- Stops False Colour keeps that frame on the GPU and decodes it (curve, luma, stops, zone colour) in its draw shader. Nothing is measured ahead of drawing.
- Its typed `stops()` (for tests) runs the same decoding into a map of stops, on demand.
- A Timeline Colour Space with no curve here (or no Colour Setup) is labelled "not measured" and draws nothing. Rec.709 Gamma 2.4 is decoded by inverting its display curve and labelled "approximated" (ADR 0007).

## Considered and rejected

- **Measuring on the Feed thread into a stops texture:** the Colour Setup and settings arrive after the frame, so the texture would hold stops decoded with the old curve until something re-measured it.
- **Passing both frames to `on_frame`:** every display and test would change for one display's need.

## Consequences

- Each drawn pixel is decoded again on every redraw: one texture read and a few `exp2`s per pixel, which is small next to the Panel's size.
- A new display that needs scene light implements `on_timeline_frame` the same way.

## Luma

Stops use the Y of the colour as seen on a D65 display: the Timeline Colour Space's luma weights, adapted (Bradford) to D65. For DaVinci Wide Gamut and Rec.709 these are the usual weights. For ACES the white is not D65, so coloured patches differ by up to 0.02 stops from AP1's own luma; neutrals are identical. This is what the ColorChecker research table prints, and it is what the Output Transform's adaptation to D65 would show.

## Update 2026-11-18 (#93): never black

- The "not measured, draws nothing" rule is replaced. When the Timeline Colour Space is not known or has no curve here, Stops False Colour measures the **Output picture** (the frame every other display sees), decoded through its display curve: the Output Colour Space's curve (gamma 2.4 for Rec.709, BT.1886, sRGB read as a pure 2.4 and Rec.2020; gamma 2.6 for P3) when the picture is converted, and Rec.709 Gamma 2.4 assumed when the Feed is shown as sent. The label says "approximated: display-referred" and, when assumed, "assumed" (ADR 0007). Think of a light meter with no scene to point at: it reads the print instead and says so.
- The Display's `on_frame` frame is now kept as well as the Timeline Colour Space frame, and the Basis (which frame, which curve) is chosen in `configure`.
- Only when even the Output curve cannot be inverted (PQ, HLG, an unknown name) does the Panel draw a short message in the middle over a neutral background, and no picture.
- The zones are 17, centred on the stop marks: edges at the midpoints between neighbouring marks (-6 1/2, -5 1/2, ..., -3/4, -1/4, +1/4, +3/4, ..., +6 1/2), black below the first and white above the last.
