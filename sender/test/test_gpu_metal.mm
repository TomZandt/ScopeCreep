// The Metal side of the fake-host test; see test_gpu.h. UNCOMPILED when written (no Mac was available to the
// author): expect to fix small compile errors on the Mac.
//
// The buffers it makes are what Resolve's are on the Mac (docs/findings/253-metal-handoff.md): MTLBuffers with
// private storage, which the CPU cannot read. The host reaches them only through blits to and from shared buffers.
#include "test_gpu.h"

#include "ofxGPURender.h"

#import <Foundation/Foundation.h>
#import <Metal/Metal.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>

namespace testgpu {

namespace {
id<MTLDevice> gDevice = nil;
id<MTLCommandQueue> gQueue = nil;

// Run one blit on the host's queue and wait for it (the test may wait; the Sender may not).
template <class F>
void blitAndWait(F encode)
{
    @autoreleasepool {
        id<MTLCommandBuffer> cb = [gQueue commandBuffer];
        id<MTLBlitCommandEncoder> blit = [cb blitCommandEncoder];
        encode(blit);
        [blit endEncoding];
        [cb commit];
        [cb waitUntilCompleted];
        if (cb.status != MTLCommandBufferStatusCompleted) {
            std::printf("a Metal blit of the test host failed: %s\n", [[cb.error localizedDescription] UTF8String]);
            std::exit(2);
        }
    }
}
}  // namespace

const char* const kEnabledProp = kOfxImageEffectPropMetalEnabled;
const char* const kHandleProp = kOfxImageEffectPropMetalCommandQueue;
const char* const kSupportedProp = kOfxImageEffectPropMetalRenderSupported;
const char* const kApiName = "Metal";
const bool kHasStagingLimit = false;

bool init(std::string& error)
{
    gDevice = MTLCreateSystemDefaultDevice();
    if (!gDevice) { error = "no Metal device: cannot run the Sender test"; return false; }
    gQueue = [gDevice newCommandQueue];
    if (!gQueue) { error = "cannot make a Metal command queue: cannot run the Sender test"; return false; }
    return true;
}

void* hostHandle() { return (__bridge void*)gQueue; }

void* alloc(size_t bytes)
{
    id<MTLBuffer> b = [gDevice newBufferWithLength:bytes options:MTLResourceStorageModePrivate];
    if (!b) {
        std::printf("newBufferWithLength failed: is the GPU out of memory?\n");
        std::exit(2);
    }
    return (__bridge_retained void*)b;  // kept for the life of the test
}

void upload(void* buf, size_t pitch, const void* host, size_t rowBytes, int rows)
{
    @autoreleasepool {
        const size_t bytes = pitch * (size_t)rows;
        id<MTLBuffer> staging = [gDevice newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        std::memset([staging contents], 0, bytes);
        for (int y = 0; y < rows; ++y) std::memcpy((char*)[staging contents] + (size_t)y * pitch, (const char*)host + (size_t)y * rowBytes, rowBytes);
        id<MTLBuffer> dst = (__bridge id<MTLBuffer>)buf;
        blitAndWait([&](id<MTLBlitCommandEncoder> blit) { [blit copyFromBuffer:staging sourceOffset:0 toBuffer:dst destinationOffset:0 size:bytes]; });
    }
}

void zero(void* buf, size_t bytes)
{
    id<MTLBuffer> dst = (__bridge id<MTLBuffer>)buf;
    blitAndWait([&](id<MTLBlitCommandEncoder> blit) { [blit fillBuffer:dst range:NSMakeRange(0, bytes) value:0]; });
}

void download(void* buf, size_t pitch, void* host, size_t rowBytes, int rows)
{
    @autoreleasepool {
        const size_t bytes = pitch * (size_t)rows;
        id<MTLBuffer> staging = [gDevice newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        id<MTLBuffer> src = (__bridge id<MTLBuffer>)buf;
        // The same queue, so it runs after whatever the plugin queued on it.
        blitAndWait([&](id<MTLBlitCommandEncoder> blit) { [blit copyFromBuffer:src sourceOffset:0 toBuffer:staging destinationOffset:0 size:bytes]; });
        for (int y = 0; y < rows; ++y) std::memcpy((char*)host + (size_t)y * rowBytes, (const char*)[staging contents] + (size_t)y * pitch, rowBytes);
    }
}

void syncHost()
{
    @autoreleasepool {
        id<MTLCommandBuffer> cb = [gQueue commandBuffer];
        [cb commit];
        [cb waitUntilCompleted];
    }
}

void keepBusy()
{
    static void* a = alloc((size_t)128 << 20);
    static void* b = alloc((size_t)128 << 20);
    @autoreleasepool {
        id<MTLBuffer> from = (__bridge id<MTLBuffer>)a, to = (__bridge id<MTLBuffer>)b;
        // A 128 MB blit takes about 4 ms (docs/findings/253-metal-handoff.md); 40 of them are well over 100 ms.
        for (int i = 0; i < 40; ++i) {
            id<MTLCommandBuffer> cb = [gQueue commandBuffer];
            id<MTLBlitCommandEncoder> blit = [cb blitCommandEncoder];
            [blit copyFromBuffer:from sourceOffset:0 toBuffer:to destinationOffset:0 size:(NSUInteger)128 << 20];
            [blit endEncoding];
            [cb commit];
        }
    }
}

}  // namespace testgpu
