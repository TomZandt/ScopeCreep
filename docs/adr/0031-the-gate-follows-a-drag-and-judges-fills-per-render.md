# The gate follows a drag, and judges a cache fill by the time per render

The viewer gate decides from pacing alone which Feed frames are Viewer Frames (Resolve never says which renders are the playhead's, ADR 0003). It held a new run's newest frame back until the run was verified as playback or ended, so a drag of the playhead showed nothing until it stopped. The scrub recorded for #103 (UHD, 50 fps, 3692 Feed frames over about 134 s) also showed a second fault: fast drags jump up to three frame numbers per render and were judged cache fills, because the fill test measured time per frame number.

## Decision

- **A drag is followed.** A run that gets past four renders without being judged a cache fill is shown from then on, every frame at once, the stop frame included. A single step still waits the hold.
- **A cache fill is judged by the time per render, not per frame number.** A fill renders one frame per render as fast as the GPU allows; a drag renders no faster than about 13 ms a render at UHD whatever the jump. The fill's limit is 12.5 ms a render (it was 20 ms; the 0.83-of-a-period limit stays), and a fill must still be ahead of its own clock.
- Cache fills stay hidden: a fill is judged by its fifth render, before the drag rule shows anything.

## Considered and rejected

- **Show every non-fill frame at once:** a fill's first frames would flash before it is recognised.
- **Keep the per-frame-number limit and show drags regardless:** a drag judged a fill is exactly what hid the picture; the two judgements cannot both stand.

## Consequences

- A fill that renders at 12.5 ms a render or slower (heavy grades, UHD) is shown like a drag. This widens the known limit in the gate's header; telling those apart needs the playhead (#16).
- The replay test (`viewer_gate.rs`, from an excerpt of the #103 recording) guards both rules. See `docs/findings/104-viewer-gate-follows-scrubbing.md`.
