# #123: Vectorscope toolbar chips

What ticket #123 showed on 2026-11-19, at the pure UI seam (`app/tests/toolbar.rs`) and the Feed seam (`app/tests/vectorscope_seam.rs`, headless GPU).

- Plots are three chips, 100 %, 200 % and Both, one at a time; Both is the default and lit when nothing is stored. One plot fills the Panel's display area as a square; both are still measured in the same pass, so switching never re-measures.
- Mono is a toggle chip, off by default. A saturated red patch is red-tinted in colour and R = G = B grey in mono, at the same plot position, and the read-back counts are identical either way. The outer hue ring and colour targets keep their colours: they are graticule, not trace.
- Both choices round-trip through the saved settings per Panel. The popover keeps trace brightness, soft spread, targets and the skin-tone line.
- Tags: "100 % plot only" or "200 % plot only" when not Both (it changes what is read); the trace colour is never tagged. The label names the plots shown.
