# On macOS the Sender blits each frame straight into the Feed slot

Date: 2026-11-27 (#255, spec #236). Follows ADR 0003; measured in `docs/findings/253-metal-handoff.md`.

Resolve on a Mac gives a plugin Metal buffers with private storage: the CPU cannot read them, the way a letter in a locked post box cannot be read by anyone but the postman. So the Sender cannot copy a frame itself; it has to ask the GPU to carry it. The question was where the GPU should put it.

## Decision

- The Metal path lives in the same Sender as the CUDA path, chosen by what Resolve offers on the platform (Metal on a Mac, CUDA on Linux and Windows): one source tree, one build per platform, the OFX glue in `sender.cpp` the same.
- The Feed's memory (shared memory, page aligned) is wrapped once per ring in an `MTLBuffer` with `newBufferWithBytesNoCopy`. Each Viewer Frame is one blit from Resolve's source buffer straight into its slot, encoded on Resolve's own command queue and committed, and the command buffer's completion handler publishes the slot. The render call returns at once; nothing waits (ADR 0003). Measured on an M4 at UHD, this took 4.2 ms against 12.3 ms for a blit into our own buffer followed by a CPU copy. The probe's one 100 ms spike was cold pages; the Sender touches the ring's pages on its worker thread when it wraps them (not yet measured).
- There is no staging buffer, so no staging memory to run short of and no CUDA-style "not enough GPU memory" state. The Feed's slots are the backpressure: when all four are being written the frame is dropped and counted, as on CUDA.
- What is the same on both GPU paths is shared, not copied: thumbnail gating, the last-Sender-in-frame rule and the registry (ADR 0024), the guest rule, the counters, ring growth and the worker thread are in `sender/src/feed_core.cpp`. A GPU backend (`feed_cuda.cpp`, `feed_metal.mm`) supplies three things: what to reserve for a ring, how to tie the Feed's memory to the GPU (pin it, or wrap it), and how to start the copy of one frame.
- The Sender links only Metal and Foundation (system frameworks), for arm64 only, and is built by `make -C sender` on the Mac.

## Considered and rejected

- **Blit into our own shared buffer, then a CPU copy into the Feed** (route A in the finding): 12 ms median, 104 ms worst when pages were cold, over the 20 ms budget at 50 fps on a bad frame.
- **A CPU copy from Resolve's buffer:** impossible, the storage is private.
- **Waiting for the GPU in the render call** (what the probe did to time its copies): it made playback stutter.

## Consequences

- The completion-handler design was not measured in the probe; the UHD 50 check on the Mac (#257) is what shows it holds Resolve's frame rate.
- The Feed's size on macOS is rounded up to a page (16 KiB) by the operating system; the wrapped buffer's length is rounded the same way.
