# #196: Can the Window tile like a window manager? A prototype and a critical look at the current Window

What spec #196 learned on 2026-11-20 from a critical look at the running app (UHD, scale 1, Resolve playing) and from a throwaway prototype of the tiling rules. The prototype is the branch `prototype/tiling-layout` (never merged); the decisions are ADRs 0038 to 0041.

## The current Window, seen at UHD (3840 x 2134, OS scale 1)

- **Wasted room on the picture rows.** The default layout's bottom two rows give each picture Panel a cell about 2,280 px wide; the 16:9 picture in it is about 670 px wide, under a third. The luma waveform's plot (aspect-true since #151) sits in the middle of a wide Panel with wide empty sides.
- **View controls move.** The zoom and Link chips sit on the same right-anchored row as each Panel's own chips, so Link's x differs on every Panel (after the zoom on the clean picture, before B/W on ARRI False Colour, before Zones on Stops False Colour) and is absent on the waveform and Vectorscope. #171 sends the chips that do not fit to the popover, so Link can vanish on a narrow Panel.
- **Chip groups run together.** The Vectorscope's eight chips (plots, Mono, tonal range) are separated only by small gaps.
- **The waveform's scale labels clip.** "> 1023" and "< 0" are cut by the title bar and Panel border (already #183, owned by spec #192). In the prototype's small waveform Panel, "1023" and "940" (and "64" and "0") drew on top of each other too, so a label that would touch another has to be left out; noted on #183.
- **Legend strips run the full Panel width** while the picture is narrower.
- **Small text and low-contrast numbers.** Scale numbers are 11 logical px and labels 12.5, with no UI scale preference; `TEXT_MUTED` on the window surface works out at about 3.8:1 (calculated, not measured). Filed as #195.

## The prototype

A single HTML page with a pure layout module (a binary tree of tab groups and splits) and a thin page around it, run in Node and in a browser. The rules it settled:

- **Dwindle works.** Splitting the focused cell along its longer side, 50/50, gave a sensible Window for 1 to 7 Panels on landscape, ultrawide, 4:3 and portrait monitors, and a portrait monitor flips the first split without any other rule.
- **Minimum size.** The prototype refused an add that would put a Panel under 120 x 96 and the tree never left an empty cell. The product owner chose to tab such a Panel into the focused tab group instead of refusing it (ADR 0038).
- **Dock zones.** The outer quarter of an edge splits, the middle tabs; the overlay showing the exact half read clearly.
- **Duplicates.** Four clean pictures numbered by reading order worked in the prototype, but numbering by position would change a Panel's settings owner when Panels move, so the app uses stable numbers (ADR 0039).
- **Auto layout** (one press) searched ordered cuts and picked a different arrangement for each of 16:9, 2.39:1, 9:16, a portrait monitor and an ultrawide Window. Run in Node: 7 Panels in 11 to 20 ms (1,280 candidate cuts, the first cut limited to the middle once there are more than 6 items), 9 Panels in about 420 ms (35,840 cuts), 3 Panels under 1 ms. A full search at 6 items is 1,344 cuts. Beyond 9 the prototype refuses.
- **Auto resize** (ratios only) left every Panel's place and nesting unchanged and gave picture cells the frame's aspect; after Auto layout it changed nothing, since Auto layout had already fitted them.
- **Auto layout as a toggle was tried and dropped.** It moved Panels whenever footage changed and needed a rule for hand arrangement; the product owner asked for a one-shot button, per page.
- **Pages.** Switching kept each page's layout, focus and undo; footage and monitor shape are Window-wide and a page does not re-arrange when they change.

## What it did not show

The prototype measured layout behaviour only, on a stand-in frame: no GPU time, no real Feed, no HiDPI, none of the app's own input handling, and no view strip (decided afterwards). The cost of copies of a display at UHD 50 is #197; the help page idea is #194; readability is #195; the waveform edge labels are #183 (spec #192).
