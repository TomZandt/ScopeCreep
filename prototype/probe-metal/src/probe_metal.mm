// PROTOTYPE (throwaway): macOS arm64 Metal OFX probe, issue #253 (parent #236). UNCOMPILED when written (no Mac
// available to the author): expect to fix small compile errors on the Mac.
//
// What it does: passes the picture through (so the viewer is unchanged), declares Metal render support, and logs
// how Resolve hands over each frame: is Metal enabled, what the image data pointer really is (an id<MTLBuffer>?),
// its storage mode, length, device, the command queue, row bytes, bounds, and how long the render action takes.
// For the first N full-size (non-thumbnail) renders it also times one copy of the frame into POSIX shared memory
// three ways, waiting for each so it can be timed (the real Sender must never wait; this is a probe):
//   A  blit src -> own MTLBuffer (shared storage), then CPU memcpy own.contents -> shm
//   B  blit src -> an MTLBuffer wrapping the mmap'd shm (newBufferWithBytesNoCopy), so the GPU writes straight in
//   C  CPU memcpy src.contents -> shm (only when src is CPU-visible, i.e. shared/managed storage)
// Env: SCOPECREEP_PROBE_DIR (default /tmp/scopecreep-probe), SCOPECREEP_PROBE_COPIES (default 30),
//      SCOPECREEP_PROBE_SHM (default /scopecreep-probe-metal; macOS limits shm names to 31 chars),
//      SCOPECREEP_PROBE_QUIET=1 (no per-render lines). touch <DIR>/copy.trigger for another batch of copies.
#include "ofxCore.h"
#include "ofxImageEffect.h"
#include "ofxImageEffectExt.h" // Resolve's own SDK: kOfxImageClipPropThumbnail lives here, not in upstream openfx
#include "ofxGPURender.h"
#include "ofxProperty.h"

#include "probe_format.h"

#import <Foundation/Foundation.h>
#import <Metal/Metal.h>

#include <chrono>
#include <cerrno>
#include <cstdarg>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <pthread.h>

#define PLUGIN_ID "com.scopecreep.metalprobe"

static OfxHost* gHost = nullptr;
static OfxPropertySuiteV1* gProp = nullptr;
static OfxImageEffectSuiteV1* gEffect = nullptr;
static std::mutex gLogMutex;

// ---------------------------------------------------------------- logging

static std::string probeDir()
{
    const char* d = std::getenv("SCOPECREEP_PROBE_DIR");
    return d && *d ? d : "/tmp/scopecreep-probe";
}

static void logf(const char* fmt, ...)
{
    std::lock_guard<std::mutex> lock(gLogMutex);
    const std::string dir = probeDir();
    mkdir(dir.c_str(), 0755);
    FILE* f = std::fopen((dir + "/probe_metal.log").c_str(), "a");
    if (!f) return;
    auto now = std::chrono::system_clock::now().time_since_epoch();
    long long ms = std::chrono::duration_cast<std::chrono::milliseconds>(now).count();
    uint64_t tid = 0;
    pthread_threadid_np(nullptr, &tid);
    std::fprintf(f, "[%lld.%03lld pid=%d tid=%llu] ", ms / 1000, ms % 1000, (int)getpid(), (unsigned long long)tid);
    va_list ap;
    va_start(ap, fmt);
    std::vfprintf(f, fmt, ap);
    va_end(ap);
    std::fputc('\n', f);
    std::fclose(f);
}

static double nowMs()
{
    return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

static bool envFlag(const char* name, bool def)
{
    const char* v = std::getenv(name);
    if (!v || !*v) return def;
    return std::strcmp(v, "0") != 0;
}

// ------------------------------------------------------- property helpers

static std::string str(OfxPropertySetHandle p, const char* name, int idx = 0)
{
    if (!p) return "<null set>";
    char* v = nullptr;
    OfxStatus s = gProp->propGetString(p, name, idx, &v);
    if (s != kOfxStatOK) return "<unset:" + std::to_string(s) + ">";
    return v ? v : "<null>";
}

static std::string ints(OfxPropertySetHandle p, const char* name, int count)
{
    if (!p) return "<null set>";
    std::string out;
    for (int i = 0; i < count; ++i) {
        int v = 0;
        OfxStatus s = gProp->propGetInt(p, name, i, &v);
        if (s != kOfxStatOK) return i == 0 ? "<unset:" + std::to_string(s) + ">" : out;
        out += (i ? "," : "") + std::to_string(v);
    }
    return out;
}

static std::string objcStr(NSString* s) { return s ? std::string([s UTF8String]) : std::string("<nil>"); }

// ------------------------------------------------------------ describe etc.

static OfxStatus onLoad()
{
    OfxPropertySetHandle hp = gHost ? gHost->host : nullptr;
    logf("LOAD host name=%s label=%s version=%s api=%s",
         str(hp, kOfxPropName).c_str(), str(hp, kOfxPropLabel).c_str(), ints(hp, kOfxPropVersion, 4).c_str(),
         ints(hp, kOfxPropAPIVersion, 2).c_str());
    logf("LOAD host MetalRenderSupported=%s CudaRenderSupported=%s OpenCLRenderSupported=%s",
         str(hp, kOfxImageEffectPropMetalRenderSupported).c_str(),
         str(hp, kOfxImageEffectPropCudaRenderSupported).c_str(),
         str(hp, kOfxImageEffectPropOpenCLRenderSupported).c_str());
    @autoreleasepool {
        NSArray<id<MTLDevice>>* all = MTLCopyAllDevices();
        for (id<MTLDevice> d in all)
            logf("LOAD metal device name=%s unifiedMemory=%d lowPower=%d registryID=%llu recommendedMaxWorkingSet=%llu",
                 objcStr(d.name).c_str(), (int)d.hasUnifiedMemory, (int)d.isLowPower,
                 (unsigned long long)d.registryID, (unsigned long long)d.recommendedMaxWorkingSetSize);
    }
    return kOfxStatOK;
}

static OfxStatus onDescribe(OfxImageEffectHandle effect)
{
    OfxPropertySetHandle p = nullptr;
    gEffect->getPropertySet(effect, &p);
    gProp->propSetString(p, kOfxPropLabel, 0, "scopecreep Metal Probe");
    gProp->propSetString(p, kOfxImageEffectPluginPropGrouping, 0, "scopecreep");
    gProp->propSetString(p, kOfxPropPluginDescription, 0,
                         "Throwaway probe: logs how Resolve hands over Metal frames. Passes the picture through.");
    gProp->propSetString(p, kOfxImageEffectPropSupportedContexts, 0, kOfxImageEffectContextFilter);
    gProp->propSetString(p, kOfxImageEffectPropSupportedContexts, 1, kOfxImageEffectContextGeneral);
    gProp->propSetString(p, kOfxImageEffectPropSupportedPixelDepths, 0, kOfxBitDepthFloat);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsMultipleClipDepths, 0, 0);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsTiles, 0, 0);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsMultiResolution, 0, 0);
    gProp->propSetString(p, kOfxImageEffectPluginRenderThreadSafety, 0, kOfxImageEffectRenderFullySafe);
    gProp->propSetString(p, kOfxImageEffectPropMetalRenderSupported, 0, "true");
    logf("DESCRIBE declared MetalRenderSupported=true");
    return kOfxStatOK;
}

static OfxStatus onDescribeInContext(OfxImageEffectHandle effect, OfxPropertySetHandle inArgs)
{
    logf("describeInContext: context=%s", str(inArgs, kOfxImageEffectPropContext).c_str());
    for (const char* name : {kOfxImageEffectSimpleSourceClipName, kOfxImageEffectOutputClipName}) {
        OfxPropertySetHandle cp = nullptr;
        gEffect->clipDefine(effect, name, &cp);
        gProp->propSetString(cp, kOfxImageEffectPropSupportedComponents, 0, kOfxImageComponentRGBA);
        gProp->propSetInt(cp, kOfxImageEffectPropSupportsTiles, 0, 0);
    }
    return kOfxStatOK;
}

// ------------------------------------------------- shared memory + copies

namespace {

struct ShmState {
    void* base = nullptr;
    size_t mapped = 0;  // page-rounded length
    bool ok = false;
    std::string err;
};

std::mutex gCopyMutex;  // one probe copy at a time; a render that finds it busy skips the copies
ShmState gShm;
id<MTLBuffer> gOwn = nil;      // path A destination (shared storage)
id<MTLBuffer> gWrapped = nil;  // path B destination (wraps gShm)
size_t gOwnBytes = 0, gWrappedBytes = 0;
int gRemaining = -1;           // copy batches left; set from env on first use
int gCopyIndex = 0;
pm::Samples gA_blit, gA_memcpy, gA_total, gB_blit, gC_memcpy, gQueueDrain;

size_t pageSize() { return (size_t)getpagesize(); }

// POSIX shared memory, created fresh (macOS allows ftruncate only once per shm object, so unlink first).
void ensureShm(size_t bytes)
{
    const size_t need = pm::roundUpToPage(bytes, pageSize());
    if (gShm.ok && gShm.mapped >= need) return;
    if (gShm.base) { munmap(gShm.base, gShm.mapped); gShm = ShmState(); }
    const char* nm = std::getenv("SCOPECREEP_PROBE_SHM");
    const std::string name = nm && *nm ? nm : "/scopecreep-probe-metal";
    shm_unlink(name.c_str());
    const int fd = shm_open(name.c_str(), O_RDWR | O_CREAT | O_EXCL, 0600);
    if (fd < 0) { gShm.err = std::string("shm_open: ") + std::strerror(errno); return; }
    if (ftruncate(fd, (off_t)need) != 0) { gShm.err = std::string("ftruncate: ") + std::strerror(errno); close(fd); return; }
    void* p = mmap(nullptr, need, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (p == MAP_FAILED) { gShm.err = std::string("mmap: ") + std::strerror(errno); return; }
    const double t0 = nowMs();
    std::memset(p, 0, need);  // pre-fault, as the real Feed would; the cost is logged once
    gShm.base = p;
    gShm.mapped = need;
    gShm.ok = true;
    logf("SHM created name=%s bytes=%zu (page=%zu) prefault_ms=%.3f", name.c_str(), need, pageSize(), nowMs() - t0);
}

// Blit `bytes` from src into dst on `queue`, wait, and return host wall ms and GPU ms.
bool blitAndWait(id<MTLCommandQueue> queue, id<MTLBuffer> src, id<MTLBuffer> dst, size_t bytes, double* wallMs, double* gpuMs)
{
    const double t0 = nowMs();
    id<MTLCommandBuffer> cb = [queue commandBuffer];
    id<MTLBlitCommandEncoder> blit = [cb blitCommandEncoder];
    [blit copyFromBuffer:src sourceOffset:0 toBuffer:dst destinationOffset:0 size:bytes];
    [blit endEncoding];
    [cb commit];
    [cb waitUntilCompleted];
    *wallMs = nowMs() - t0;
    *gpuMs = ([cb GPUEndTime] - [cb GPUStartTime]) * 1000.0;
    if (cb.status != MTLCommandBufferStatusCompleted) {
        logf("COPY blit failed: status=%ld error=%s", (long)cb.status, objcStr(cb.error.localizedDescription).c_str());
        return false;
    }
    return true;
}

// Times one frame into shm by the three routes. `bytes` = rowBytes * height of the source.
void probeCopies(id<MTLCommandQueue> queue, id<MTLBuffer> src, size_t bytes)
{
    std::unique_lock<std::mutex> lk(gCopyMutex, std::try_to_lock);
    if (!lk.owns_lock()) return;

    if (gRemaining < 0) {
        const char* c = std::getenv("SCOPECREEP_PROBE_COPIES");
        gRemaining = c && *c ? std::atoi(c) : 30;
    }
    if (gRemaining == 0) {
        const std::string trig = probeDir() + "/copy.trigger";
        if (access(trig.c_str(), F_OK) == 0 && unlink(trig.c_str()) == 0) {
            const char* c = std::getenv("SCOPECREEP_PROBE_COPIES");
            gRemaining = c && *c ? std::atoi(c) : 30;
        }
    }
    if (gRemaining <= 0) return;
    --gRemaining;
    const int idx = ++gCopyIndex;

    ensureShm(bytes);
    if (!gShm.ok) { logf("COPY #%d no shm: %s", idx, gShm.err.c_str()); return; }
    id<MTLDevice> dev = src.device;

    // Drain Resolve's queue so route timings do not include Resolve's own pending GPU work.
    {
        const double t0 = nowMs();
        id<MTLCommandBuffer> cb = [queue commandBuffer];
        [cb commit];
        [cb waitUntilCompleted];
        const double d = nowMs() - t0;
        gQueueDrain.add(d);
        logf("COPY #%d queue drain (empty command buffer round trip, includes Resolve's earlier queued work) %.3f ms", idx, d);
    }

    // Route A: own buffer.
    double ownAllocMs = 0;
    if (!gOwn || gOwnBytes < bytes) {
        const double t0 = nowMs();
        gOwn = [dev newBufferWithLength:bytes options:MTLResourceStorageModeShared];
        gOwnBytes = bytes;
        ownAllocMs = nowMs() - t0;
        if (gOwn) std::memset([gOwn contents], 0, bytes);  // pre-fault
    }
    if (gOwn) {
        double wall = 0, gpu = 0;
        if (blitAndWait(queue, src, gOwn, bytes, &wall, &gpu)) {
            const double t0 = nowMs();
            std::memcpy(gShm.base, [gOwn contents], bytes);
            const double cpu = nowMs() - t0;
            gA_blit.add(wall);
            gA_memcpy.add(cpu);
            gA_total.add(wall + cpu);
            logf("COPY #%d A blit->own(shared)+memcpy->shm: alloc=%.3f ms, blit wall=%.3f ms (gpu=%.3f ms), memcpy=%.3f ms (%.2f GB/s), "
                 "total=%.3f ms (%.2f GB/s) bytes=%zu",
                 idx, ownAllocMs, wall, gpu, cpu, pm::gbPerSec(bytes, cpu), wall + cpu, pm::gbPerSec(bytes, wall + cpu), bytes);
        }
    }

    // Route B: GPU blit straight into a buffer wrapping the shm mapping.
    double wrapMs = 0;
    if (!gWrapped || gWrappedBytes != gShm.mapped) {
        const double t0 = nowMs();
        gWrapped = [dev newBufferWithBytesNoCopy:gShm.base length:gShm.mapped options:MTLResourceStorageModeShared deallocator:nil];
        gWrappedBytes = gShm.mapped;
        wrapMs = nowMs() - t0;
        if (!gWrapped) logf("COPY #%d B newBufferWithBytesNoCopy on the shm mapping returned nil (alignment/length/permissions?)", idx);
    }
    if (gWrapped) {
        double wall = 0, gpu = 0;
        if (blitAndWait(queue, src, gWrapped, bytes, &wall, &gpu)) {
            gB_blit.add(wall);
            logf("COPY #%d B blit->wrapped shm: wrap=%.3f ms, blit wall=%.3f ms (gpu=%.3f ms, %.2f GB/s) bytes=%zu",
                 idx, wrapMs, wall, gpu, pm::gbPerSec(bytes, wall), bytes);
        }
    }

    // Route C: CPU copy straight from the source buffer (valid only for CPU-visible storage).
    if ([src contents] != nullptr && src.storageMode != MTLStorageModePrivate) {
        const double t0 = nowMs();
        std::memcpy(gShm.base, [src contents], bytes);
        const double cpu = nowMs() - t0;
        gC_memcpy.add(cpu);
        logf("COPY #%d C memcpy src.contents->shm: %.3f ms (%.2f GB/s) bytes=%zu", idx, cpu, pm::gbPerSec(bytes, cpu), bytes);
    } else {
        logf("COPY #%d C skipped: source is not CPU-visible (contents=%p storage=%s)", idx, [src contents],
             pm::storageModeName((unsigned long)src.storageMode).c_str());
    }

    if (gRemaining == 0 || idx % 10 == 0) {
        logf("SUMMARY queue-drain: %s", gQueueDrain.summary("drain").c_str());
        logf("SUMMARY A: %s | %s | %s", gA_blit.summary("blit").c_str(), gA_memcpy.summary("memcpy").c_str(), gA_total.summary("total").c_str());
        logf("SUMMARY B: %s", gB_blit.summary("blit-into-shm").c_str());
        logf("SUMMARY C: %s", gC_memcpy.summary("cpu-memcpy-from-src").c_str());
    }
}

}  // namespace

// ------------------------------------------------------------------ render

struct ImageInfo {
    OfxPropertySetHandle props = nullptr;
    void* data = nullptr;
    int bounds[4] = {0, 0, 0, 0};
    int rowBytes = 0;
    std::string depth, components;
};

static bool readImage(OfxPropertySetHandle p, ImageInfo& im)
{
    im.props = p;
    gProp->propGetPointer(p, kOfxImagePropData, 0, &im.data);
    for (int i = 0; i < 4; ++i) gProp->propGetInt(p, kOfxImagePropBounds, i, &im.bounds[i]);
    gProp->propGetInt(p, kOfxImagePropRowBytes, 0, &im.rowBytes);
    im.depth = str(p, kOfxImageEffectPropPixelDepth);
    im.components = str(p, kOfxImageEffectPropComponents);
    return im.data != nullptr;
}

static std::string describeBuffer(void* ptr)
{
    if (!ptr) return "null";
    @autoreleasepool {
        id obj = (__bridge id)ptr;
        // Ask the object, rather than assume: only touch MTLBuffer API if it answers like one.
        if (![obj conformsToProtocol:@protocol(MTLBuffer)]) {
            char b[160];
            std::snprintf(b, sizeof b, "NOT an MTLBuffer: class=%s ptr=%p", objcStr(NSStringFromClass([obj class])).c_str(), ptr);
            return b;
        }
        id<MTLBuffer> buf = (id<MTLBuffer>)obj;
        char b[512];
        std::snprintf(b, sizeof b, "MTLBuffer class=%s length=%lu storage=%s cpuVisible=%d contents=%p device=%s label=%s",
                      objcStr(NSStringFromClass([obj class])).c_str(), (unsigned long)buf.length,
                      pm::storageModeName((unsigned long)buf.storageMode).c_str(), [buf contents] != nullptr ? 1 : 0, [buf contents],
                      objcStr(buf.device.name).c_str(), objcStr(buf.label).c_str());
        return b;
    }
}

static OfxStatus onRender(OfxImageEffectHandle effect, OfxPropertySetHandle inArgs)
{
    const double t0 = nowMs();
    @autoreleasepool {
        double time = 0;
        gProp->propGetDouble(inArgs, kOfxPropTime, 0, &time);
        int rw[4] = {0, 0, 0, 0};
        for (int i = 0; i < 4; ++i) gProp->propGetInt(inArgs, kOfxImageEffectPropRenderWindow, i, &rw[i]);
        int metal = 0;
        gProp->propGetInt(inArgs, kOfxImageEffectPropMetalEnabled, 0, &metal);
        void* queuePtr = nullptr;
        gProp->propGetPointer(inArgs, kOfxImageEffectPropMetalCommandQueue, 0, &queuePtr);

        OfxImageClipHandle srcClip = nullptr, dstClip = nullptr;
        OfxPropertySetHandle srcClipProps = nullptr, dstClipProps = nullptr;
        gEffect->clipGetHandle(effect, kOfxImageEffectSimpleSourceClipName, &srcClip, &srcClipProps);
        gEffect->clipGetHandle(effect, kOfxImageEffectOutputClipName, &dstClip, &dstClipProps);
        OfxPropertySetHandle srcP = nullptr, dstP = nullptr;
        ImageInfo src, dst;
        const bool haveSrc = gEffect->clipGetImage(srcClip, time, nullptr, &srcP) == kOfxStatOK && readImage(srcP, src);
        const bool haveDst = gEffect->clipGetImage(dstClip, time, nullptr, &dstP) == kOfxStatOK && readImage(dstP, dst);
        const bool thumb = ints(srcClipProps, kOfxImageClipPropThumbnail, 1) == "1";
        const int w = rw[2] - rw[0], h = rw[3] - rw[1];

        if (!envFlag("SCOPECREEP_PROBE_QUIET", false)) {
            logf("RENDER time=%.3f render_window=%d,%d,%d,%d thumbnail=%d metalEnabled=%d cudaEnabled=%s openclEnabled=%s "
                 "queue_ptr=%p",
                 time, rw[0], rw[1], rw[2], rw[3], (int)thumb, metal, ints(inArgs, kOfxImageEffectPropCudaEnabled, 1).c_str(),
                 ints(inArgs, kOfxImageEffectPropOpenCLEnabled, 1).c_str(), queuePtr);
            logf("RENDER   src: depth=%s components=%s bounds=%d,%d,%d,%d rowbytes=%d data=%s", src.depth.c_str(), src.components.c_str(),
                 src.bounds[0], src.bounds[1], src.bounds[2], src.bounds[3], src.rowBytes, metal ? describeBuffer(src.data).c_str() : "(cpu pointer)");
            logf("RENDER   dst: depth=%s components=%s bounds=%d,%d,%d,%d rowbytes=%d data=%s", dst.depth.c_str(), dst.components.c_str(),
                 dst.bounds[0], dst.bounds[1], dst.bounds[2], dst.bounds[3], dst.rowBytes, metal ? describeBuffer(dst.data).c_str() : "(cpu pointer)");
            if (metal && queuePtr) {
                id<MTLCommandQueue> q = (__bridge id<MTLCommandQueue>)queuePtr;
                logf("RENDER   queue: class=%s label=%s device=%s", objcStr(NSStringFromClass([(id)q class])).c_str(), objcStr(q.label).c_str(),
                     objcStr(q.device.name).c_str());
            }
        }

        double submitMs = 0;
        if (haveSrc && haveDst && src.depth == kOfxBitDepthFloat) {
            if (metal && queuePtr) {
                id<MTLCommandQueue> queue = (__bridge id<MTLCommandQueue>)queuePtr;
                id<MTLBuffer> sbuf = (__bridge id<MTLBuffer>)src.data;
                id<MTLBuffer> dbuf = (__bridge id<MTLBuffer>)dst.data;
                // Passthrough on Resolve's queue, NOT waited for (as the OFX header asks).
                const double s0 = nowMs();
                id<MTLCommandBuffer> cb = [queue commandBuffer];
                id<MTLBlitCommandEncoder> blit = [cb blitCommandEncoder];
                const bool sameLayout = src.rowBytes == dst.rowBytes && src.bounds[1] == dst.bounds[1] && src.bounds[3] == dst.bounds[3];
                if (sameLayout) {
                    const NSUInteger n = (NSUInteger)src.rowBytes * (NSUInteger)(src.bounds[3] - src.bounds[1]);
                    [blit copyFromBuffer:sbuf sourceOffset:0 toBuffer:dbuf destinationOffset:0 size:n];
                } else {
                    const NSUInteger rowBytesCopy = (NSUInteger)w * 16;
                    for (int y = rw[1]; y < rw[3]; ++y) {
                        const NSUInteger so = (NSUInteger)(y - src.bounds[1]) * src.rowBytes + (NSUInteger)(rw[0] - src.bounds[0]) * 16;
                        const NSUInteger dO = (NSUInteger)(y - dst.bounds[1]) * dst.rowBytes + (NSUInteger)(rw[0] - dst.bounds[0]) * 16;
                        [blit copyFromBuffer:sbuf sourceOffset:so toBuffer:dbuf destinationOffset:dO size:rowBytesCopy];
                    }
                }
                [blit endEncoding];
                [cb commit];
                submitMs = nowMs() - s0;

                if (!thumb && sameLayout && w > 1000)  // full-size frames only (UHD: w == 3840)
                    probeCopies(queue, sbuf, (size_t)src.rowBytes * (size_t)(src.bounds[3] - src.bounds[1]));
            } else {
                logf("RENDER metal not enabled by the host (metalEnabled=%d queue=%p): CPU path, no passthrough done by the probe", metal, queuePtr);
            }
        }

        const double ms = nowMs() - t0;
        if (!envFlag("SCOPECREEP_PROBE_QUIET", false))
            logf("RENDER   done: passthrough encode+commit=%.3f ms, whole render action=%.3f ms (includes probe copies when they ran)", submitMs, ms);

        if (srcP) gEffect->clipReleaseImage(srcP);
        if (dstP) gEffect->clipReleaseImage(dstP);
        return (haveSrc && haveDst) ? kOfxStatOK : kOfxStatFailed;
    }
}

// -------------------------------------------------------------------- entry

static OfxStatus mainEntry(const char* action, const void* handle, OfxPropertySetHandle inArgs, OfxPropertySetHandle outArgs)
{
    OfxImageEffectHandle effect = (OfxImageEffectHandle)handle;
    logf("ACTION %s", action);
    // Resolve treats the default reply to (Create|Destroy)Instance as a failed create and retries without rendering.
    if (std::strcmp(action, kOfxActionCreateInstance) == 0 || std::strcmp(action, kOfxActionDestroyInstance) == 0)
        return kOfxStatOK;
    if (std::strcmp(action, kOfxActionLoad) == 0) return onLoad();
    if (std::strcmp(action, kOfxActionDescribe) == 0) return onDescribe(effect);
    if (std::strcmp(action, kOfxImageEffectActionDescribeInContext) == 0) return onDescribeInContext(effect, inArgs);
    if (std::strcmp(action, kOfxImageEffectActionRender) == 0) return onRender(effect, inArgs);
    if (std::strcmp(action, kOfxImageEffectActionBeginSequenceRender) == 0 ||
        std::strcmp(action, kOfxImageEffectActionEndSequenceRender) == 0) {
        void* q = nullptr;
        gProp->propGetPointer(inArgs, kOfxImageEffectPropMetalCommandQueue, 0, &q);
        logf("ACTION %s metalEnabled=%s queue_ptr=%p", action, ints(inArgs, kOfxImageEffectPropMetalEnabled, 1).c_str(), q);
        return kOfxStatReplyDefault;
    }
    return kOfxStatReplyDefault;
}

static void setHost(OfxHost* host)
{
    gHost = host;
    if (!host) return;
    gProp = (OfxPropertySuiteV1*)host->fetchSuite(host->host, kOfxPropertySuite, 1);
    gEffect = (OfxImageEffectSuiteV1*)host->fetchSuite(host->host, kOfxImageEffectSuite, 1);
}

static OfxPlugin gPlugin = {kOfxImageEffectPluginApi, 1, PLUGIN_ID, 0, 1, setHost, mainEntry};

extern "C" {
__attribute__((visibility("default"))) int OfxGetNumberOfPlugins(void) { return 1; }
__attribute__((visibility("default"))) OfxPlugin* OfxGetPlugin(int nth) { return nth == 0 ? &gPlugin : nullptr; }
}
