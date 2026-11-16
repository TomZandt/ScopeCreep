# #19 Playback rate, dropped frames, more than one Sender, and cache fills while paused

What ticket #19 learned by testing DaVinci Resolve 21.2.0.6 Studio on Linux (RTX 3070 Ti), with the app and the real Sender (probe-alias build of this branch, Feed protocol 1.2) on the timeline node. Every Resolve step ran under `flock -o ~/scopecreep-testdata/resolve.lock`; playback was driven by keystroke (L, K) with no scripting while playing. The app's per-frame log (`SCOPECREEP_FRAME_LOG`) recorded every Feed frame's Sender render time, frame number and the app's decision.

Projects: scratch copies `scopecreep-test-s19-24`, `-25`, `-50`, `-60` (the yrgb test project's .drp with the HD `natural` clip x20, the timeline node carrying the Sender, timeline rate set by script), and `scopecreep-test-s19-two` (an import of the dwg-p3d65 .drp; the original project was never opened).

## The playback rate is Resolve's, within ±0.01 fps at 24, 25 and 50

15 s plays from the timeline start, app started with `--no-scripting` (see the last section). The app's reading once a second, against an independent check (frame numbers advanced over the Sender's render times, from frame 30 to 5 frames before the stop):

| Timeline | App's reading in steady playback | Independent pace |
|---|---|---|
| 24 fps | 23.997 – 24.005 | 24.007 |
| 25 fps | 24.997 – 25.002 | 25.019 |
| 50 fps | 49.994 – 50.004 | 50.003 |
| 60 fps | 57.99 – 58.09 (58.4 in its first second) | 58.13 |

- **At 60 fps Resolve itself plays at about 58 fps** on this HD project: steady 16.7 ms renders, with a stall of about 50 ms (three periods) once a second; it does not catch up. The reading shows that, as it should.
- The reading appears about 3.5 s after play is pressed (the first 0.5 s of a run is left out and 2 s are needed for ±0.1 fps) and gives way to "paused" 0.5 s after the last frame.
- **Resolve renders now and then 10–30 ms off the frame clock**, and a cold playback start renders the first second **out of order and in bursts** (e.g. frames 2, 4, 6, 7, 5, 8, 11, 10, 9 a few ms apart, gaps of up to 190 ms). The first version of the rate restarted at every bunched render and blanked for a second at a time; the fit now runs through them.

## Dropped frames

- 0 dropped in the 24, 25 and 50 fps plays; 1 at 50 and 60 in some runs (`Sender busy`: a staging copy still in flight), 2 (`app too slow`) once in a 50 fps cache fill.
- With two Senders on one timeline both are copied, and 4 `Sender busy` drops were counted in 8 s at 24 fps HD.
- The split ("app too slow" = overwritten before the app read it, "Sender busy" = skipped without waiting) is covered by synthetic tests that overfill the Feed (`app/tests/feed_seam.rs`).

## Two Senders on one timeline (real Resolve)

`scopecreep-test-s19-two`, timeline `hidden-branch`: the timeline node's Sender and a second one in a branch mixed in at 0% (whose frames are all zeros, #9). Both render every frame, about 2–10 ms apart; Resolve numbered them 57 (timeline node) and 58 (branch).

- **The app showed only Sender 57**, every frame (198 frames read from each), and flagged the other: "Another Sender is active: Sender 58 (process 6279). Showing Sender 57 (process 6279) only. Keep one Sender, on the last timeline node.".
- The rule is "lowest instance number", and Resolve numbers instances in the order it creates them. Here the timeline node's came first; nothing guarantees it.
- **Found and fixed:** in the first run the branch Sender's very first frame (black) was shown for 0.2 s at the cold playback start, because it arrived 2 ms after the timeline node's first frame and before the two had been seen in step. A Sender's first frame now cannot take the Feed from a driver that rendered within half a period.
- The Sender registry (protocol 1.2) worked in Resolve: each instance appeared in the header with its timeline rate, host name and a render count. A second Resolve process was not tried (Resolve did not run twice); the guest path is covered by the mock-host test and two-process synthetic tests.

## Cache fills while paused

Paused on frame 11 with project cache Smart and every clip node's cache Enabled (`SetNodeCacheMode`), then a unique Pan set on every clip: Resolve re-rendered frame 11 within 4 ms (the new picture) and about a second later filled its cache for the whole timeline.

| Timeline | Cache renders | Pace | Shown by the app |
|---|---|---|---|
| 24 fps | 480 in 3.4 s | 140/s, 5.8x real time, 7 ms each | none: picture stays on frame 11 |
| 50 fps | 1006 in 9.5 s | 106/s, 2.1x real time | none (final gate) |
| 60 fps | 1043 in 11.1 s | 94/s, 1.6x real time | none (final gate) |

- **At 50 and 60 fps the fill renders from two threads**: mostly in pairs (two frames under a millisecond apart, then 10–30 ms), each frame rendered about twice (1006 renders for 480 frames), and now and then one thread re-renders a few frames a little way back. Averaged over a few frames it sometimes slows to real time. The pairs never stop.
- #14's gate (a run of 4+ frames each under half a period apart) missed all of this at 50 and 60: the picture ended on frame 856 / 802 instead of 11. Successive gates were tried against these traces; the one shipped judges a run's last 8 frames: faster than 0.83 of real time and ahead of the run's own clock is a fill; it is playback again only when its last 8 frames came one at a time at the frame rate; a short run cut off a fill is part of it. With it, none of the fill was shown at 24, 50 or 60.
- **Limit (as #14 predicted, now narrower):** a fill that renders no faster than about 0.83 of real time, one frame at a time, cannot be told from playback by pacing. Not seen on this machine at HD; UHD or heavy grades at 50/60 fps were not tried.
- The Viewer Frame after an edit is a repeat render of the paused frame and is shown at once; a step during a fill may be held until the fill ends.

## Playback starts and the gate

- A cold start (above) is ragged; the gate holds the start until the run has settled (up to about 0.4 s at 24 fps here), then shows every frame: 353 of 365 frames shown at 24, 742 of 753 at 50, 867 of 879 at 60.

## Reading the project during playback starves Resolve

With the app's scripting helper on (#16), two 24/25 fps plays went wrong: in one, Resolve played 24 frames in 12 s (8 project reads were made during the attempt); in the other no steady rate appeared for 8 s. The helper reads after 1 s with no render, and a cold playback start in Resolve can render nothing for longer than that (up to 5.7 s seen), so reads land on playback starts. The measurements above were made with `--no-scripting`, which played normally every time.

