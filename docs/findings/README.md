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
| [57](57-arri-false-colour.md) | Milestone 2: does ARRI False Colour show each ColorChecker patch in the research table's band, change band at each edge, and toggle its legend (headless)? |
| [53](53-waveform-look.md) | Milestone 2: does the waveform's phosphor look follow density, keep strays faint and edge rows clear, and look the same in any Panel size (headless)? |
| [54](54-black-and-white-picture-view.md) | Milestone 2: is every ColorChecker patch drawn black and white at the Luma the waveform traces for it, and does it follow the waveform's luma weights (headless)? |

## Adding findings

When a ticket answers a question by testing, its closing comment holds the evidence. Copy that comment into a new file here, numbered by the issue, and add a row to the table. Do it in the same change that closes the ticket.

Anything the ticket could not verify becomes a GitHub issue (or a comment on the planned ticket that will cover it), and the findings file links to it. When a later ticket settles a check, update the findings file it came from.
| [56](56-gamut-legal-warning.md) | Milestone 2: does the gamut/legal warning tint, limit, tolerate (R103) and label correctly (headless)? |
