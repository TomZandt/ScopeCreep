# Findings

What we learned by testing, kept in the repo so it isn't lost when an issue thread scrolls away. Each file is copied from the closing comments of its GitHub issue. Decisions drawn from these findings are in [`../stage0-report.md`](../stage0-report.md) and the ADRs in [`../adr/`](../adr/).

**Still to do or check** lives in GitHub issues, never in these files: [open issues labelled `ready-for-human`](https://github.com/TomZandt/scopecreep/issues?q=is%3Aopen+label%3Aready-for-human) (needs a person at Resolve or a decision) and [`ready-for-agent`](https://github.com/TomZandt/scopecreep/issues?q=is%3Aopen+label%3Aready-for-agent).

| File | Question |
|---|---|
| [04](04-probe-sender.md) | Does a plugin load in Resolve, and does Resolve tell it the colour space of each frame? |
| [05](05-external-scripting.md) | One-time setup: external scripting set to Local |
| [06](06-test-harness-and-colour-setup.md) | Can the Colour Setup be read by script? (The Resolve test harness) |
| [07](07-sender-node-placement.md) | Placing the Sender on timeline nodes, and cloning it to every test project |
| [08](08-resolve-luma-weights.md) | Which luma weights does Resolve's own waveform use? ([measurement notes](08-luma-weights-measurement-notes.md)) |
| [09](09-hidden-branch.md) | Can Resolve do the Output Conversion for us through a hidden branch? |
| [10](10-published-maths-accuracy.md) | Does the published colour maths match Resolve exactly? |
| [11](11-sender-behaviour-in-resolve.md) | How does Resolve treat the Sender: caching, Shift-D, placement, thumbnails? |
| [12](12-uhd-copy-cost.md) | What does copying UHD float frames out of Resolve cost? |
| [13](13-report-and-decisions.md) | The Stage 0 report and the decisions it made |
| [14](14-feed-reaches-the-app.md) | Milestone 1: what the Feed tracer bullet verified in Resolve, and its limits |
| [16](16-colour-setup-by-script.md) | Reading the Colour Setup by script while Resolve runs: every test project, read times, reads during playback |
| [15](15-luma-waveform-uhd.md) | Does the luma waveform keep up with UHD float at 24, 25, 50 and 60 fps? |
| [17](17-output-conversion-accuracy.md) | Does the app's GPU Output Conversion of the real Feed match Resolve's renders, and what did 32-bit float need? |
| [18](18-levels.md) | What do video and data Levels change in Resolve, and does the waveform follow them? |
| [19](19-playback-rate-drops-and-senders.md) | Is the app's playback rate Resolve's? How do two Senders and background cache fills behave at 24, 25, 50 and 60 fps? |
| [20](20-performance-benchmark.md) | Does the Sender, or the Sender plus the app, slow Resolve down at UHD 50 fps? |
| [52](52-colorchecker-test-frames.md) | Do the ColorChecker test frames match the research table, and do their grey patches reach the waveform where the Output Conversion says? |
| [46](46-sender-colour-setup-controls.md) | Can the Sender node grey out its manual Colour Setup controls, and can it show the project's values in them? |
| [50](50-panel-grid.md) | Milestone 2: does the Panel grid fill, drag, remember and draw correctly (headless)? |
| [55](55-channel-view-and-loupe.md) | Milestone 2: does the Channel view draw the chosen channel, and does the Loupe magnify in exact blocks, move, stay inside, zoom 100 to 800 % and persist (headless)? |
| [51](51-panel-settings-menu.md) | Milestone 2: do Panel settings persist per Panel, and does the luma weights setting change the measurement and the label (headless)? |
| [65](65-highlight-neutrals.md) | Milestone 2: are all six ColorChecker greys green and no coloured patch green, and does a known channel offset turn green only once the tolerance covers it (headless)? |
| [101](101-skin-tone-solid-colours.md) | Skin Tone: solid flat colours on the natural picture at the Feed seam |
| [57](57-arri-false-colour.md) | Milestone 2: does ARRI False Colour show each ColorChecker patch in the research table's band, change band at each edge, and toggle its legend (headless)? |
| [53](53-waveform-look.md) | Milestone 2: does the waveform's phosphor look follow density, keep strays faint and edge rows clear, and look the same in any Panel size (headless)? |
| [54](54-black-and-white-picture-view.md) | Milestone 2: is every ColorChecker patch drawn black and white at the Luma the waveform traces for it, and does it follow the waveform's luma weights (headless)? |
| [56](56-gamut-legal-warning.md) | Milestone 2: does the gamut/legal warning tint, limit, tolerate (R103) and label correctly (headless)? |
| [64](64-saturation-heatmap.md) | Milestone 2: does the saturation heatmap show each ColorChecker patch at its Cb/Cr distance, greys cool, 100 % bars at the 100 % point, a scale that remaps and a legend that toggles (headless)? |
| [100](100-saturation-heatmap-overlay.md) | Milestone 3: does the heatmap keep the picture as its Luma grey with saturation laid over it, and what default scale lets a graded picture use most of it (headless)? |
| [59](59-colour-waveform.md) | Milestone 2: does the Colour Waveform tint each spot by the average colour of its pixels at the luma waveform's positions, with the same settings (headless)? |
| [60](60-skin-tone-line.md) | Where does Resolve draw its vectorscope skin-tone line? (123.3° ± 0.5° on screen, from a screenshot) |
| [61](61-vectorscopes.md) | Milestone 2: do the vectorscopes land bars and ColorChecker hues on target, at twice the radius, with the assumed skin-tone line (headless)? |
| [63](63-skin-tone-panel.md) | Milestone 2: does the Skin Tone Panel colour ColorChecker skin by hue against the assumed line, keep neutrals and far hues grey, go green and magenta with distance, and follow its tolerance (headless)? |
| [93](93-stops-false-colour-scale.md) | Does Stops False Colour put the ColorChecker in the right one of 17 zones, lay its scale out horizontally with numbers under their swatches, thin its text instead of overlapping it, and draw zones or a message instead of black (headless)? |
| [58](58-stops-false-colour.md) | Milestone 2: does Stops False Colour measure the ColorChecker's published stops in every supported Timeline Colour Space, and do both modes paint the right zones (headless)? |
| [62](62-cie-plot.md) | Milestone 2: does the CIE plot put every ColorChecker Patch at its reference chromaticity, draw the right triangles and label both spaces (headless)? |
| [67](67-milestone-2-performance.md) | Milestone 2: does Resolve still play UHD 50 at 50 fps with the Sender, and with the app showing all 14 Panels? |
| [review-milestone-2](review-milestone-2.md) | Milestone 2 code review: what the review found in the integration branch, what was fixed and what was left, with the tests that pin it |
| [114](114-style-tokens.md) | Style: are chrome colours and sizes tokens by role with no visible change, and does the check catch a raw literal? |
| [115](115-style-components.md) | Style: do the components lay out and look right in every state at 1x and 2x, and is every legend horizontal? |
| [95](95-panels-out-of-the-window.md) | Taking the gamut/legal warning, Highlight Neutrals and CIE plot out of the Window: what stays, and GPU memory when running tests beside Resolve |
| [110](110-picture-stage.md) | Refactor: does every Picture View still draw the same through the shared Picture stage? |
| [96](96-panel-layout.md) | Panels span cells: the default layout, minimum sizes at 1280 x 760, borders, saved layouts, title bar |
| [98](98-named-layouts.md) | Named layouts: saved, recalled, renamed and kept at the pure seam; what a recalled layout does when Panels have changed |
| [99](99-window-text.md) | The Window's text: the Colour Setup once in a top bar, short title bars with only the tags a Panel needs, a performance-only status line |
| [117](117-luma-waveform-channels.md) | Does the luma waveform measure and draw R, G and B beside Luma at each channel's code, show out-of-range per channel in the edge rows, keep one chip on and remember the choice? |
| [92](92-one-vectorscope-panel.md) | Does one Vectorscope Panel show both plots with one setting, one measurement and the same clipping, and what happens to old saved layouts and settings (headless)? |
| [97](97-swap-panels.md) | Does dragging a Panel onto another swap cells and spans, remember it, and never confuse a border drag with a Panel drag (pure seam)? |
| [107](107-shared-gpu-test-bench.md) | Refactor: does one shared GPU test bench replace the copied setup with the tests unchanged? |
| [108](108-measurement-module.md) | Refactor: do Measured Space and luma weights live in a measurement module of their own, with the seam tests unchanged? |
| [116](116-panel-toolbar.md) | Does the Channel view's Y R G B toolbar switch, remember, shrink on small Panels and show the right tags (pure seam)? |
| [160](160-clean-picture-channel-chips.md) | Do Y R G B chips on the clean picture switch it between colour and one channel as grey, and does a saved B/W carry over? |
| [161](161-clean-picture-loupe.md) | Does the clean picture's Loupe chip magnify colour and each channel in exact blocks, stay inside, and take no pointer events when off? |
| [162](162-channel-view-out.md) | With the Channel view out of the Window, does the default layout fill every cell at the minimum Panel size, and does a saved layout naming it still recall? |
| [118](118-bw-toggle.md) | Does the B/W toggle switch the unpainted picture between Luma grey and colour without changing what a Panel paints? |
| [126](126-stops-mode-chips.md) | Is the Stops False Colour mode one click on the title bar, remembered, with B/W only in Grey +1? |
| [104](104-viewer-gate-follows-scrubbing.md) | Does the viewer gate show the picture while the playhead is dragged, and the stop frame at once, without showing cache fills (replay of the recorded scrub)? |
| [143](143-colorise-chip.md) | Does a Colorise chip tint the luma waveform's Y trace in the picture's colour (synthetic and ColorChecker frames), leave counts and edge rows alone, and stay off, remembered and out of the tags (headless)? |
| [144](144-colorise-channels.md) | Does Colorise work on any Y R G B combination (channel colours on, neutral off, each trace at its code, edge rows per channel), and do three neutral traces read on ColorChecker frames (headless)? |
| [125](125-waveform-scale-units.md) | Does each scale unit (10-bit codes, %, nits) name the same trace positions, leave the counts alone, offer nits only for PQ and HLG, and tag the unit (pure and Feed seam)? |
| [145](145-colour-waveform-out.md) | Does the Window register 8 Panels, fill every cell of its default layout at the minimum size, and still recall a layout saved with the Colour Waveform (pure seam)? |
| [123](123-vectorscope-chips.md) | Does the Vectorscope title bar choose 100 %, 200 % or both plots and a colour or mono trace, remembered, with the plots tagged and the counts unchanged (headless)? |
| [124](124-vectorscope-tonal-range.md) | Does the Vectorscope's Tonal range chip plot only the colours of Shadows, Mids or Highlights, count clipping only from them, and tag the range (Feed seam, headless)? |
| [109](109-feed-loop.md) | Refactor: do the app and the tests run one Feed loop, and does it measure again after a setting or a Colour Setup change with Resolve stopped, and stay quiet when idle? |
| [84](84-brightness-slider.md) | Does a slider row in the Panel menu set waveform trace brightness (range, step, clamp, reset, legacy list values, default identical to the old Normal, drag past the ends) without changing the label or counts (pure and display seam)? |
| [89](89-gamut-thresholds.md) | Do the gamut/legal warning's blue and red thresholds work as sliders (defaults identical, blue never above red, old list values map to the number they named, the label names values and codes; Feed seam)? |
| [88](88-tolerance-sliders.md) | Do the Skin Tone and Highlight Neutrals tolerance sliders keep today's default look, widen the lit area monotonically, map old list values and clamp (display seam)? |
| [86](86-vectorscope-sliders.md) | Do the vectorscope trace brightness and soft spread sliders keep the old look at their defaults and old list values, make spread 0 the old Off, and leave the label alone (display seam)? |
| [87](87-cie-brightness-slider.md) | Does the CIE plot trace brightness slider keep the old look at its default and old list values, scale the trace monotonically and leave the label alone (display seam)? |
| [90](90-heatmap-scale-slider.md) | Does a scale slider on the saturation heatmap keep the default look, take any step, read old list values, clamp, and move the picture, label and tag (display seam)? |
| [127](127-graticule-brightness.md) | Does a graticule brightness slider on every trace display keep the default look, scale only the marks (never the trace, clipping, locus or label), and never fade a mark below the plot (Feed seam)? |
| [151](151-waveform-aspect.md) | Does the luma waveform's plot take the Feed frame's aspect (16:9, 2.39:1, 4:3, 9:16), centred with its gutter and edge rows, and follow a resolution change (pure seam and drawing seam)? |
| [85](85-soft-spread-slider.md) | Does a soft spread slider on the luma and Colour Waveforms keep the old On and Off looks, take any step from 0 to 3 pixels and leave counts and label alone? |
| [102](102-mattes-stay-black.md) | Do black mattes (letterbox and pillarbox) stay black on ARRI False Colour, Stops False Colour, Skin Tone and the saturation heatmap (Feed seam)? |
| [165](165-clean-picture-zoom.md) | Does the clean picture zoom from Fit to 400 % with the wheel toward the pointer and a chip with a slider, in exact squares above 100 %, with mattes and strip untouched, and never save the zoom (pure and Feed seam)? |
| [166](166-clean-picture-pan.md) | Does a middle-drag pan the zoomed clean picture exactly with the pointer, clamped at the Panel's edge and centred when smaller, and does zoom after a pan still go toward the pointer (pure and Feed seam)? |
| [167](167-picture-panels-zoom.md) | Does every picture Panel in the Window (ARRI and Stops False Colour, Skin Tone, saturation heatmap) zoom and pan on its own in exact squares with mattes black and the legend strip untouched, and show a zoom chip that survives a small Panel (Feed and Toolbar seams)? |
| [176](176-zoom-800-link-default.md) | Does zoom stop at 800 % in exact 8-pixel squares, does Link open on (saved off stays off, rubbish is on) and light on every picture Panel, and does a click on the zoom chip go to Fit on all Panels while linked (zoom model, Feed, Toolbar and Settings seams)? |
| [168](168-link.md) | Do picture Panels of different sizes share one zoom and spot while Link is on (Fit fits each, on takes the clicked Panel's view, off keeps each), with a link chip lit on all of them, and is Link remembered while zoom is not (zoom model, Feed, Toolbar and Settings seams)? |
| [146](146-commands-and-hotkeys.md) | Are the layout shortcuts named commands with default hotkeys that behave as before, and do assign, clear, reset, conflicts, reserved keys and the hotkeys file behave (pure command and hotkey seam)? |

## Adding findings

When a ticket answers a question by testing, its closing comment holds the evidence. Copy that comment into a new file here, numbered by the issue, and add a row to the table. Do it in the same change that closes the ticket.

Anything the ticket could not verify becomes a GitHub issue (or a comment on the planned ticket that will cover it), and the findings file links to it. When a later ticket settles a check, update the findings file it came from.
