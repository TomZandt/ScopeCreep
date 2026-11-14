# The Sender is the primary Feed Source, and scopecreep performs the Output Conversion

Resolve 21.2 offers no software video output (no native NDI, IP or frame API), so the Feed comes from an OFX plugin (the Sender), with Blackmagic Capture as a secondary Feed Source. Capture sees the true post-transform output but needs hardware and is limited to 10/12-bit integer (8-bit in Resolve free). The Sender is software-only, cross-platform and 32-bit float, but it sees the Timeline Colour Space, not the Output Colour Space. So scopecreep has to perform the Output Conversion itself.

## Considered Options

- **Resolve converts, we discard the result.** A side branch in the node tree runs Resolve's own CST into the Sender and is mixed in at 0%. It would be exact, but Stage 0 found it does not work as built: Resolve still renders the hidden probe for every frame, but it receives all zeros and the branch changes the delivered output (see `docs/stage0-report.md`). Parked, not rejected: it could later cover what published maths cannot.
- **Published maths (chosen).** Stage 0 proved it exact against Resolve's own renders for YRGB Rec.709, DWG/DI to Rec.709, P3-D65 and Rec.2100 PQ, and ACES 2.0 to Rec.709, P3-D65 and PQ. DaVinci's tone mapping is unpublished and untestable by script, so it can only be approximated, and the scopes must say so.
- **CST sandwich done by the user.** Works, but it's the clumsy setup the product exists to remove.

## Consequences

- Every Output Conversion is labelled exact or approximated in the UI. We never present an approximation as truth.
- Accuracy is checked against frames Resolve itself renders in the Output Colour Space (16-bit TIFF or EXR), not against our own maths. No capture hardware test rig is planned.
- The recommended placement is the Sender as the last timeline node, not an adjustment clip (which competitors recommend). Adjustment clips appear to miss timeline-level grades. The Sender must handle caching (Resolve renders frames in the background) and Shift-D bypass.
- Stage 0 changed the Colour Setup source: OFX colour management gives a plugin no colour space (ADR 0005), so the Output Conversion is chosen from a Colour Setup read by script or set manually.
- Only setups proven exact in the Stage 0 accuracy table are labelled exact. Untested setups (HLG, Rec.2020 Gamma 2.4, P3-DCI, other ACES output transforms, ACES 1.3, tone or gamut mapping on) are labelled approximated.
- The Sender sees a node's input, before that node's own grade, and gets no frames under Shift-D. Placement guidance and non-viewer render handling are in `docs/stage0-report.md`.
