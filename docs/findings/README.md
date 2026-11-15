# Findings

What we learned by testing, kept in the repo so it isn't lost when an issue thread scrolls away. Each file is copied from the closing comments of its GitHub issue. Decisions drawn from these findings are in [`../stage0-report.md`](../stage0-report.md) and the ADRs in [`../adr/`](../adr/).

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

## Adding findings

When a ticket answers a question by testing, its closing comment holds the evidence. Copy that comment into a new file here, numbered by the issue, and add a row to the table. Do it in the same change that closes the ticket.
