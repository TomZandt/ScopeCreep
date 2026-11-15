# #12 Stage 0: Cost of copying UHD float frames out of Resolve

> Copied from the GitHub issue comments on TomZandt/scopecreep#12 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-14)

Stage 0 check 6 (Resolve 21.2.0.6 Studio, RTX 3070 Ti, PCIe 4.0 x16). The probe opts into Resolve's CUDA render path (`SCOPECREEP_PROBE_CUDA=1`) and, per non-thumbnail render, starts an async copy of the full UHD 32-bit float RGBA frame into a pinned shared-memory ring (`/dev/shm`, 4 slots, `cudaHostRegister` + `cudaMemcpyAsync` on its own stream, completion published by `cudaLaunchHostFunc`) and returns. It never waits; a full ring overwrites the oldest unread slot and counts a drop. Code: `prototype/probe-sender/` (incl. out-of-process reader, `make cuda-test`); scripts: `harness/experiments/copy_cost/`.

UHD float RGBA, YRGB, render cache off, real-time playback, 20 s per run, copy off vs on vs on with a reader touching every byte (2 reps each):

| fps | off: rate, dropped | on | on+reader | probe time in render off / on | PCIe from GPU (on) | GPU SM off / on |
|---|---|---|---|---|---|---|
| 23.976 | 23.97, 0 | 23.97, 0 | 23.97, 0 | 0.043 / 0.086 ms | 3.5 GB/s | 16 / 27% |
| 25 | 25.00, 0 | 25.00, 0 | 25.00, 0 | 0.042 / 0.077 ms | 3.5 GB/s | 15 / 28% |
| 50 | 50.00, 0 | 50.00, 0 | 50.00, 0 | 0.039 / 0.069 ms | 6.9 GB/s | 31 / 49% |
| 60 | 59.79, 0 | 59.78, 0 | 59.77, 0 | 0.039 / 0.070 ms | 8.7 GB/s | 32 / 60% |

- Copy GPU time ~5.7-6.5 ms (~23 GB/s, near PCIe 4.0 x16 line rate); submit-to-complete latency ~9-10 ms mean, up to 45 ms max. p99 render time <= 0.15 ms. The very first copy costs ~66-228 ms (pinning 530 MB). Staged vs direct modes behave the same; direct costs more GPU.
- **Frames are intact:** 2,444 frames passed a GPU checksum checked out-of-process (0 bad); 4 UHD frames from the ring were bit-identical to Resolve's own dump EXRs; CPU-path and CUDA-path frames are byte-identical.
- **Free-run** (uncapped Deliver, 600 frames, DNxHR LB, 3 reps): 60.4 fps copy off, 51.8 on, 49.5 on + reader: ~14-18% throughput cost when saturated, zero in real-time playback. Likely encoder-bound, so not a pure GPU ceiling.
- **Without CUDA** (Resolve's CPU path) the probe took ~13 ms/render (synchronous GPU->host copy) and UHD playback fell to ~30 fps on 50/60 fps timelines: Resolve slowed instead of dropping. **The CUDA path is required for ADR 0003 at UHD.**

**ADR 0003 ("Resolve never waits") is achievable at UHD float, 23.976-60 fps, with no fallback needed** on this machine: ~8.7-9.3 GB/s of ~23 GB/s PCIe at 60 fps, +~28 points GPU utilisation. Fallbacks (half-float, GPU-side reduction, GPU-to-GPU sharing) are for weaker GPUs/higher load; their accuracy cost wasn't measured because they weren't needed.

Constraints for the real Sender: **VRAM is the tight resource** (Resolve held 7.6-7.8 of 8 GB; two staging buffers add 265 MB); the Sender must **skip the copy when no reader is attached** (the probe copies regardless); don't call Resolve scripting during playback (it starves the main thread for minutes); Resolve caches OFX plugin descriptions keyed on the binary's mtime, so env-var changes only apply after the binary is rebuilt/touched.

**Unverified:** Resolve's own playback/dropped-frame counters (rate and drops are inferred from the probe's per-render trace, so frames dropped after the probe wouldn't show); > 60 fps; grades before the sender node; cache-fill bursts at UHD; long runs; reader crash (slot stays "reading" until ring restart, writer just drops); colour-managed projects (only YRGB tested); multiple probe instances; GPU/PCIe numbers are coarse 1 s `nvidia-smi dmon` averages.


