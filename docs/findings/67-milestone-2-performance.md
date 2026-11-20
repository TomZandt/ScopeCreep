# #67 Performance with every Panel shown: does the app slow Resolve down at UHD 50?

What ticket #67 measured on 2026-11-17. DaVinci Resolve 21.2 Studio on Linux, NVIDIA GeForce RTX 3070 Ti, the Sender unchanged since v0.1.0 (probe-alias build), the app built from `main` at c6bc626 with all 14 Panels (release build). Method as in [#20](20-performance-benchmark.md): `harness/experiments/uhd50_bench` (`build.py`, then `run.py`), UHD 3840×2160 at 50 fps only, loop playback on, 15 s warm-up and 60 s measured per case, Resolve's own fps readout captured every 5 s.

## Results

| Case | Resolve fps readout (12 samples) | Frames through the Sender | App |
|---|---|---|---|
| Sender off | 50, green, every sample | n/a | not running |
| Sender on, no app | 50, green, every sample | 49.99 per second | not running |
| Sender on with the app, all 14 Panels shown | 50, green, every sample | 49.99 per second | read 3,775 frames, no gap in the sequence; publish to read median 0.52 ms, 99th percentile 1.05 ms, maximum 6.09 ms |

**Pass.** Resolve played UHD 50 at 50 fps in every case, with the Sender and with the Sender and the app showing every Panel (ADR 0003). The screenshot at the end of the app run shows all 14 Panels drawing.

The fps readout crops of all 36 samples are byte-identical, so Resolve showed the same "50" and green dot throughout.

## Not measured

Each Panel's own GPU time per UHD frame (user story 85) was not measured here; it is tracked in #82. The app keeps up with every frame, so no Panel is known to break the bar.

## Notes

- The benchmark project `scopecreep-test-bench50` is kept in Resolve's scopecreep folder, and exported to `~/scopecreep-testdata/drp/scopecreep-test-bench50.drp` for next time.
- Raw captures: `~/scopecreep-testdata/bench/uhd50-20261117-190058/` (not in git).
- `run.py` now tolerates an existing output folder (`os.makedirs(..., exist_ok=True)`); the first run in a session tripped over one.

## Re-run on v0.3.1 (2026-11-20)

The release build of v0.3.1 (7 Panels, the joined Vectorscope with both target sets, the menu bar), same project and method. **Pass**: Resolve's readout showed a green 50 in all 36 samples (byte-identical crops), the Sender rendered 49.99 frames a second with and without the app, and the app read 3,775 frames with no gap in the sequence (publish to read median 0.54 ms, 99th percentile 3.53 ms, maximum 23.6 ms). Raw captures: `~/scopecreep-testdata/bench/uhd50-20261120-011837/` (not in git).

- Resolve must be visible on a monitor for the run. Two first attempts with Resolve on a workspace that wasn't shown played nothing (the Sender saw about 1 render a second, the "off" case can't tell). The run that passed moved Resolve to the left 4K monitor, where `run.py`'s default fps-readout crop (1395,255) sits; `SCOPECREEP_BENCH_GRAB` now sets the crop when Resolve is elsewhere.
