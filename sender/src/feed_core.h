// The part of the Sender's frame transport that is the same on every GPU API, and the seam the GPU-specific parts
// plug into (feed_cuda.cpp on Linux and Windows, feed_metal.mm on macOS). Internal: sender.cpp sees only feed_copy.h.
//
// feed_core.cpp owns everything that is not about moving pixels: publishing the Feed, building and retiring rings
// on the worker thread, the registry and the guest rule (ADR 0024), the counters, the error line, and the part of
// a frame submission that is the same everywhere (try-lock, ask for a larger ring, take a slot, write its header,
// publish when the copy has finished). A backend provides only the three things that differ: what to reserve for
// a ring, how to tie the Feed's memory to the GPU, and how to start the copy of one frame. ADR 0003 applies to all
// of it: nothing here or in a backend may wait for the GPU, the app or a lock on the render path.
#pragma once
#include "feed_copy.h"

#include <cstddef>
#include <string>

namespace scopecreep {

// What a backend keeps for one Feed's layout (CUDA: staging buffers and pinning; Metal: the buffer wrapping the
// Feed's memory). Made on the worker thread, used from render threads, freed on the worker thread.
struct GpuRing {
    virtual ~GpuRing() = default;
};

struct Ring;

// Tells the core a started copy has finished. Handed to the backend inside FrameCopy; valid until it is used.
struct SlotRef {
    Ring* ring;
    int index;
};

class Backend {
public:
    virtual ~Backend() = default;
    // Worker thread, before the Feed exists: reserve what slots of `slotCapacity` bytes need on the GPU side. It
    // is what fails when graphics memory is short, and nothing has been published yet. nullptr and `err` on failure.
    virtual GpuRing* reserve(size_t slotCapacity, std::string& err) = 0;
    // Worker thread, with the Feed made and mapped at `base` for `bytes`: tie that memory to the GPU. "" when done,
    // else the text for the Feed's error line; the Feed then stays published and gets no frames.
    virtual std::string attach(GpuRing* gpu, void* base, size_t bytes) = 0;
    // Worker thread, just before the Feed's mapping goes away: let go of what attach tied to it.
    virtual void detach(GpuRing* gpu, void* base) = 0;
};

// The backend of this build. Defined by the one GPU file that is linked.
Backend& backend();

enum class Ready { Yes, Busy, No };  // Busy: counted as a drop and skipped. No: skipped, the Feed already says why.

// Where one frame goes: the pixels are copied to `dstPixels`, which is `dstOffset` bytes into the Feed's mapping.
struct FrameCopy {
    void* dstPixels = nullptr;
    size_t dstOffset = 0;
    size_t rowBytes = 0, frameBytes = 0;
    int height = 0;
    SlotRef* done = nullptr;
};

// One frame submission, as the backend does it.
class Copier {
public:
    virtual ~Copier() = default;
    // Render thread, without waiting: can a copy be started now? (CUDA: a staging buffer is free.)
    virtual Ready ready(GpuRing* gpu) = 0;
    // Render thread, without waiting: start the copy and return. The backend must call feedSlotDone (or
    // feedSlotFailed) exactly once, when it has finished, from any thread, and never after returning a failure.
    // "" when started, else why not.
    virtual std::string start(GpuRing* gpu, const FrameCopy& copy) = 0;
};

// The shared half of a frame submission. Call only when feedReaderPresent() and not feedIsGuest().
bool feedSubmit(const FrameInfo& info, Copier& copier);

// A started copy has finished: publish the slot. Any thread, touches only atomics.
void feedSlotDone(SlotRef* slot);
// A started copy failed: give the slot back and say so in the Feed. Any thread.
void feedSlotFailed(SlotRef* slot, const std::string& text);

}  // namespace scopecreep
