# #35: when GPU memory runs out

What ticket #35 showed on 2026-11-20, with the fake CUDA host (`make -C sender test`) and at the Feed seam (`app/tests/feed_loop_seam.rs`), on the RTX 3070 Ti with Resolve open. Nothing was checked with the GPU really full: both sides were made to run short by a cap on allocation size (below).

Why it matters: Resolve can hold about 7.7 of 8 GB while it plays UHD (finding 12), and colourists run other GPU apps beside it. On 2026-11-15 the desktop reported GPU memory full with dictation workers (about 2.9 GB) and an editor (about 1.2 GB) open; scopecreep itself used about 70 MB idle.

## The Sender

- Its GPU memory is the staging buffers (one frame each, 133 MB at UHD float; two by default). They are made on the Sender's worker thread, never in a render call, and the picture has already been passed through on Resolve's stream by then, so a failure costs Resolve nothing: the render call returned in under 5 ms and the picture passed through bit for bit while short.
- The reason goes in the Feed header's existing `sender_error` field (protocol 1.0, offset 512). No protocol change was needed: the field was made for this, and the app already shows it as "Sender: …". The text is now one sentence a colourist acts on: "Not enough GPU memory in Resolve to send frames (needs about 133 MB). Close another GPU app; the Sender tries again every few seconds." The amount is one staging buffer, the least the Sender can work with. Other CUDA errors keep their technical text.
- While short, the Feed it had stays (frames that fit it still flow; larger ones are counted as dropped, Sender busy), and it tries again 5 s after the failure. In the test, once the cap was lifted the next render after the retry time built the ring, the new Feed had no error, and frames arrived bit for bit.
- Test hook: `SCOPECREEP_SENDER_GPU_LIMIT_MB` in the Sender's environment makes a staging buffer bigger than that fail as if the GPU were full (read on the worker thread each time a ring is built).

## The app

- Its large allocations are the frame textures (two staging buffers, the frame, the converted frame: about four frames, 530 MB at UHD) and the waveform's buffers (counts, colour sums, channel counts: 4, 12 and 12 bytes per column per code value, up to 110 MB at UHD with everything on). Each is made inside a wgpu `OutOfMemory` error scope (`app/src/gpu_memory.rs`), so a failure comes back as "no memory" instead of an invalid resource used later.
- Short of memory for the picture, the frame is left out: nothing is measured, the last picture and Scopes stay on screen. Short for the waveform, it lets its buffers go and draws nothing; the picture and other Scopes carry on. Either way the next frame tries again, so the app carries on by itself as soon as memory frees up (with Resolve stopped, that is the next frame Resolve renders).
- The Window says it in the notice, before any Feed notice: "Not enough GPU memory for the picture (needs about 266 MB). Close another GPU app; scopecreep will carry on when memory frees up." ("the scopes", or "the picture and the scopes"). The amount is what failed in the last frame (266 MB is a UHD frame's two staging buffers), rounded up to whole MB. The notice goes with the first frame that gets everything it needs.
- Test hook: `SCOPECREEP_GPU_MEMORY_LIMIT_MB` (or `gpu_memory::set_limit` in a test, per thread) makes an allocation bigger than the cap fail as if the GPU were full.
- The forced failure skips the allocation; the error-scope path with a real out-of-memory from the driver was not exercised (doing so would mean filling the GPU Resolve uses).

## Not checked

- The real Sender in Resolve with GPU memory exhausted: that Resolve keeps playing undisturbed, the Window shows the Sender's sentence, and frames resume once memory frees up.
- The real app with GPU memory exhausted (an out-of-memory reported by the driver through the error scope, not the cap), and the Window's notice at the app (`SCOPECREEP_GPU_MEMORY_LIMIT_MB=50 scopecreep` with UHD playing shows it without filling the GPU).
