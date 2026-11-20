# #183: the luma waveform's edge-row labels drawn whole

What ticket #183 showed on 2026-11-20, headless: a drawing-seam test of `Waveform::scale_texts` (`app/tests/waveform_seam.rs`). No window was opened and Resolve was not used.

## The cause and the change

- #32 found "> 1023" and "< 0" cut by the title bar's line and the Panel's bottom border. They were centred on the edge rows, which sit at the very top and bottom of the content when the frame is tall enough to fill the Panel; `main.rs` clips scale text to the content.
- `scale_texts` now keeps the middle of each of those two labels at least half a line (`TYPE_SCALE` x scale x `TYPE_SCALE_LINE` / 2) from the content's top and bottom, so the whole line is inside. This happens before the overlap check, so a label that moves is still tested against its neighbours (940 and 64 in codes). The plot, the edge rows and every count do not move.

## What the test showed

- With a tall frame (the plot fills the content's height), at scale 1.0, 1.5 and 2.0 and in codes, percent and nits, the top of the "> ..." label is at or below the content's top and the bottom of the "< ..." label at or above its bottom. The test failed before the change.
- The existing waveform readings and seam tests are unchanged and green.

## Not verified

- Looking at the real app on a real monitor and on HiDPI: #198.
