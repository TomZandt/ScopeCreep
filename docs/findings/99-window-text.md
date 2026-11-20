# #99: the Window's text says only what a colourist acts on

What ticket #99 showed on 2026-11-18, at the pure text seam (`app/tests/window_text.rs`, `colour_setup_seam.rs` and `feed_seam.rs`; no GPU, no Resolve).

- The Colour Setup is one line in the top bar, for example `ACEScct → Rec.709 BT.1886  ·  Video levels  ·  from project, assumes ACES 2.0  ·  Output Conversion exact for ACES 2.0`. When the width is short its last parts go first and the route (Timeline → Output) is always kept.
- On an exact Colour Setup at default settings only the Vectorscope ("100 %" or "200 %") and the Channel view ("Blue", "200 %") carry tags; every other Panel's title is just its name. Tags appear for "approximated", "unconverted", "Rec.709 assumed", "full levels" or "video levels assumed" (waveforms), "luma weights: match Resolve", a non-default heatmap scale, skin tolerance, vectorscope targets and Stops mode, and "not measured".
- At 1280 x 760 the narrowest Panels are about 239 pixels wide. With the usual Colour Setups every title bar shows its name and every tag on one line, at 1x and 2x; with a picture shown as sent and every non-default setting on, the name still fits and the tags that fit are shown in priority order, the rest only in the hover. The widths are the layout's estimate (7.6 pixels a character), not the shaped text.
- The status line is the rate, the drops with why, and the feeding timeline; the Sender instance, process id, Resolve version and timeline rate moved to its hover with the project and the Feed. At 320 pixels (296 inside the margins) the rate and the drops are whole and visible; the timeline goes first, then why frames dropped, never a part cut in half.
- The feeding timeline's name comes from the Sender when the scripting side has supplied it, else from the project as last read; with neither it is left out rather than named by instance number.

Not verified (a person at the app): how the top bar, the title bars and the two hovers look and read at 1280 x 760 and on a real screen, and that the estimated widths match the shaped text: filed as ready-for-human: https://github.com/TomZandt/scopecreep/issues/135

**Checked at the app by the product owner, 2026-11-20 (#135):** the top bar, title bars, hovers, 320 px status line and the three Colour Setup title bars all pass.
