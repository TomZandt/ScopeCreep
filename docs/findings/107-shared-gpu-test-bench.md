# #107: one shared GPU test bench

What ticket #107 showed on 2026-11-18.

- The GPU setup (device, playing synthetic and ColorChecker frames through the Feed, drawing Panels, reading pixels and counts) that was copied into 16 display test files now lives in `app/tests/common/mod.rs`. About 1,600 lines went from the files and about 450 were added.
- Each file's assertions are unchanged; the suite passed before and after.
- GPU tests can fail with `RequestDeviceError(OutOfMemory)` when Resolve and the desktop hold most of the card's memory (see #95); that is the machine, not the bench.
