# #253: Resolve's Metal hand-off on the Mac

What the probe (`prototype/probe-metal/`) showed on 2026-11-27: Mac mini (Mac16,10), Apple M4, macOS 26.6.2, DaVinci Resolve 21.1.1.10 (OFX host API 1.4). UHD 50 project, probe on the timeline node. This Mac could not hold UHD 50 playback, so the copy timings were taken under load.

## How frames arrive

- Resolve's GPU processing offers only Metal on this Mac (no Auto/Metal choice). All 2.7k renders had `metalEnabled=1`, `cudaEnabled=0`, `openclEnabled=0`. The host-level `MetalRenderSupported` read false on the first Load of a session and true on the second.
- `src` and `dst` are `MTLBuffer`s (class `AGXG16GFamilyBuffer`), 128 MiB for a 3840x2160 RGBA float frame (rowbytes 61440), `storage=private`, `contents=nil`. **The CPU cannot read the source**, so a direct CPU copy is impossible.
- One command queue (`AGXG16GFamilyCommandQueue`), the same pointer on every render. Resolve sends Begin/EndSequenceRender around each render. Thumbnail renders (288x162) also arrive on Metal.
- Unified memory (`unifiedMemory=1`).

## Copy cost, 30 full-size copies of 132,710,400 bytes

| Route | Median | Max |
|---|---|---|
| A: blit to our own shared buffer, then CPU memcpy into the shm | 12.3 ms (blit 4.6, memcpy 8.1) | 104 ms (cold pages, 1.3-2 GB/s) |
| B: blit straight into a buffer wrapping the shm | **4.2 ms** (about 30-35 GB/s) | 9.9 ms |
| C: CPU memcpy from the source | impossible (private storage) | |

- `newBufferWithBytesNoCopy` on the mmap'd shm worked every time (0.111 ms first wrap, then none; 120 copies, no nil). The shm must be page-rounded; pages are 16 KiB.
- The passthrough alone costs 0.087 ms to encode and commit; the whole render action took a median 0.632 ms (p95 1.456 ms).
- The probe's copies waited for the GPU, which is why playback stuttered. The Sender must not wait; the completion-handler or fence design was **not measured**.

## Recommendation for the Sender's Metal path

Blit straight into the Feed slot (route B), with a completion handler publishing the slot, never waiting in the render call. It fits the 20 ms budget at 50 fps with room; route A spikes past it.

## Lessons for the Sender

- Answer CreateInstance and DestroyInstance with `kOfxStatOK`. Falling through to `kOfxStatReplyDefault` is a failed create to Resolve: it retried about 500 times and never rendered.
- The timeline node asks for the **General** context. Declare Filter and General (as `sender/src/sender.cpp` does); Filter alone fails with "Context ... General is not supported".
- `kOfxImageClipPropThumbnail` is in Resolve's own `ofxImageEffectExt.h`, not upstream openfx. The SDK is at `/Library/Application Support/Blackmagic Design/DaVinci Resolve/Developer/OpenFX/OpenFX-1.4/include`.
- `/Library/OFX/Plugins` did not exist before installing.
