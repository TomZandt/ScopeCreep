# #9 Stage 0: Hidden branch - can Resolve do the Output Conversion exactly?

> Copied from the GitHub issue comments on TomZandt/scopecreep#9 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-13)

**Findings so far: the hidden branch as hand-built does NOT work.** (Resolve 21.2 Studio, `scopecreep-test-dwg-p3d65`, timeline `hidden-branch`; experiment scripts in `harness/experiments/hidden_branch/`.) Leaving this open: it needs a human to try node-tree variants.

Node graph (from `GetNodeGraph()`): `sender`, an unlabelled Color Space Transform, a mixer, `sender-hidden` (probe, key output gain set to 0). Wiring itself couldn't be inspected by script.

1. **Does the hidden probe get a frame for every played/scrubbed frame at 0% mix?** Yes. 192 scrub steps (sequential and random) and 4 keystroke plays: both probe instances logged a RENDER for all 96 distinct times, none missing. **But the hidden probe's frame is all zeros** on every channel/pixel (ramp, chart, out-of-range, natural, frame 72). The plain `sender` probe's dump is bit-identical to the Timeline-Colour-Space feed dump.
2. **Do the hidden probe's frames match Resolve's Output-Colour-Space render?** No: it receives zeros (max abs err ~1.09 vs the 1e-4 tolerance). Disabling the mixer, the CST, or both in a scratch copy didn't change that, so the zeros come from the hidden node itself: either key output gain 0 makes Resolve feed the plugin a blank source, or its input isn't wired to the CST output. Not yet distinguished.
3. **Does the hidden branch change the delivered output?** **Yes, massively** (render of `hidden-branch` vs the main timeline, which matches the reference exactly): max abs diff ramp 7.9, chart 11.8, out-of-range 8434, natural 271; the viewer is visibly blown out. Bisecting: disabling only the CST makes the render bit-identical to main; disabling `sender-hidden` or the mixer does not. So the CST is in the delivered signal path, and neither key output gain 0 nor the mixer as built isolates it.
4. **Playback cost?** None measurable at 1080p24 (96/96 frames rendered; ~39.9 ms between new frames for both timelines, 24 fps = 41.7 ms; probe copy ~3-5 ms). Rough: playback is capped at 24 fps so spare headroom isn't measured; UHD belongs to #12.

**Variants needing a human** (scripts can't create nodes or set key gain):
1. Open the timeline graph and note the real wiring.
2. Set key output gain on `sender-hidden` to 1 temporarily and dump, to see whether gain 0 blanks the probe's source.
3. Try a Layer Mixer (or a mixer with the hidden branch's weight at 0) instead of key gain 0, so the probe still sees the branch but it never reaches the output.
4. Try the CST wired off the main path so only the hidden node's input comes from it.

Unverified: wiring; whether key gain really is 0; scratch .drp round trip fidelity; playback dropped-frame counters (not reachable by script).

Note: #10 already shows published maths is exact for every tested setup except DaVinci tone mapping / gamut mapping on and ACES 1.3, so the hidden branch's remaining value is covering those.


