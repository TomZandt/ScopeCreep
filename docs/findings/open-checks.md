# Open checks

Everything we know we haven't verified yet, in one place. Each line points to the findings file with the evidence. When a check is done, record the result in the relevant findings file and delete the line here. Tickets that will cover a check are named.

## Needs a person at Resolve (scripts can't do it)

- **DaVinci tone mapping on:** the setting reads back "DaVinci" but renders identically to off by script; check in the UI whether it is really applied, then measure it. Until then it is labelled approximated. ([10](10-published-maths-accuracy.md))
- **Gamut mapping on:** the scripting API accepts only "None"; set it in the UI and measure. ([10](10-published-maths-accuracy.md))
- **ACES 1.3:** the version can't be set or read by script; build an ACES 1.3 project in the UI, then port the 1.3 output transforms. ([06](06-test-harness-and-colour-setup.md), [10](10-published-maths-accuracy.md))
- **Hidden branch variants:** note the real node wiring, try key output gain 1, try a Layer Mixer or mixer weight 0, try the CST off the main path. Parked, not needed for Milestone 1. ([09](09-hidden-branch.md))
- **Timeline node cache mode Off:** can't be set by script on a timeline node; set it in the UI and confirm it stops cache bursts reaching the Sender. ([11](11-sender-behaviour-in-resolve.md))
- **Color Output render cache:** can't be enabled by script; check whether it hides the Sender on cached playback. ([11](11-sender-behaviour-in-resolve.md))
- **A real adjustment clip** above the footage, and a strong grade on an earlier timeline node, to confirm the placement guidance. ([11](11-sender-behaviour-in-resolve.md))
- **Shift-D and scrubbing on the Edit and Cut pages**, and a screenshot of the bypassed viewer. ([11](11-sender-behaviour-in-resolve.md))
- **A real fast mouse scrub** with the app running (keystrokes can't simulate a drag). ([14](14-feed-reaches-the-app.md))
- **Resolve's waveform Extents and Video vs Data Level Scopes** settings, and how they change readings. Ticket #18. ([08](08-resolve-luma-weights.md))
- **Resolve's own playback and dropped-frame counters:** not scriptable; every rate so far is inferred from the Sender's timing. Read them on screen during a benchmark run. Ticket #20. ([12](12-uhd-copy-cost.md))

## Not tested yet (could be automated)

- **Colour Setups never measured:** Rec.2100 HLG, Rec.2020 Gamma 2.4, P3-DCI, other ACES output transforms. Labelled approximated until the accuracy suite covers them. Ticket #17. ([10](10-published-maths-accuracy.md))
- **Data levels:** every test project uses video levels; a data-levels project is needed. Ticket #18. ([06](06-test-harness-and-colour-setup.md))
- **UHD at 50 and 60 fps with the real Sender and app**, long runs, and multiple Sender instances. Tickets #19, #20. ([12](12-uhd-copy-cost.md), [14](14-feed-reaches-the-app.md))
- **A real cache fill with the real Sender and app** (only tested synthetically), and whether pacing can tell cache fills from playback at 50 and 60 fps (expected not to). Ticket #19. ([14](14-feed-reaches-the-app.md))
- **Shift-D, a disabled node and the Fusion page with the app running** (the "no frames" state is only tested synthetically). ([14](14-feed-reaches-the-app.md))
- **Colour-managed projects at UHD:** the copy-cost test used YRGB only. ([12](12-uhd-copy-cost.md))
- **Reader crash during copy:** a slot stays "reading" until the ring restarts (the writer just drops). ([12](12-uhd-copy-cost.md))
- **Grades before the Sender node** in the performance tests. ([12](12-uhd-copy-cost.md))
- **GPU cost of the Sender's passthrough** with no app attached, and Resolve's GPU memory with and without the Sender. ([14](14-feed-reaches-the-app.md))
- **Frame rates above 60 fps.** ([12](12-uhd-copy-cost.md))
- **The one-off render mismatch** (max diff 9.0) seen once on the first ACEScct clone; not reproduced. ([07](07-sender-node-placement.md))

## Open product decisions

- **Default luma weights:** primaries-derived (current default, ADR 0004) or Match Resolve (what Resolve's own waveform shows on P3)? ([08](08-resolve-luma-weights.md), [13](13-report-and-decisions.md))
- **How the Rust app reads Resolve's Python/Lua scripting API.** Ticket #16 is deciding this now.
