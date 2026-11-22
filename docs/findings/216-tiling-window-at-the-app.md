# 216: the tiling Window at the app (first looks, 2026-11-22)

Copied from the product owner's checks at the app, a UHD monitor, with the Feed from `feed-synth` (no Resolve). Two looks: the first, on the build of the 20 tickets of spec #196; the second, a guided pass of ten steps on the build after #224 and #225.

## First look: what was wrong, and where it went

- The **+** on the Page bar, and the edge **+**, opened their list at the top left of the Window: fixed (anchors worked out at click time).
- Panel titles said "Clean picture 2", "False Colour 3" with one on the page, and the Add list showed every instance: fixed (a number only when a page holds two or more of a display; the list shows each display once, A to Z).
- The Colour Setup line showed no text (#217): fixed (its fill was drawn over the text).
- The view strip read "Fit Fit Link" (the button and the readout): the readout is now a percentage; the strip itself was then replaced by the view block on each Panel's title bar (ADR 0042, then ADR 0044).
- The app froze now and then. **Not reproduced**: a stress test of the Feed thread against the Window thread never hung. One real stall was removed (the display set's write lock taken every draw). If it freezes again: `eu-stack -p $(pgrep scopecreep)` before closing it, and file the stack as an issue.
- The owner asked for: one page by default with their own arrangement as the default layout, the displays renamed (Channel Viewer, Waveform, Vectorscope, False Colour - Arri, False Colour - Stops, Skin Tone, Saturation), three hidden displays back (Gamut Warning, Neutrals, Chromaticity), the Sender called ScopeCreep, Link on by default, the Colour Setup line in the menu bar row, layout options on a right-click of a page tab, popover sliders that update while sliding with a value box and a reset on every amount, a reset of every Panel, and the brand accent #d2ff00 (#222, #223, #224, #225, #226, #228).

## Second look: ten steps, what the owner saw

1. Top of the Window (menu bar row, Colour Setup line, Page bar): yes.
2. Panel title bars (chips, a divider, Fit / zoom / Link, the settings button; scopes too): yes, **but the separation was not enough** and the view controls on scopes are pointless. Became #226 (spacing and the Hairline look, the block only on picture Panels).
3. Link and zoom (wheel, Fit, Link off): yes.
4. The edge **+**, the lists opening under their buttons, A to Z, each display once: yes.
5. Dock by dragging (overlay shows the exact half, "Tab here", Escape, Shift swap): yes.
6. Ctrl+Z through layout changes: yes.
7. Page tab right-click, adding pages, Ctrl+1 to 9: yes.
8. Auto layout (and a repeat press for the next-best arrangement), Auto resize, Undo: yes.
9. A Panel's popover (live sliders, value box, reset arrows): yes, **but "there is no reset all Panels to default anywhere"**. Became #228.
10. The Window at its least size: not reported.

## Not verified (homes)

- The Window at its least size (1420 x 833 after #226) and the look at UHD and HiDPI of the accent, the +N chip and the lime against the false-colour legends: a box on the standing queue #219.
- Auto layout and Auto resize on 2.39:1 and 9:16 footage in Resolve, and the UHD 50 loop (Sender off, on, on with the app, ADR 0003) with the most Panels and with copies of a display, including the cost of two Vectorscopes and four Channel Viewers (#186, #187): a box on the standing queue #120.
- The three displays that came back (#222), the new Sender name (#223): boxes already on #219 and #120.
