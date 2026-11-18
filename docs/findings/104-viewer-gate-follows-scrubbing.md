# #104: the viewer gate follows a scrub

What ticket #104 showed on 2026-11-18 by replaying the scrub recorded for #103 through the gate (`app/src/viewer_gate.rs`, pure seam, no GPU). The recording (Resolve 21.2, UHD, 50 fps, `SCOPECREEP_FRAME_LOG`): 3692 Feed frames over about 134 s of playback, slow and fast mouse scrubs, a stop mid-drag, J reverse, shuttle and frame steps. The 6004-line log is `~/scopecreep-testdata/scrub/frames.log`, outside the repo; a 277-frame excerpt is the replay test's data. The replay offers each frame at its read time and ticks the gate every 5 ms.

## What the recording shows

- **Scrub renders come 13 to 25 ms apart**, jumping 1 to 3 frame numbers each (fast drags), or 250 ms to 1.5 s apart (slow drags, Resolve renders each frame slowly). Playback steps are 20.0 ms; shuttle at 2x steps 2 frames every 20 ms.
- **The old gate** showed 2316 of 3692 frames, 144 gaps over 100 ms between shown frames, the longest 8.2 s. 1257 frames waited over 45 ms, many for seconds.
- **The cause was twofold.** A new run's frame was held until the run ended or was verified. Worse, fast drags and shuttle were judged cache fills: the fill test measured the time per frame number, so 3 frames in 15 ms looked like 5 ms a frame. A 2x shuttle at 20 ms a render looked like 10 ms a frame.

## The change (ADR 0031)

- A run that gets past four renders without being judged a cache fill is a drag (or playback): from then on every frame is shown at once, the stop frame included.
- Fills are judged per render, not per frame number, and a fill must average under 12.5 ms a render (it was 20 ms) as well as under 0.83 of a period. Fills in the existing seam tests render every 12 ms; the scrub's windows run 13 to 25 ms.

## Result on the same recording

- 3313 of 3692 frames shown (was 2316); 176 frames waited over 45 ms (was about 1250) and 88 over 80 ms. What is left is the first five renders of every new run (about 60 to 75 ms at 13 to 15 ms a render, the price of telling a fill from a drag) and the stretches rendered 7 to 12 ms apart, which stay hidden as cache fills.
- Gaps over 100 ms between shown frames are 142 (was 144, longest 8.2 s): they are where the Sender itself rendered nothing new, such as a slow drag (one render every 250 ms to 1.5 s) or a stationary playhead.
- The stop frame is shown within 25 ms of arriving.
- Unchanged: the existing seam tests (cache fills at 12 ms, in pairs and jumping back; playback; steps) pass as written; a synthetic fill (HD, 24 fps, 6 ms a frame) shows nothing; playback at 25 fps shows every frame once verified; a lone step is shown after the hold.

## Not verified

- The recording holds no deliberate cache fill: the fill rule is checked by the synthetic fill and by the three fill-like stretches. A fill at UHD that renders slower than 12.5 ms a frame is shown (the known limit, widened). A person at Resolve should record one (#139).
- How a followed scrub looks in the Window at real sizes.
