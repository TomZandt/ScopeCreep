# Colorise is one chip on the luma waveform, and the Colour Waveform leaves the Window

Resolve's waveform has one Colorize switch, inside whichever of its three exclusive modes (Y, CbCr or RGB) is chosen (`docs/research/colorise-on-the-waveform.md`, checked at Resolve 21 in #140). scopecreep had two Panels for what is one drawing in two inks: the luma waveform and the Colour Waveform (ADR 0022). With the Y R G B chips (#117) the luma waveform can show any combination, which Resolve's exclusive modes do not allow; that made a separate coloured Panel for Y alone the odd one out.

## Decision

- The luma waveform gets a **Colorise** chip on its title bar, default off, remembered per Panel (ADR 0013). It applies to whichever of Y, R, G and B are showing: a Y trace takes the average colour of the pixels at each position (ADR 0022's rule), R, G and B traces take their channel colours; with Colorise off the traces are neutral, as Resolve draws them.
- Colorise changes how the trace is drawn, not what is measured, so it is never a tag (spec #113's rule for drawing settings).
- The **Colour Waveform leaves the Window**, as the Black and white Panel did: its code and tests stay. The Window goes from 9 Panels to 8.
- Cost is measured, not assumed: the tint reads the same counts as the luma waveform (ADR 0022), and per-channel tints add work at UHD 50 (ADR 0003). The ticket records it in a finding.

## Considered and rejected

- **Keep both Panels.** Two Panels, one measurement. It costs a cell and a second set of settings, for a view the chip gives.
- **Colorise on Y only.** Simpler, but Resolve's switch is for the whole scope and the product owner expects it on any combination of the chips.
- **Follow Resolve's three exclusive modes (#202).** Colorise would apply only within Y, CbCr or RGB. Not taken: the chip is already a separate switch remembered per Panel, which is Resolve's sticky-across-modes behaviour, and combining Y with R, G and B is a superset of what Resolve offers.

## Consequences

- #117's R, G and B traces, now always in channel colours, become the Colorise-on look; their off look is neutral. The ticket must show how three overlapping neutral traces read, and the finding records it. If that is unreadable, revisit this ADR.
- ADR 0022's tint rule is kept; it now serves the chip, not a Panel.
- Checking Resolve 21's behaviour at the app is a ready-for-human issue.
