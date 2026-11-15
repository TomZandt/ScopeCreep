// The Sender's frame transport: Resolve's frame on the GPU -> the Feed (shared memory), without ever waiting.
//
// Everything that touches CUDA or the Feed lives behind this interface, so the OFX glue (sender.cpp) needs
// neither. The strategy is the one Stage 0 proved (docs/stage0-report.md, section 4):
//   render call:  copy the frame into a small private staging buffer on Resolve's own CUDA stream (cheap, GPU to
//                 GPU), queue an asynchronous copy from staging into a pinned Feed slot on the Sender's stream,
//                 queue a callback that publishes the slot, and return. Nothing is waited for.
//   no reader:    nothing at all is queued.
// ADR 0003: no call here may block on the app, on a lock, or on a full ring.
#pragma once
#include <cstddef>
#include <cstdint>

namespace scopecreep {

struct FrameInfo {
    int width = 0, height = 0;
    double frameTime = 0;   // source frame number, as the host gave it
    double frameRate = 0;   // frames per second of the timeline, 0 = unknown
    uint32_t instance = 0;  // which Sender instance
    uint64_t renderNs = 0;  // monotonic time the render call began
    // The Colour Setup set by hand on the Sender node (Feed protocol 1.1 text), or provenance 0 for none.
    uint32_t colourProvenance = 0;
    const char* colourSetup = nullptr;
    size_t colourSetupBytes = 0;
};

enum class Render { Fed, Idle, Thumbnail, Unsupported };  // what a render call turned out to be, for the Feed's counters

// Publish the Feed (with no ring yet) so an app can find the Sender. Safe to call repeatedly and from any thread.
// If the Feed already belongs to another running Resolve, this process joins it as a guest instead: its
// instances are listed in the Feed's registry and their renders recorded, but it never copies a frame
// (feed/PROTOCOL.md, "More than one Sender").
void feedOpen(const char* hostName);

// Is this process a guest in another Resolve's Feed?
bool feedIsGuest();

// A Sender instance was created / destroyed: list it in, or take it off, the Feed's registry. Not on the render
// path (may wait briefly for the Feed's lock).
void feedRegister(uint32_t instance, double frameRate);
void feedUnregister(uint32_t instance);

// Record a non-thumbnail render call of `instance` in the registry (a few atomic stores). About once a second a
// guest also checks, without waiting, whether the owner has replaced its Feed or quit (then this process takes
// the Feed over).
void feedNoteRender(uint32_t instance, double frameTime, uint64_t renderNs);

// Is an app attached to the Feed right now? One atomic load and a clock read.
bool feedReaderPresent();

// Start the asynchronous copy of a 32-bit float RGBA frame that lives on the GPU (`src` is a device pointer, rows
// `srcRowBytes` apart) into the Feed, and return at once. Call only when feedReaderPresent().
// Returns true if a copy was started. Otherwise the frame was not fed and the Feed's counters or `sender_error`
// say why (ring being built, everything busy, out of graphics memory).
// The first call with a reader attached builds the ring: a one-off cost of pinning the shared memory.
bool feedSubmitCuda(void* hostStream, const void* src, size_t srcRowBytes, const FrameInfo& info);

// Count one render call in the Feed's counters.
void feedCount(Render what, uint64_t renderNs, uint64_t callNs);

// Tell the app something is wrong (shown in the app), or clear it with "".
void feedSetError(const char* text);

// The picture passes through untouched: dst = src on the host's CUDA stream. Never blocks.
bool cudaPassthrough(void* hostStream, void* dst, size_t dstRowBytes, const void* src, size_t srcRowBytes, size_t widthBytes, size_t height);

}  // namespace scopecreep
