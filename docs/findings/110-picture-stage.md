# 110: a shared Picture stage for every Picture View

Refactor (spec #106): nine Picture Views now draw through `app/src/picture_stage.rs`; no behaviour was meant to change.

- Every existing seam test passes unchanged, the new `app/tests/picture_stage_seam.rs` covers the stage itself (fit and centre at several Panel shapes and offsets, scale, bottom-up Feed frames and top-down frames, the legend strip and bar, fill, no frame yet).
- One difference by design (ADR 0028): the five views that centred the picture with its unrounded size now round it first, as the other four did, so a picture can sit one device pixel differently in a Panel with a half pixel to spare. No test showed it.
- `display_seam::the_displays_draw_at_the_screens_scale` failed once with `wgpu error: Out of Memory` in a full run while Resolve held the GPU, and passed on its own; it is an environment limit, not the change.
