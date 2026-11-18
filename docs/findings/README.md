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

## Adding findings

When a ticket answers a question by testing, its closing comment holds the evidence. Copy that comment into a new file here, numbered by the issue, and add a row to the table. Do it in the same change that closes the ticket.

Anything the ticket could not verify becomes a GitHub issue (or a comment on the planned ticket that will cover it), and the findings file links to it. When a later ticket settles a check, update the findings file it came from.
| [67](67-milestone-2-performance.md) | Milestone 2: does Resolve still play UHD 50 at 50 fps with the Sender, and with the app showing all 14 Panels? |
| [review-milestone-2](review-milestone-2.md) | Milestone 2 code review: what the review found in the integration branch, what was fixed and what was left, with the tests that pin it |
| [114](114-style-tokens.md) | Style: are chrome colours and sizes tokens by role with no visible change, and does the check catch a raw literal? |
| [115](115-style-components.md) | Style: do the components lay out and look right in every state at 1x and 2x, and is every legend horizontal? |
| [95](95-panels-out-of-the-window.md) | Taking the gamut/legal warning, Highlight Neutrals and CIE plot out of the Window: what stays, and GPU memory when running tests beside Resolve |
