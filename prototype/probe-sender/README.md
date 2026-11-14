# Probe Sender (PROTOTYPE, throwaway) - issue #4

Stage 0 check 1: does Resolve's OFX colour management tell a plugin the colour space of each frame?
Only the findings carry forward; this code does not.

    make            # build scopecreep_probe.ofx.bundle
    make install    # copy to /usr/OFX/Plugins (restart Resolve afterwards; it scans plugins at startup)
    make test       # offline: EXR bit-exact round trip + fake-host drive of the built plugin
    make cuda-check # toolchain smoke test (nvcc at /opt/cuda)

Log: `/tmp/scopecreep-probe/probe.log` (override dir with `SCOPECREEP_PROBE_DIR`).
Frame dump: `touch /tmp/scopecreep-probe/dump.trigger`; the next render writes
`frame_inst<N>_t<time>_<W>x<H>.exr` (32-bit float RGBA, uncompressed, bit-exact; rows flipped to top-down).
Colour style declared to the host: `SCOPECREEP_PROBE_STYLE=ocio|full|core|basic|none` (default ocio),
set in the environment Resolve is launched from.
Script: `python3 scripts/apply_probe.py [--apply]` lists the effect (and applies it with `--apply`).

## Issue #12: async GPU frame copy into shared memory (cost of copying UHD float out of Resolve)

Still a throwaway. With `SCOPECREEP_PROBE_CUDA=1` (set in the environment Resolve is launched from) the probe opts into
Resolve's CUDA render path (images arrive as device pointers), passes the picture through with a device copy on Resolve's
stream, and on every non-thumbnail render starts an asynchronous copy of the whole 32-bit float RGBA frame into a slot of a
shared-memory ring (`/dev/shm/scopecreep-probe-ring`, pinned with `cudaHostRegister`, own CUDA stream, `cudaMemcpyAsync`,
completion learned from a host callback). It never waits on the copy or on a reader: a full ring overwrites the oldest unread
slot and counts a drop; if nothing can be taken the frame is skipped and counted. Code: `src/cuda_copy.cu`, layout and
protocol: `src/ring_layout.h`. Env knobs are listed at the top of `src/probe_sender.cpp`
(`SCOPECREEP_PROBE_COPY`, `_COPY_MODE=staged|direct`, `_SLOTS`, `_STAGING`, `_CHECKSUM`, `_QUIET`, `_TRACE`).
`touch /tmp/scopecreep-probe/copy.off` switches the copy off at run time (passthrough continues); `rm` switches it on.

    make                 # plugin (static cudart, no OpenEXR) + build/probe_reader
    make cuda-test       # offline: fake renderer -> same copy code -> reader verifying GPU checksums (no Resolve)
    build/probe_reader --verify --touch --exr-dir /tmp/scopecreep-probe --seconds 20
                         # --verify: CPU vs GPU checksum (needs SCOPECREEP_PROBE_CHECKSUM=1);
                         # --exr-dir: bit-exact compare with dump-trigger EXRs of the same frame time; --touch: read every byte

`COPYSTATS` lines in the log (once a second) give renders/s, time in render, copy GPU time (cudaEvent), submit-to-completion
latency and drop counters; `SCOPECREEP_PROBE_TRACE=1` writes one CSV line per render to `render_trace.csv`.
Measurement scripts and results: see the closing comment on issue #12.
