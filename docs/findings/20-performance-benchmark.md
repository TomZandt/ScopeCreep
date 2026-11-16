# #20 Performance benchmark: does the Sender or the app slow Resolve down at UHD 50?

What ticket #20 measured on 2026-11-16. DaVinci Resolve 21.2 Studio on Linux, NVIDIA GeForce RTX 3070 Ti (driver 615.78.08), Sender built from `main` after the review fix 53e2c9d (ring build and guest scan off the render thread), probe-alias build (`make -C sender PROBE_ALIAS=1 install`).

## Scope

UHD (3840×2160) at 50 fps only, by the product owner's decision: it is the heaviest PAL case, so if Resolve keeps up here, 25 fps is covered. The question is only whether there is a performance drop with the Sender, and with the Sender plus the app.

## Method

- One project in the `scopecreep` folder, built by `harness/experiments/uhd50_bench/build.py`: YRGB, Rec.709 Gamma 2.4, render cache off, six copies of the UHD float EXR test clip (30 s), on two timelines: one with the Sender on its timeline node, one ("no-sender") with the same clips and no node.
- Resolve's loop playback on. For each case `run.py` starts playback by keystroke, lets one 15 s pass load the media into memory, then measures for 60 s. No scripting during playback.
- **Sender off**: the no-sender timeline. Measured by Resolve's own fps readout in the viewer, captured every 5 s (it can't be read by script). The Sender's on/off switch (`Graph.SetNodeEnabled`) worked in one session and returned nothing in the next, so a second timeline is used instead.
- **Sender on, no app**: Resolve's fps readout, plus the Sender's render count from the Feed header (frames Resolve actually rendered through the Sender).
- **Sender on with the app**: as above, with the app running on another monitor and `SCOPECREEP_FRAME_LOG` recording every frame the app read.

## Results

| Case | Resolve fps readout (12 samples) | Frames through the Sender | Drops |
|---|---|---|---|
| Sender off | 50, green, every sample | n/a | none shown |
| Sender on, no app | 50, green, every sample | 49.99 per second | none |
| Sender on with the app | 50, green, every sample | 49.99 per second (frame log: 50.00) | app missed 0 of 3,774 frames; no gap between renders over 29.3 ms |

**Pass.** Resolve played UHD 50 at 50 fps in every case: the Sender and the app cost Resolve nothing measurable (ADR 0003). The loop's wrap from the end of the timeline to the start caused no visible hitch.

An earlier run the same evening with the Sender from before 53e2c9d gave the same result (50 everywhere, app missed 0 frames).

## Notes

- Resolve's own dropped-frame count isn't scriptable; the green readout (red when Resolve can't keep up) and the Sender's continuous frame numbers stand in for it.
- A new app window on Resolve's monitor covers Resolve, which then stops playing; the script moves the app to another monitor.
- Results (not in git): `~/scopecreep-testdata/bench/uhd50-20261116-211512/`.
