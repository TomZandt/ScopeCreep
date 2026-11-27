// See feed_core.h: the Feed, the ring, the registry and the worker, the same for every GPU API. Rules for this file
// (ADR 0003, Resolve never waits):
//  - nothing here waits for the GPU (no stream, event or fence waits, anywhere, and none in a backend either);
//  - the one mutex is only ever taken with try_lock on the render path: if another render thread holds it, this
//    frame is skipped and counted, not waited for;
//  - a backend's completion callback (a GPU driver thread) touches only atomics;
//  - everything that takes real time runs on the Sender's own worker thread, never on a render thread: building a
//    ring (the backend's reservations, pinning or wrapping the Feed's memory) the first time an app is attached and
//    when a larger frame arrives, letting an old ring go, and a guest's once-a-second look at the other Resolve's
//    Feed (scanning and mapping files). A render call only reads state, sets a flag and wakes the worker; a frame
//    that arrives while a ring is being built passes through and is counted as dropped ("Sender busy").
#include "feed_core.h"

#include "platform.h"
#include "scopecreep_feed.h"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#ifndef SCOPECREEP_SENDER_VERSION
#define SCOPECREEP_SENDER_VERSION "0.0.0"
#endif

namespace scopecreep {
namespace {

constexpr uint32_t kSlots = 4;                         // Stage 0: four slots held UHD at 60 fps with no drops
constexpr uint64_t kRetryNs = 5ull * 1000000000ull;    // after a failed ring build, leave Resolve alone for a while
constexpr uint64_t kGraceNs = 2ull * 1000000000ull;    // an old ring is let go this long after it was replaced

}  // namespace

// One Feed file and the GPU resources that go with its layout.
struct Ring {
    scfeed_writer w{};
    GpuRing* gpu = nullptr;                // the backend's part; freed with the ring
    bool usable = false;                   // the backend has tied the Feed's memory to the GPU
    std::atomic<int> inFlight{0};          // copies started and not yet published
    uint64_t retiredNs = 0;
    std::vector<SlotRef> slots;            // callback arguments, one per Feed slot
};

namespace {

std::atomic<Ring*> gRing{nullptr};  // the current Feed; replaced, never freed while it might be in use
std::mutex gMu;                     // guards everything below; render path uses try_lock only
std::vector<Ring*> gRetired;
std::atomic<uint64_t> gRetryAfterNs{0};
// The worker. Its state is leaked on purpose: the thread is detached and may still be waiting when the process
// exits, and a condition variable must not be destroyed with a waiter.
std::mutex& gWorkMu = *new std::mutex;
std::condition_variable& gWorkCv = *new std::condition_variable;
std::atomic<size_t> gWantFrameBytes{0};   // a render call saw a frame this big that the ring cannot hold; 0 = none
std::atomic<bool> gGuestCheckDue{false};  // a render call asks for the guest's once-a-second look at the Feed
std::atomic<bool> gHaveRetired{false};    // gRetired is not empty (written under gMu)
std::atomic<bool> gErrorShown{false};
std::string gHostName;
std::once_flag gOpenOnce, gWorkerOnce;
// While this process is a guest: the header of the other Resolve's Feed (never unmapped: a render thread may
// still be using an old one; they are 4 KB and replaced rarely). nullptr while this process owns the Feed.
std::atomic<scfeed_header*> gGuest{nullptr};
// What the OS needs kept for the current gGuest (Windows: its handles, ADR 0049). Written under gMu or in
// feedOpen's call_once. An old guest's view stays mapped after its handles close; it keeps its memory.
scfeed_os_feed gGuestHeld{};
std::atomic<uint64_t> gNextGuestCheckNs{0};
constexpr uint64_t kGuestCheckNs = 1000000000ull;
std::vector<std::pair<uint32_t, double>> gRegistered;  // this process's instances (guarded by gMu)

uint64_t myPid() { return scfeed_pid(); }

// The header the registry is in: the other Resolve's Feed while a guest, else our own.
scfeed_header* registryHeader()
{
    if (scfeed_header* g = gGuest.load(std::memory_order_acquire)) return g;
    Ring* r = gRing.load(std::memory_order_acquire);
    return r ? r->w.hdr : nullptr;
}

// Publish a Feed with no ring and make it current. Caller holds gMu or is in feedOpen's call_once.
bool publishEmptyFeed()
{
    Ring* r = new Ring();
    if (scfeed_writer_create(&r->w, scfeed_path(), 0, 0, SCOPECREEP_SENDER_VERSION, gHostName.c_str()) != SCFEED_OK) {
        std::fprintf(stderr, "ScopeCreep: cannot create the Feed at %s: %s\n", scfeed_path(), lastOsError().c_str());
        delete r;
        return false;
    }
    if (Ring* old = gRing.exchange(r, std::memory_order_acq_rel)) {
        old->retiredNs = scfeed_now_ns();
        gRetired.push_back(old);
        gHaveRetired.store(true, std::memory_order_relaxed);
    }
    return true;
}

void setError(Ring* r, const std::string& text)
{
    if (!r || !r->w.hdr) return;
    scfeed_set_text(r->w.hdr->sender_error, sizeof r->w.hdr->sender_error, text.c_str());
    gErrorShown.store(!text.empty(), std::memory_order_relaxed);
}

void freeRing(Ring* r)
{
    if (r->gpu && r->w.hdr) backend().detach(r->gpu, r->w.hdr);
    scfeed_writer_close(&r->w);
    delete r->gpu;
    delete r;
}

// Publish a new Feed file with a ring whose slots hold `frameBytes`, and tie it to the GPU.
// Takes real time (a backend may pin or wrap slots * frameBytes of memory): worker thread only, no lock held.
// Returns nullptr and `err` on failure.
Ring* buildRing(size_t frameBytes, std::string& err)
{
    // The backend's reservations first: they are what fails when graphics memory is short, and nothing has been
    // published yet.
    const size_t capacity = (frameBytes + 4095) & ~(size_t)4095;
    GpuRing* gpu = backend().reserve(capacity, err);
    if (!gpu) return nullptr;
    Ring* r = new Ring();
    r->gpu = gpu;
    if (scfeed_writer_create(&r->w, scfeed_path(), kSlots, capacity, SCOPECREEP_SENDER_VERSION, gHostName.c_str()) != SCFEED_OK) {
        err = std::string("cannot create the Feed at ") + scfeed_path() + ": " + lastOsError();
        r->w.hdr = nullptr;
        freeRing(r);
        return nullptr;
    }
    const std::string problem = backend().attach(gpu, r->w.hdr, r->w.bytes);
    if (!problem.empty()) {
        // The new Feed is already published; say what went wrong in it and leave it without frames.
        setError(r, problem);
        err.clear();
    } else {
        r->usable = true;
    }
    r->slots.resize(kSlots);
    for (uint32_t i = 0; i < kSlots; ++i) r->slots[i] = SlotRef{r, (int)i};
    return r;
}

// Let go of rings that were replaced a while ago and have no copies in flight. Worker thread only; takes gMu just
// long enough to pick them, and frees them (the GPU side lets go of its memory) outside it. Every ring that was replaced ends up here,
// whatever the render threads do afterwards, because the worker keeps coming back until gRetired is empty.
void reap(uint64_t nowNs)
{
    std::vector<Ring*> done;
    {
        std::lock_guard<std::mutex> lock(gMu);
        for (size_t i = 0; i < gRetired.size();) {
            Ring* r = gRetired[i];
            if (r->inFlight.load(std::memory_order_acquire) == 0 && nowNs - r->retiredNs > kGraceNs) {
                done.push_back(r);
                gRetired.erase(gRetired.begin() + (long)i);
            } else {
                ++i;
            }
        }
        gHaveRetired.store(!gRetired.empty(), std::memory_order_relaxed);
    }
    for (Ring* r : done) freeRing(r);
}

// Make `fresh` the current Feed and retire the ring it replaces. Worker thread only.
void swapIn(Ring* fresh)
{
    std::lock_guard<std::mutex> lock(gMu);
    Ring* previous = gRing.load(std::memory_order_acquire);
    // The counters and the registry carry on across Feed files; copied now, as late as possible, so renders
    // counted while the ring was being built are not lost.
    if (previous && previous->w.hdr) {
        const scfeed_header* o = previous->w.hdr;
        scfeed_header* n = fresh->w.hdr;
        scfeed_registry_copy(n, o, 0);
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
    if (Ring* old = gRing.exchange(fresh, std::memory_order_acq_rel)) {
        old->retiredNs = scfeed_now_ns();
        gRetired.push_back(old);
        gHaveRetired.store(true, std::memory_order_relaxed);
    }
}

// Build a ring that holds `frameBytes` and make it current. Worker thread only.
void growRing(size_t frameBytes)
{
    Ring* r = gRing.load(std::memory_order_acquire);
    if (!r) return;
    if (r->w.hdr->slot_count != 0 && frameBytes <= r->w.hdr->slot_capacity) return;  // already done
    const uint64_t now = scfeed_now_ns();
    if (now < gRetryAfterNs.load(std::memory_order_relaxed)) return;
    std::string err;
    Ring* fresh = buildRing(frameBytes, err);
    if (!fresh) {
        setError(r, err);
        gRetryAfterNs.store(scfeed_now_ns() + kRetryNs, std::memory_order_relaxed);
        return;
    }
    swapIn(fresh);
}

// A guest's once-a-second look at the Feed: has the owner replaced its Feed file or quit? The scanning and
// mapping happen without gMu; the state change takes it.
void refreshGuest()
{
    scfeed_header* guest = gGuest.load(std::memory_order_acquire);
    if (!guest) return;
    scfeed_os_feed held;
    scfeed_header* owner = scfeed_live_feed_of_another(scfeed_path(), myPid(), &held);
    if (owner && owner->feed_id == guest->feed_id) {
        scfeed_header_unmap(owner, &held);
        return;
    }
    std::lock_guard<std::mutex> lock(gMu);
    scfeed_os_close_kept(&gGuestHeld);
    gGuestHeld = held;
    if (owner) {
        gGuest.store(owner, std::memory_order_release);  // the owner published a new Feed file: follow it
    } else {
        // The owner has quit (or its Feed is gone): this process takes the Feed over.
        if (!publishEmptyFeed()) return;
        gGuest.store(nullptr, std::memory_order_release);
    }
    scfeed_header* h = registryHeader();
    for (const auto& reg : gRegistered) scfeed_registry_add(h, myPid(), reg.first, reg.second, gHostName.c_str());
}

void workerLoop()
{
    for (;;) {
        {
            std::unique_lock<std::mutex> lock(gWorkMu);
            auto due = [] {
                return gWantFrameBytes.load(std::memory_order_acquire) != 0 || gGuestCheckDue.load(std::memory_order_acquire);
            };
            if (gHaveRetired.load(std::memory_order_relaxed)) gWorkCv.wait_for(lock, std::chrono::milliseconds(250), due);
            else gWorkCv.wait(lock, due);  // nothing to do: asleep until a render call asks for something
        }
        if (const size_t want = gWantFrameBytes.exchange(0, std::memory_order_acq_rel)) growRing(want);
        if (gGuestCheckDue.exchange(false, std::memory_order_acq_rel)) refreshGuest();
        reap(scfeed_now_ns());
    }
}

void wakeWorker() { gWorkCv.notify_one(); }  // never waits; a wake that is missed is repeated by the next render

}  // namespace

void feedOpen(const char* hostName)
{
    std::call_once(gOpenOnce, [&] {
        gHostName = hostName ? hostName : "";
        // The first running Resolve keeps the Feed; a second one joins it as a guest.
        if (scfeed_header* owner = scfeed_live_feed_of_another(scfeed_path(), myPid(), &gGuestHeld)) {
            gGuest.store(owner, std::memory_order_release);
            gNextGuestCheckNs.store(scfeed_now_ns() + kGuestCheckNs, std::memory_order_relaxed);
            return;
        }
        publishEmptyFeed();
    });
    std::call_once(gWorkerOnce, [] { std::thread(workerLoop).detach(); });
}

bool feedIsGuest() { return gGuest.load(std::memory_order_acquire) != nullptr; }

void feedRegister(uint32_t instance, double frameRate)
{
    std::lock_guard<std::mutex> lock(gMu);
    gRegistered.emplace_back(instance, frameRate);
    if (scfeed_header* h = registryHeader()) scfeed_registry_add(h, myPid(), instance, frameRate, gHostName.c_str());
}

void feedUnregister(uint32_t instance)
{
    std::lock_guard<std::mutex> lock(gMu);
    for (size_t i = 0; i < gRegistered.size(); ++i)
        if (gRegistered[i].first == instance) { gRegistered.erase(gRegistered.begin() + (long)i); break; }
    if (scfeed_header* h = registryHeader()) scfeed_registry_remove(h, myPid(), instance);
}

void feedNoteRender(uint32_t instance, double frameTime, uint64_t renderNs)
{
    scfeed_header* h = registryHeader();
    if (!h) return;
    scfeed_registry_note(h, myPid(), instance, frameTime, renderNs);
    if (!gGuest.load(std::memory_order_acquire) || renderNs < gNextGuestCheckNs.load(std::memory_order_relaxed)) return;
    // The look at the owner's Feed (file scanning, mapping) is the worker's job, not a render thread's.
    gNextGuestCheckNs.store(renderNs + kGuestCheckNs, std::memory_order_relaxed);
    gGuestCheckDue.store(true, std::memory_order_release);
    wakeWorker();
}

bool feedReaderPresent()
{
    // A guest reports the owner's app too, so Resolve keeps calling render and the renders can be registered
    // (the app flags them). A guest never copies a frame.
    if (const scfeed_header* g = gGuest.load(std::memory_order_acquire)) return scfeed_reader_present(g, scfeed_now_ns());
    const Ring* r = gRing.load(std::memory_order_acquire);
    return r && scfeed_reader_present(r->w.hdr, scfeed_now_ns());
}

void feedCount(Render what, uint64_t renderNs, uint64_t callNs)
{
    if (feedIsGuest()) return;  // the counters belong to the owner's renders
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
    if (feedIsGuest()) return;  // the owner's error line is not ours to write
    const bool clear = !text || !*text;
    if (clear && !gErrorShown.load(std::memory_order_relaxed)) return;
    setError(gRing.load(std::memory_order_acquire), clear ? "" : text);
}

void feedSlotDone(SlotRef* s)
{
    scfeed_writer_publish(&s->ring->w, s->index);
    s->ring->inFlight.fetch_sub(1, std::memory_order_release);
}

void feedSlotFailed(SlotRef* s, const std::string& text)
{
    scfeed_writer_abandon(&s->ring->w, s->index);
    setError(s->ring, text);
    s->ring->inFlight.fetch_sub(1, std::memory_order_release);
}

bool feedSubmit(const FrameInfo& info, Copier& copier)
{
    if (feedIsGuest()) return false;  // a guest sends no frames
    Ring* r = gRing.load(std::memory_order_acquire);
    if (!r || info.width <= 0 || info.height <= 0) return false;
    std::unique_lock<std::mutex> lock(gMu, std::try_to_lock);
    if (!lock.owns_lock()) {  // another render thread is in here: do not wait for it
        scfeed_add64(&r->w.hdr->drops_busy, 1);
        return false;
    }
    r = gRing.load(std::memory_order_acquire);

    const size_t rowBytes = (size_t)info.width * 16, frameBytes = rowBytes * (size_t)info.height;
    if (r->w.hdr->slot_count == 0 || frameBytes > r->w.hdr->slot_capacity) {
        // No ring yet, or the frame has outgrown it: the worker publishes a new Feed (the app re-attaches within
        // a few milliseconds). This frame is not fed; it is counted as dropped because the Sender is busy.
        if (info.renderNs >= gRetryAfterNs.load(std::memory_order_relaxed)) {
            size_t was = gWantFrameBytes.load(std::memory_order_relaxed);
            while (frameBytes > was && !gWantFrameBytes.compare_exchange_weak(was, frameBytes, std::memory_order_acq_rel)) {}
            wakeWorker();
        }
        scfeed_add64(&r->w.hdr->drops_busy, 1);
        return false;
    }
    if (!r->usable) return false;  // the error is already in the Feed header

    switch (copier.ready(r->gpu)) {
    case Ready::Yes: break;
    case Ready::Busy: scfeed_add64(&r->w.hdr->drops_busy, 1); return false;
    case Ready::No: return false;
    }
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
    scfeed_slot_set_colour_setup(s, info.colourProvenance, info.colourSetup, info.colourSetupBytes);

    FrameCopy copy;
    copy.dstPixels = scfeed_slot_pixels(r->w.hdr, (uint32_t)si);
    copy.dstOffset = (size_t)((const char*)copy.dstPixels - (const char*)r->w.hdr);
    copy.rowBytes = rowBytes;
    copy.frameBytes = frameBytes;
    copy.height = info.height;
    copy.done = &r->slots[(size_t)si];
    r->inFlight.fetch_add(1, std::memory_order_acq_rel);  // before the copy can finish and take it off again
    const std::string failure = copier.start(r->gpu, copy);
    if (!failure.empty()) {
        // Whatever was queued writes into a slot nobody will read. Give the slot back and say what happened.
        r->inFlight.fetch_sub(1, std::memory_order_acq_rel);
        scfeed_writer_abandon(&r->w, si);
        setError(r, failure);
        return false;
    }
    if (gErrorShown.load(std::memory_order_relaxed)) setError(r, "");
    return true;
}

}  // namespace scopecreep
