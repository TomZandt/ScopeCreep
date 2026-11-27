// The Metal half of the Sender's frame transport (macOS, Apple Silicon); see feed_core.h for the shared half.
//
// UNCOMPILED when written: the author had no Mac. Expect to fix small compile errors on the Mac. The design is the
// one measured in docs/findings/253-metal-handoff.md (route B), on Resolve 21 on an M4:
//  - Resolve's source image is an MTLBuffer with private storage: the CPU cannot read it, so a CPU copy is out;
//  - the Feed's shared memory is wrapped once per ring in an MTLBuffer (newBufferWithBytesNoCopy on the mmap'd
//    memory: page aligned, length rounded up to a page, 16 KiB here), and each frame is one blit from Resolve's
//    buffer straight into its slot: about 4 ms for a UHD RGBA float frame, against 12 ms with a staging copy;
//  - the blit is encoded on Resolve's own command queue, so it is ordered after the work that made the frame, and
//    committed; the command buffer's completion handler publishes the slot. The render call returns at once.
//    The probe waited for the GPU to time its copies and stuttered playback: this file never waits (ADR 0003).
// Rules for this file: no waitUntilCompleted, no waitUntilScheduled, no semaphores, nothing that can block a render
// thread. The completion handler (a Metal thread) touches only atomics, through feedSlotDone / feedSlotFailed.
#include "feed_core.h"

#include "platform.h"
#include "scopecreep_feed.h"

#import <Foundation/Foundation.h>
#import <Metal/Metal.h>

#include <atomic>
#include <string>

#include <unistd.h>

namespace scopecreep {
namespace {

// The GPU of Resolve's command queue, as first seen on a render call: the ring's buffer must be made on it. A device
// object lives as long as the process, so it is retained once and never released.
std::atomic<void*> gDevice{nullptr};

void noteDevice(id<MTLCommandQueue> queue)
{
    void* seen = gDevice.load(std::memory_order_acquire);
    if (seen) return;
    void* mine = (void*)CFBridgingRetain(queue.device);
    if (!gDevice.compare_exchange_strong(seen, mine, std::memory_order_acq_rel)) CFRelease(mine);
}

size_t roundUpToPage(size_t n)
{
    const size_t page = (size_t)getpagesize();  // 16 KiB on Apple Silicon
    return (n + page - 1) / page * page;
}

struct MetalRing : GpuRing {
    id<MTLDevice> device = nil;
    id<MTLBuffer> wrapped = nil;  // wraps the whole Feed mapping; nil until attach
    size_t wrappedBytes = 0;
};

class MetalBackend : public Backend {
public:
    GpuRing* reserve(size_t slotCapacity, std::string& err) override
    {
        // Nothing to reserve on the GPU: the Feed's own memory is the destination, there is no staging buffer.
        void* device = gDevice.load(std::memory_order_acquire);
        if (!device) { err = "no Metal device has been seen yet"; return nullptr; }
        MetalRing* r = new MetalRing();
        r->device = (__bridge id<MTLDevice>)device;
        return r;
    }

    std::string attach(GpuRing* gpu, void* base, size_t bytes) override
    {
        MetalRing* r = static_cast<MetalRing*>(gpu);
        @autoreleasepool {
            const size_t length = roundUpToPage(bytes);
            // `base` is the start of an mmap, so page aligned; the object behind it is rounded up to a page by
            // macOS, so the rounded length is all backed (feed/src/platform/macos.rs).
            r->wrapped = [r->device newBufferWithBytesNoCopy:base length:length options:MTLResourceStorageModeShared deallocator:nil];
            if (!r->wrapped) return "cannot wrap the Feed's memory for Metal (newBufferWithBytesNoCopy returned nil)";
            r->wrappedBytes = length;
        }
        // Touch every page now, here on the worker thread, so the first frames do not pay for cold pages (the
        // probe saw 100 ms on a first copy). An atomic OR of zero changes nothing, so it cannot race with the
        // counters, slot states and heartbeat that other threads and the app write in these pages.
        const size_t page = (size_t)getpagesize();
        for (size_t off = 0; off < bytes; off += page) __atomic_fetch_or((volatile unsigned char*)base + off, 0, __ATOMIC_RELAXED);
        return "";
    }

    void detach(GpuRing* gpu, void* base) override
    {
        static_cast<MetalRing*>(gpu)->wrapped = nil;  // before the mapping goes away
    }
};

// One frame: a blit from Resolve's buffer into the slot, on Resolve's queue.
class MetalCopier : public Copier {
public:
    MetalCopier(void* queue, void* src, size_t srcOffset, size_t srcRowBytes) : queue_(queue), src_(src), srcOffset_(srcOffset), srcRowBytes_(srcRowBytes) {}

    Ready ready(GpuRing* gpu) override { return static_cast<MetalRing*>(gpu)->wrapped ? Ready::Yes : Ready::No; }

    std::string start(GpuRing* gpu, const FrameCopy& copy) override
    {
        MetalRing* ring = static_cast<MetalRing*>(gpu);
        @autoreleasepool {
            id<MTLCommandQueue> queue = (__bridge id<MTLCommandQueue>)queue_;
            id<MTLBuffer> src = (__bridge id<MTLBuffer>)src_;
            if (queue.device != ring->device) return "Resolve is rendering on a different GPU than the one the Feed was set up for";
            // A blit outside a buffer is a Metal API error that can take Resolve down with it: check first.
            const size_t srcEnd = srcOffset_ + (size_t)(copy.height - 1) * srcRowBytes_ + copy.rowBytes;
            if (srcEnd > (size_t)src.length) return "the frame is larger than Resolve's buffer";
            if (copy.dstOffset + copy.frameBytes > ring->wrappedBytes) return "the frame does not fit the Feed's slot";

            id<MTLCommandBuffer> cb = [queue commandBuffer];
            id<MTLBlitCommandEncoder> blit = cb ? [cb blitCommandEncoder] : nil;
            if (!blit) return "cannot encode the frame copy on Resolve's Metal queue";
            if (srcRowBytes_ == copy.rowBytes) {
                // Rows are tight, as Resolve lays them out: the frame is one run of bytes.
                [blit copyFromBuffer:src sourceOffset:srcOffset_ toBuffer:ring->wrapped destinationOffset:copy.dstOffset size:copy.frameBytes];
            } else {
                for (int y = 0; y < copy.height; ++y)
                    [blit copyFromBuffer:src
                            sourceOffset:srcOffset_ + (size_t)y * srcRowBytes_
                                toBuffer:ring->wrapped
                       destinationOffset:copy.dstOffset + (size_t)y * copy.rowBytes
                                    size:copy.rowBytes];
            }
            [blit endEncoding];
            SlotRef* done = copy.done;
            [cb addCompletedHandler:^(id<MTLCommandBuffer> finished) {
                if (finished.status == MTLCommandBufferStatusCompleted) {
                    feedSlotDone(done);
                } else {
                    const char* why = finished.error ? [[finished.error localizedDescription] UTF8String] : "unknown error";
                    feedSlotFailed(done, std::string("the frame copy failed: ") + (why ? why : "unknown error"));
                }
            }];
            [cb commit];  // not waited for
        }
        return "";
    }

private:
    void* queue_;
    void* src_;
    size_t srcOffset_, srcRowBytes_;
};

}  // namespace

Backend& backend()
{
    static MetalBackend b;
    return b;
}

bool metalPassthrough(void* queuePtr, void* dstPtr, size_t dstOffset, size_t dstRowBytes, void* srcPtr, size_t srcOffset, size_t srcRowBytes,
                      size_t widthBytes, size_t height)
{
    @autoreleasepool {
        id<MTLCommandQueue> queue = (__bridge id<MTLCommandQueue>)queuePtr;
        id<MTLBuffer> src = (__bridge id<MTLBuffer>)srcPtr;
        id<MTLBuffer> dst = (__bridge id<MTLBuffer>)dstPtr;
        id<MTLCommandBuffer> cb = [queue commandBuffer];
        id<MTLBlitCommandEncoder> blit = cb ? [cb blitCommandEncoder] : nil;
        if (!blit) return false;
        if (srcRowBytes == widthBytes && dstRowBytes == widthBytes) {
            [blit copyFromBuffer:src sourceOffset:srcOffset toBuffer:dst destinationOffset:dstOffset size:widthBytes * height];
        } else {
            for (size_t y = 0; y < height; ++y)
                [blit copyFromBuffer:src sourceOffset:srcOffset + y * srcRowBytes toBuffer:dst destinationOffset:dstOffset + y * dstRowBytes size:widthBytes];
        }
        [blit endEncoding];
        [cb commit];  // not waited for: the OFX Metal contract is that the host's queue orders it
        return true;
    }
}

bool feedSubmitMetal(void* queuePtr, void* srcBuffer, size_t srcOffset, size_t srcRowBytes, const FrameInfo& info)
{
    @autoreleasepool {
        noteDevice((__bridge id<MTLCommandQueue>)queuePtr);  // before the ring is asked for: it is built on this device
        MetalCopier copier(queuePtr, srcBuffer, srcOffset, srcRowBytes);
        return feedSubmit(info, copier);
    }
}

}  // namespace scopecreep
