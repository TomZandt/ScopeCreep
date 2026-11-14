// See feed_copy.h. Rules for this file (ADR 0003, Resolve never waits):
//  - no cudaStreamSynchronize / cudaEventSynchronize / cudaDeviceSynchronize, anywhere;
//  - the one mutex is only ever taken with try_lock on the render path: if another render thread holds it, this
//    frame is skipped and counted, not waited for;
//  - the completion callback (a CUDA thread) touches only atomics;
//  - the two things that do take real time happen once, not per frame: building a ring (pinning its memory) the
//    first time an app is attached, and letting an old ring go after a larger frame replaced it.
#include "feed_copy.h"

#include "scopecreep_feed.h"

#include <atomic>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

#include <cuda_runtime.h>

#ifndef SCOPECREEP_SENDER_VERSION
#define SCOPECREEP_SENDER_VERSION "0.0.0"
#endif

namespace scopecreep {
namespace {

constexpr uint32_t kSlots = 4;                         // Stage 0: four slots held UHD at 60 fps with no drops
constexpr uint64_t kRetryNs = 5ull * 1000000000ull;    // after a failed ring build, leave Resolve alone for a while
constexpr uint64_t kGraceNs = 2ull * 1000000000ull;    // an old ring is let go this long after it was replaced

// One Feed file and the GPU resources that go with its layout.
struct Ring {
    scfeed_writer w{};
    bool pinned = false;
    std::vector<void*> staging;            // device buffers, each slot_capacity bytes
    std::vector<cudaEvent_t> stagingFree;  // complete when the copy out of that staging buffer has finished
    int nextStaging = 0;
    std::atomic<int> inFlight{0};          // copies queued and not yet published
    uint64_t retiredNs = 0;
    struct Slot { Ring* ring; int index; };
    std::vector<Slot> slots;               // callback arguments, one per Feed slot
};

std::atomic<Ring*> gRing{nullptr};  // the current Feed; replaced, never freed while it might be in use
std::mutex gMu;                     // guards everything below; render path uses try_lock only
std::vector<Ring*> gRetired;
cudaStream_t gStream = nullptr;     // the Sender's own stream: the copies to host memory run here, not on Resolve's
cudaEvent_t gStaged = nullptr;      // "the frame is in staging", recorded on Resolve's stream
uint64_t gRetryAfterNs = 0;
std::atomic<bool> gErrorShown{false};
std::string gHostName;
std::once_flag gOpenOnce;

int stagingCount()
{
    const char* v = std::getenv("SCOPECREEP_SENDER_STAGING");
    const int n = v && *v ? std::atoi(v) : 2;  // Stage 0 ran with two; graphics memory is tight, so no more
    return n < 1 ? 1 : (n > 4 ? 4 : n);
}

std::string cudaError(const char* what, cudaError_t e) { return std::string(what) + ": " + cudaGetErrorString(e); }

void setError(Ring* r, const std::string& text)
{
    if (!r || !r->w.hdr) return;
    scfeed_set_text(r->w.hdr->sender_error, sizeof r->w.hdr->sender_error, text.c_str());
    gErrorShown.store(!text.empty(), std::memory_order_relaxed);
}

// Runs on a CUDA thread when the copy into a slot has finished: publish the frame.
void CUDART_CB onCopied(void* arg)
{
    Ring::Slot* s = (Ring::Slot*)arg;
    scfeed_writer_publish(&s->ring->w, s->index);
    s->ring->inFlight.fetch_sub(1, std::memory_order_release);
}

void freeRing(Ring* r)
{
    for (void* p : r->staging) cudaFree(p);
    for (cudaEvent_t e : r->stagingFree) cudaEventDestroy(e);
    if (r->pinned) cudaHostUnregister(r->w.hdr);
    scfeed_writer_close(&r->w);
    delete r;
}

// Publish a new Feed file with a ring whose slots hold `frameBytes`, pin it, and allocate staging.
// Takes real time (it pins slots * frameBytes of memory). Caller holds gMu. Returns nullptr and `err` on failure.
Ring* buildRing(size_t frameBytes, const Ring* previous, std::string& err)
{
    if (!gStream) {
        cudaError_t e = cudaStreamCreateWithFlags(&gStream, cudaStreamNonBlocking);
        if (e != cudaSuccess) { gStream = nullptr; err = cudaError("cudaStreamCreate", e); return nullptr; }
        e = cudaEventCreateWithFlags(&gStaged, cudaEventDisableTiming);
        if (e != cudaSuccess) { err = cudaError("cudaEventCreate", e); return nullptr; }
    }
    // Staging first: it is what fails when graphics memory is short, and nothing has been published yet.
    Ring* r = new Ring();
    const size_t capacity = (frameBytes + 4095) & ~(size_t)4095;
    cudaError_t lastErr = cudaSuccess;
    for (int i = 0; i < stagingCount(); ++i) {
        void* p = nullptr;
        cudaEvent_t ev = nullptr;
        lastErr = cudaMalloc(&p, capacity);
        if (lastErr != cudaSuccess) break;
        lastErr = cudaEventCreateWithFlags(&ev, cudaEventDisableTiming);
        if (lastErr != cudaSuccess) { cudaFree(p); break; }
        cudaEventRecord(ev, gStream);  // free from the start
        r->staging.push_back(p);
        r->stagingFree.push_back(ev);
    }
    if (r->staging.empty()) {
        err = "not enough graphics memory for the Feed (" + cudaError("cudaMalloc", lastErr) + ")";
        cudaGetLastError();  // clear the sticky error so it is not blamed on Resolve
        freeRing(r);
        return nullptr;
    }
    if (scfeed_writer_create(&r->w, scfeed_path(), kSlots, capacity, SCOPECREEP_SENDER_VERSION, gHostName.c_str()) != SCFEED_OK) {
        err = std::string("cannot create the Feed at ") + scfeed_path() + ": " + std::strerror(errno);
        r->w.hdr = nullptr;
        freeRing(r);
        return nullptr;
    }
    if (previous && previous->w.hdr) {  // the counters carry on across Feed files
        const scfeed_header* o = previous->w.hdr;
        scfeed_header* n = r->w.hdr;
        n->frames_published = scfeed_load64(&o->frames_published);
        n->drops_overwritten = scfeed_load64(&o->drops_overwritten);
        n->drops_busy = scfeed_load64(&o->drops_busy);
        n->renders_total = scfeed_load64(&o->renders_total);
        n->renders_thumbnail = scfeed_load64(&o->renders_thumbnail);
        n->renders_idle = scfeed_load64(&o->renders_idle);
        n->renders_unsupported = scfeed_load64(&o->renders_unsupported);
        n->render_call_ns_total = scfeed_load64(&o->render_call_ns_total);
        n->last_render_ns = scfeed_load64(&o->last_render_ns);
    }
    const cudaError_t e = cudaHostRegister(r->w.hdr, r->w.bytes, cudaHostRegisterDefault);
    if (e != cudaSuccess) {
        // The new Feed is already published; say what went wrong in it and leave it without frames.
        setError(r, "cannot pin the Feed's memory (" + cudaError("cudaHostRegister", e) + ")");
        cudaGetLastError();
        err.clear();
    } else {
        r->pinned = true;
    }
    r->slots.resize(kSlots);
    for (uint32_t i = 0; i < kSlots; ++i) r->slots[i] = Ring::Slot{r, (int)i};
    return r;
}

// Let go of rings that were replaced a while ago and have no copies in flight. Caller holds gMu.
void reap(uint64_t nowNs)
{
    for (size_t i = 0; i < gRetired.size();) {
        Ring* r = gRetired[i];
        if (r->inFlight.load(std::memory_order_acquire) == 0 && nowNs - r->retiredNs > kGraceNs) {
            freeRing(r);
            gRetired.erase(gRetired.begin() + (long)i);
        } else {
            ++i;
        }
    }
}

}  // namespace

void feedOpen(const char* hostName)
{
    std::call_once(gOpenOnce, [&] {
        gHostName = hostName ? hostName : "";
        Ring* r = new Ring();
        if (scfeed_writer_create(&r->w, scfeed_path(), 0, 0, SCOPECREEP_SENDER_VERSION, gHostName.c_str()) != SCFEED_OK) {
            std::fprintf(stderr, "scopecreep Sender: cannot create the Feed at %s: %s\n", scfeed_path(), std::strerror(errno));
            delete r;
            return;
        }
        gRing.store(r, std::memory_order_release);
    });
}

bool feedReaderPresent()
{
    const Ring* r = gRing.load(std::memory_order_acquire);
    return r && scfeed_reader_present(r->w.hdr, scfeed_now_ns());
}

void feedCount(Render what, uint64_t renderNs, uint64_t callNs)
{
    Ring* r = gRing.load(std::memory_order_acquire);
    if (!r) return;
    scfeed_header* h = r->w.hdr;
    scfeed_add64(&h->renders_total, 1);
    if (what == Render::Idle) scfeed_add64(&h->renders_idle, 1);
    if (what == Render::Thumbnail) scfeed_add64(&h->renders_thumbnail, 1);
    if (what == Render::Unsupported) scfeed_add64(&h->renders_unsupported, 1);
    scfeed_add64(&h->render_call_ns_total, callNs);
    scfeed_store64(&h->last_render_ns, renderNs);
}

void feedSetError(const char* text)
{
    const bool clear = !text || !*text;
    if (clear && !gErrorShown.load(std::memory_order_relaxed)) return;
    setError(gRing.load(std::memory_order_acquire), clear ? "" : text);
}

bool cudaPassthrough(void* hostStream, void* dst, size_t dstRowBytes, const void* src, size_t srcRowBytes, size_t widthBytes, size_t height)
{
    return cudaMemcpy2DAsync(dst, dstRowBytes, src, srcRowBytes, widthBytes, height, cudaMemcpyDeviceToDevice, (cudaStream_t)hostStream) == cudaSuccess;
}

bool feedSubmitCuda(void* hostStream, const void* src, size_t srcRowBytes, const FrameInfo& info)
{
    Ring* r = gRing.load(std::memory_order_acquire);
    if (!r || info.width <= 0 || info.height <= 0) return false;
    std::unique_lock<std::mutex> lock(gMu, std::try_to_lock);
    if (!lock.owns_lock()) {  // another render thread is in here: do not wait for it
        scfeed_add64(&r->w.hdr->drops_busy, 1);
        return false;
    }
    r = gRing.load(std::memory_order_acquire);
    reap(info.renderNs);

    const size_t rowBytes = (size_t)info.width * 16, frameBytes = rowBytes * (size_t)info.height;
    if (r->w.hdr->slot_count == 0 || frameBytes > r->w.hdr->slot_capacity) {
        // No ring yet, or the frame has outgrown it: publish a new Feed. The app re-attaches within a few
        // milliseconds; this frame is not fed.
        if (info.renderNs < gRetryAfterNs) return false;
        std::string err;
        Ring* fresh = buildRing(frameBytes, r, err);
        if (!fresh) {
            setError(r, err);
            gRetryAfterNs = info.renderNs + kRetryNs;
            return false;
        }
        r->retiredNs = info.renderNs;
        gRetired.push_back(r);
        gRing.store(fresh, std::memory_order_release);
        return false;
    }
    if (!r->pinned) return false;  // the error is already in the Feed header

    int stg = -1;
    const int n = (int)r->staging.size();
    for (int k = 0; k < n; ++k) {
        const int c = (r->nextStaging + k) % n;
        if (cudaEventQuery(r->stagingFree[(size_t)c]) == cudaSuccess) { stg = c; break; }  // a query, not a wait
    }
    if (stg < 0) {  // every staging buffer still has a copy in flight
        cudaGetLastError();  // cudaEventQuery reports "not ready" as an error; do not leave it for Resolve to find
        scfeed_add64(&r->w.hdr->drops_busy, 1);
        return false;
    }
    cudaGetLastError();
    r->nextStaging = (stg + 1) % n;
    const int si = scfeed_writer_acquire(&r->w);
    if (si < 0) return false;  // counted by the Feed

    scfeed_slot* s = scfeed_slot_at(r->w.hdr, (uint32_t)si);
    s->pixel_format = SCFEED_PIXEL_RGBA32F;
    s->width = (uint32_t)info.width;
    s->height = (uint32_t)info.height;
    s->row_bytes = (uint32_t)rowBytes;
    s->row_order = SCFEED_ROWS_BOTTOM_UP;
    s->data_bytes = frameBytes;
    s->frame_time = info.frameTime;
    s->frame_rate = info.frameRate;
    s->render_ns = info.renderNs;
    s->publish_ns = 0;
    s->sender_instance = info.instance;
    s->flags = 0;
    s->colour_setup_provenance = 0;
    s->colour_setup_bytes = 0;

    const cudaStream_t host = (cudaStream_t)hostStream;
    void* staging = r->staging[(size_t)stg];
    cudaError_t e;
    // 1. On Resolve's stream: a private copy of the frame, so the copy to host memory can outlive this call.
    e = cudaMemcpy2DAsync(staging, rowBytes, src, srcRowBytes, rowBytes, (size_t)info.height, cudaMemcpyDeviceToDevice, host);
    if (e == cudaSuccess) e = cudaEventRecord(gStaged, host);
    // 2. On the Sender's stream, ordered after (1) by the event (the GPU waits, this thread does not).
    if (e == cudaSuccess) e = cudaStreamWaitEvent(gStream, gStaged, 0);
    if (e == cudaSuccess) e = cudaMemcpyAsync(scfeed_slot_pixels(r->w.hdr, (uint32_t)si), staging, frameBytes, cudaMemcpyDeviceToHost, gStream);
    if (e == cudaSuccess) e = cudaEventRecord(r->stagingFree[(size_t)stg], gStream);
    // 3. When the copy has finished, a callback on the Sender's stream publishes the slot.
    if (e == cudaSuccess) {
        r->inFlight.fetch_add(1, std::memory_order_acq_rel);
        e = cudaLaunchHostFunc(gStream, onCopied, &r->slots[(size_t)si]);
        if (e != cudaSuccess) r->inFlight.fetch_sub(1, std::memory_order_acq_rel);
    }
    if (e != cudaSuccess) {
        // Whatever was queued writes into a slot nobody will read. Give the slot back and say what happened.
        scfeed_writer_abandon(&r->w, si);
        setError(r, cudaError("the frame copy could not be queued", e));
        cudaGetLastError();
        return false;
    }
    if (gErrorShown.load(std::memory_order_relaxed)) setError(r, "");
    return true;
}

}  // namespace scopecreep
