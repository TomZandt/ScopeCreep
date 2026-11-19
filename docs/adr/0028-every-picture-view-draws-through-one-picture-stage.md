# Every Picture View draws through one Picture stage

Nine Picture Views each carried their own copy of the same drawing code: fit and centre the frame in its Panel, flip a bottom-up frame, cache what is bound to the GPU per texture, pack values for the shader by hand, and lay out a legend strip. The copies had drifted: five centred the picture with its unrounded size and four rounded it first, so the same frame could sit one pixel apart in two Panels, and every rule that must hold for all of them (mattes stay black, #102) would have had to be made nine times.

## Decision

- `picture_stage` is the only code that places, flips, binds and draws a Picture View's frame and its legend strip. A Picture View supplies a **colour rule** (WGSL with a `struct Values`, `fn colour(uv) -> vec3` and optionally `fn legend(uv) -> vec3`), its **values** (a plain `Pod` struct that is `Values` on the GPU, checked by size) and optionally a **legend** (strip height and bar position).
- The stage gives the rule `load_picture(uv)` (the exact texel) and `sample_picture(uv)` (smoothed where the device can), with the flip already done in `uv`.
- Placement is one function, `place`: scale to fit, round the picture's size to whole device pixels, then centre and round its position. The five views that centred with the unrounded size now round it first, as the other four did, so a frame can sit at most one device pixel differently from before in a Panel whose room leaves a half pixel over.
- The Channel view's Loupe stays its own extra draw on top of the stage's picture, using `place` and the stage's texture. Stops False Colour keeps its compute measuring pass and its text layout and draws through the stage with a "fill the Panel" call for its message.

## Considered and rejected

- **A trait with the colour rule as a Rust method:** the rule has to run on the GPU, so it would still be WGSL text with Rust around it.
- **Leaving Stops False Colour out:** it is the view whose legend the Window work moves next (#102, #94), so it is the one that most needs the shared strip.

## Consequences

- A new Picture View is its rule, its values and its settings; a fix to placement, flip or sampling is made once and tested once (`tests/picture_stage_seam.rs`).
- The stage always declares a sampler and a texture binding, even for rules that only use `load_picture`.

## Note, 2026-11-19 (#159)

The Loupe is no longer part of the Channel view: it is its own piece (`loupe.rs`) that any Picture View can draw on top of the stage. It is built from the Panel's own colour rule (the same WGSL the stage gets, with the same `Values`), so it magnifies exactly what the Panel shows. It still uses `place` and the stage's texture, as this ADR said. The Channel view is its first user.

## Note, 2026-11-19 (#161)

The clean picture is the Loupe's second user: it draws the Loupe on top of the stage only while its Loupe chip is on, with the same `Values` it gave the stage, and passes pointer events to it only while on.

## Note, 2026-11-19 (#164)

Placement of a Picture View's frame comes from the zoom model (`zoom.rs`, spec #163), which the stage asks instead of calling `place` directly. At Fit it gives exactly what `place` gives, so nothing on screen changes; `place` stays the Fit answer and what the Loupe uses.
