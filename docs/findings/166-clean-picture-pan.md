# #166: clean picture pan

What ticket #166 showed on 2026-11-19, on the pure zoom seam (`app/tests/zoom.rs`) and at the Feed seam (`app/tests/clean_zoom_seam.rs`; a GPU, no Resolve).

- A pan moves the picture exactly as far as the pointer moves (placement shifts by the drag, in device pixels, at any whole zoom).
- It clamps: dragged far either way, the picture's edge stops at the Panel's edge; a further drag moves nothing. An axis on which the picture is smaller than the Panel stays centred; at Fit there is nothing to pan.
- Zooming after a pan keeps the picture pixel under the pointer (within a pixel).
- Feed seam: a middle-drag over a zoomed split frame (dark left, bright right) moves the split with the pointer; after the release a move changes nothing; a middle-drag at Fit changes nothing; the left button and moves with no drag are not the Panel's with the Loupe off.

Not verified (a person at the app beside Resolve, #170): the middle-drag with a real mouse in the running window.
