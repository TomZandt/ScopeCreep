# The Sender is the primary Feed Source, and scopecreep performs the Output Conversion

Resolve 21.2 offers no software video output (no native NDI, IP or frame API), so the Feed comes from an OFX plugin (the Sender), with Blackmagic Capture as a secondary Feed Source. Capture sees the true post-transform output but needs hardware and is limited to 10/12-bit integer (8-bit in Resolve free). The Sender is software-only, cross-platform and 32-bit float, but it sees the Timeline Colour Space, not the Output Colour Space. So scopecreep has to perform the Output Conversion itself.

## Considered Options

- **Resolve converts, we discard the result.** A side branch in the node tree runs Resolve's own CST into the Sender and is mixed in at 0%. Preferred if prototyping proves Resolve still evaluates the hidden branch, because it would be exact.
- **Published maths.** ACES output transforms, DWG/DI, Rec.709, PQ and HLG are exact. DaVinci's tone mapping is unpublished, so it can only be approximated, and the scopes must say so.
- **CST sandwich done by the user.** Works, but it's the clumsy setup the product exists to remove.

## Consequences

- Every Output Conversion is labelled exact or approximated in the UI. We never present an approximation as truth.
- Accuracy is checked against frames Resolve itself renders in the Output Colour Space (16-bit TIFF or EXR), not against our own maths. No capture hardware test rig is planned.
- The recommended placement is the Sender as the last timeline node, not an adjustment clip (which competitors recommend). Adjustment clips appear to miss timeline-level grades. The Sender must handle caching (Resolve renders frames in the background) and Shift-D bypass.
