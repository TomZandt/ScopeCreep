# The clean picture carries the channel and the Loupe, and the Channel view leaves the Window

A colourist checking noise or one channel had to look away from the clean picture, the largest Panel, to the Channel view in a 2 x 2 corner. The clean picture's B/W toggle was already the Channel view's Y chip, so the Window had two controls for one job. Hardware monitors and Resolve's Viewer treat a single channel as a one-press switch on the main picture, not a separate display. The black and white Panel (#118) and the Colour Waveform (ADR 0032) left the Window the same way.

## Decision

- The clean picture has **Y R G B chips** in a "one or none" mode (ADR 0013's note, #160): none lit is colour, the default; the lit chip clicked again returns to colour. Y is Luma grey with the waveform's weights (ADR 0004), so it replaces the B/W chip. A saved B/W "on" reads as Y.
- The clean picture has a **Loupe chip** (#161), off by default. The Loupe is one shared piece (#159) built from the Panel's colour rule, so it magnifies what the Panel shows; its state is kept beside the settings by display name (ADR 0015).
- The **Channel view leaves the Window** (#162). The Window registers 7 Panels. The Channel view's module, tests and saved settings are kept (one switch, `IN_WINDOW.channel_view`, turns it back on), and a saved layout that names it still recalls by the #98 rule: the Panel is dropped and a neighbour fills its cells.
- The default layout is retuned: ARRI False Colour three columns and Stops False Colour two on the third row, the Saturation heatmap three columns and Skin tone two on the fourth. Every cell is used and every Panel is at least the minimum size at 1280 x 760, the default Window size (the app sets no minimum Window size yet, #172; the toolbar fits only from about 190 px, #171).

## Considered and rejected

- **Keep the Channel view as well.** A second Panel for the same switch and the same Loupe, taking 4 cells for a quick look.
- **Delete the Channel view's code.** It may return if users miss a side-by-side channel Panel, and its tests and saved settings cost nothing to keep.

## Consequences

- The channel (value, chip, tooltip, title tag, shader index) is one table, `toolbar::CHANNELS`, that the chips, the colour rule and the title tag all read. The Loupe's on test, zoom and title tag are one place, `loupe.rs`, and what it reports for tests is `LoupeDrawn`.

- ADR 0011's "every display has its own Panel" holds for what is in the Window; a display can leave it without being deleted (see its dated note). ADR 0015's Loupe state now belongs to the clean picture as well as the Channel view.
- The check of the look against Resolve 21 (#75) moves to the clean picture.
