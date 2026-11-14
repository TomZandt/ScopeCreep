// PROTOTYPE (throwaway): Stage 0 check 6 (issue #12). Asynchronous GPU -> shared-memory frame copy.
//
// Everything that touches CUDA lives here. Rules (ADR 0003, Resolve never waits):
//  - cudaCopyFrame() only enqueues work and returns; no cudaStreamSynchronize / cudaEventSynchronize anywhere
//    on the render path (the one blocking call, cudaReadbackBlocking, is the manual dump-trigger diagnostic);
//  - completion is learned through a host callback enqueued on our own stream, never polled by waiting;
//  - a full ring overwrites the oldest unread slot (counted); if nothing can be taken the frame is skipped (counted).
#include "cuda_copy.h"
#include "ring_layout.h"

#include <chrono>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <vector>

#include <cuda_runtime.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

namespace sc {
namespace {

struct SlotRec {                 // host-side bookkeeping per slot
    cudaEvent_t evStart = nullptr, evEnd = nullptr;
    unsigned long long* dChk = nullptr;   // device-side checksum accumulator
    std::atomic<int> harvest{0};          // 0 idle, 1 in flight, 2 callback done (events complete)
    uint32_t index = 0;
};

std::mutex gMu;                  // guards slot/staging acquisition and stats (held for microseconds, never across GPU waits)
bool gReady = false;
CopyConfig gCfg;
void* gBase = nullptr;
size_t gBytes = 0;
RingHeader* gHdr = nullptr;
cudaStream_t gStream = nullptr;
std::vector<SlotRec*> gSlots;
std::vector<void*> gStaging;
std::vector<cudaEvent_t> gStgDone;
cudaEvent_t gEvSrc = nullptr;
int gNextStaging = 0;
CopyStats gStats;
std::string gLastErr;

double nowMs()
{
    return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

bool check(cudaError_t e, const char* what)
{
    if (e == cudaSuccess) return true;
    gLastErr = std::string(what) + ": " + cudaGetErrorString(e);
    return false;
}

__global__ void kChecksum(const unsigned char* base, size_t pitch, unsigned w4, size_t n, unsigned long long* out)
{
    unsigned long long local = 0;
    for (size_t i = (size_t)blockIdx.x * blockDim.x + threadIdx.x; i < n; i += (size_t)gridDim.x * blockDim.x) {
        const size_t y = i / w4, x = i % w4;
        const unsigned v = *(const unsigned*)(base + y * pitch + x * 4);
        local += (unsigned long long)v * (2ull * i + 1ull);
    }
    for (int off = 16; off > 0; off >>= 1) local += __shfl_down_sync(0xffffffffu, local, off);
    if ((threadIdx.x & 31) == 0) atomicAdd(out, local);
}

void CUDART_CB onComplete(void* arg)
{
    SlotRec* r = (SlotRec*)arg;
    SlotHeader* sh = slotHeader(gBase, r->index, gCfg.slotBytes);
    const double now = nowMs();
    sh->completeWallMs = now;
    const double lat = now - sh->submitWallMs;
    {
        std::lock_guard<std::mutex> l(gMu);
        gStats.completed++;
        gStats.latMsSum += lat;
        if (lat > gStats.latMsMax) gStats.latMsMax = lat;
    }
    r->harvest.store(2, std::memory_order_release);
    gHdr->published.fetch_add(1, std::memory_order_relaxed);
    sh->state.store(kReady, std::memory_order_release);
}

// Collect the GPU time of a finished copy (non-blocking: the host callback has already run, so both events are complete).
// Caller holds gMu.
void harvestOne(SlotRec* r)
{
    int exp = 2;
    if (r->harvest.compare_exchange_strong(exp, 0)) {
        float ms = 0;
        if (cudaEventElapsedTime(&ms, r->evStart, r->evEnd) == cudaSuccess) {
            gStats.harvested++;
            gStats.gpuMsSum += ms;
            if (ms > gStats.gpuMsMax) gStats.gpuMsMax = ms;
        }
    }
}

// Returns slot index or -1. Caller holds gMu.
int acquireSlot()
{
    for (int attempt = 0; attempt < 2; ++attempt) {
        for (uint32_t i = 0; i < (uint32_t)gCfg.nslots; ++i) {
            SlotHeader* sh = slotHeader(gBase, i, gCfg.slotBytes);
            uint32_t exp = kFree;
            if (sh->state.compare_exchange_strong(exp, kWriting, std::memory_order_acq_rel)) return (int)i;
        }
        int oldest = -1;
        uint64_t oldestSeq = ~0ull;
        for (uint32_t i = 0; i < (uint32_t)gCfg.nslots; ++i) {
            SlotHeader* sh = slotHeader(gBase, i, gCfg.slotBytes);
            if (sh->state.load(std::memory_order_acquire) == kReady && sh->seq < oldestSeq) { oldestSeq = sh->seq; oldest = (int)i; }
        }
        if (oldest < 0) return -1;
        SlotHeader* sh = slotHeader(gBase, (uint32_t)oldest, gCfg.slotBytes);
        uint32_t exp = kReady;
        if (sh->state.compare_exchange_strong(exp, kWriting, std::memory_order_acq_rel)) {
            gHdr->dropOverwrite.fetch_add(1, std::memory_order_relaxed);
            gStats.dropOverwrite++;
            return oldest;
        }
        // the reader grabbed it between our scan and the CAS: look again
    }
    return -1;
}

}  // namespace

bool cudaCopyReady() { return gReady; }
std::string cudaCopyLastError() { return gLastErr; }

std::string cudaCopyInit(const CopyConfig& cfg)
{
    if (gReady) return "";
    gCfg = cfg;
    gCfg.slotBytes = (gCfg.slotBytes + 4095) & ~(size_t)4095;
    gBytes = ringBytes((uint32_t)gCfg.nslots, gCfg.slotBytes);

    int fd = shm_open(gCfg.shmName.c_str(), O_RDWR | O_CREAT, 0666);
    if (fd < 0) return std::string("shm_open: ") + std::strerror(errno);
    if (ftruncate(fd, 0) != 0 || ftruncate(fd, (off_t)gBytes) != 0) { close(fd); return std::string("ftruncate: ") + std::strerror(errno); }
    gBase = mmap(nullptr, gBytes, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (gBase == MAP_FAILED) { gBase = nullptr; return std::string("mmap: ") + std::strerror(errno); }
    std::memset(gBase, 0, kHeaderBytes);  // header; slot headers are zero already (fresh file)
    gHdr = (RingHeader*)gBase;
    gHdr->magic = kRingMagic;
    gHdr->version = kRingVersion;
    gHdr->nslots = (uint32_t)gCfg.nslots;
    gHdr->slotBytes = gCfg.slotBytes;

    // Pin the whole ring once. This faults in and locks ~nslots*133 MB: it costs hundreds of ms, once.
    if (!check(cudaHostRegister(gBase, gBytes, cudaHostRegisterPortable), "cudaHostRegister")) return gLastErr;
    if (!check(cudaStreamCreateWithFlags(&gStream, cudaStreamNonBlocking), "cudaStreamCreate")) return gLastErr;
    for (int i = 0; i < gCfg.nslots; ++i) {
        SlotRec* r = new SlotRec();
        r->index = (uint32_t)i;
        if (!check(cudaEventCreate(&r->evStart), "cudaEventCreate") || !check(cudaEventCreate(&r->evEnd), "cudaEventCreate") ||
            !check(cudaMalloc((void**)&r->dChk, 8), "cudaMalloc chk"))
            return gLastErr;
        gSlots.push_back(r);
    }
    if (!check(cudaEventCreateWithFlags(&gEvSrc, cudaEventDisableTiming), "cudaEventCreate")) return gLastErr;
    if (gCfg.staged) {
        for (int i = 0; i < gCfg.nstaging; ++i) {
            void* p = nullptr;
            if (!check(cudaMalloc(&p, gCfg.slotBytes), "cudaMalloc staging")) return gLastErr;
            cudaEvent_t e;
            if (!check(cudaEventCreateWithFlags(&e, cudaEventDisableTiming), "cudaEventCreate")) return gLastErr;
            cudaEventRecord(e, gStream);  // "done" from the start
            gStaging.push_back(p);
            gStgDone.push_back(e);
        }
    }
    gReady = true;
    return "";
}

bool cudaPassthrough(void* stream, void* dst, size_t dstRowBytes, const void* src, size_t srcRowBytes,
                     size_t widthBytes, size_t height)
{
    return check(cudaMemcpy2DAsync(dst, dstRowBytes, src, srcRowBytes, widthBytes, height, cudaMemcpyDeviceToDevice,
                                   (cudaStream_t)stream), "passthrough");
}

bool cudaCopyFrame(void* stream, const void* src, size_t srcRowBytes, int width, int height, double time, int inst)
{
    const double t0 = nowMs();
    const size_t wBytes = (size_t)width * 16, frameBytes = wBytes * (size_t)height;
    cudaStream_t S = (cudaStream_t)stream;
    std::lock_guard<std::mutex> lock(gMu);
    if (!gReady) return false;
    if (frameBytes > gCfg.slotBytes) { gHdr->dropOversize.fetch_add(1); gStats.dropOversize++; return false; }

    int stg = -1;
    if (gCfg.staged) {
        for (int k = 0; k < gCfg.nstaging; ++k) {
            int c = (gNextStaging + k) % gCfg.nstaging;
            if (cudaEventQuery(gStgDone[c]) == cudaSuccess) { stg = c; break; }  // non-blocking query
        }
        if (stg < 0) { gHdr->dropNoStaging.fetch_add(1); gStats.dropNoStaging++; return false; }
        gNextStaging = (stg + 1) % gCfg.nstaging;
    }
    const int si = acquireSlot();
    if (si < 0) { gHdr->dropNoSlot.fetch_add(1); gStats.dropNoSlot++; return false; }
    SlotRec* rec = gSlots[(size_t)si];
    harvestOne(rec);  // the slot's previous copy has completed (it was FREE or READY): bank its GPU time before reuse
    SlotHeader* sh = slotHeader(gBase, (uint32_t)si, gCfg.slotBytes);
    void* host = slotData(gBase, (uint32_t)si, gCfg.slotBytes);
    sh->seq = gHdr->nextSeq.fetch_add(1);
    sh->time = time;
    sh->inst = (uint32_t)inst;
    sh->width = (uint32_t)width;
    sh->height = (uint32_t)height;
    sh->hasChecksum = gCfg.checksum ? 1 : 0;
    sh->checksum = 0;
    sh->submitWallMs = t0;
    // A previous copy's GPU timing not yet harvested is simply lost for that slot (overwritten slot).
    rec->harvest.store(1, std::memory_order_release);

    bool ok = true;
    const unsigned w4 = (unsigned)width * 4;
    const size_t nwords = (size_t)w4 * (size_t)height;
    if (gCfg.staged) {
        // 1) on Resolve's stream: private device copy of the frame (cheap D2D) so the host copy can outlive render()
        ok &= check(cudaMemcpy2DAsync(gStaging[(size_t)stg], wBytes, src, srcRowBytes, wBytes, (size_t)height,
                                      cudaMemcpyDeviceToDevice, S), "stage D2D");
        ok &= check(cudaEventRecord(gEvSrc, S), "event src");
        // 2) on our own stream, ordered after (1) by an event, not by waiting on the CPU
        ok &= check(cudaStreamWaitEvent(gStream, gEvSrc, 0), "wait src");
        ok &= check(cudaEventRecord(rec->evStart, gStream), "event start");
        ok &= check(cudaMemcpyAsync(host, gStaging[(size_t)stg], frameBytes, cudaMemcpyDeviceToHost, gStream), "D2H");
        if (gCfg.checksum) {
            ok &= check(cudaMemsetAsync(rec->dChk, 0, 8, gStream), "memset chk");
            kChecksum<<<1024, 256, 0, gStream>>>((const unsigned char*)gStaging[(size_t)stg], wBytes, w4, nwords, rec->dChk);
            ok &= check(cudaGetLastError(), "kChecksum");
            ok &= check(cudaMemcpyAsync(&sh->checksum, rec->dChk, 8, cudaMemcpyDeviceToHost, gStream), "chk D2H");
        }
        ok &= check(cudaEventRecord(rec->evEnd, gStream), "event end");
        ok &= check(cudaEventRecord(gStgDone[(size_t)stg], gStream), "event staging done");
    } else {
        // Direct: the D2H is queued on Resolve's own stream, so it delays Resolve's later GPU work by the copy time.
        ok &= check(cudaEventRecord(rec->evStart, S), "event start");
        ok &= check(cudaMemcpy2DAsync(host, wBytes, src, srcRowBytes, wBytes, (size_t)height, cudaMemcpyDeviceToHost, S), "D2H direct");
        if (gCfg.checksum) {
            ok &= check(cudaMemsetAsync(rec->dChk, 0, 8, S), "memset chk");
            // memset must precede the kernel: same stream, ordered
            kChecksum<<<1024, 256, 0, S>>>((const unsigned char*)src, srcRowBytes, w4, nwords, rec->dChk);
            ok &= check(cudaGetLastError(), "kChecksum");
            ok &= check(cudaMemcpyAsync(&sh->checksum, rec->dChk, 8, cudaMemcpyDeviceToHost, S), "chk D2H");
        }
        ok &= check(cudaEventRecord(rec->evEnd, S), "event end");
        ok &= check(cudaStreamWaitEvent(gStream, rec->evEnd, 0), "wait end");
    }
    // 3) completion notification: a host function on our stream publishes the slot when the copy is done
    ok &= check(cudaLaunchHostFunc(gStream, onComplete, rec), "cudaLaunchHostFunc");
    if (!ok) {
        // enqueue failed part-way: free the slot (the half-queued work is harmless, it writes into a slot nobody reads as READY)
        rec->harvest.store(0);
        sh->state.store(kFree, std::memory_order_release);
        return false;
    }
    gStats.started++;
    const double dt = nowMs() - t0;
    gStats.submitMsSum += dt;
    if (dt > gStats.submitMsMax) gStats.submitMsMax = dt;
    return true;
}

void cudaCopyCountThumb()
{
    if (!gReady) return;
    gHdr->skippedThumb.fetch_add(1);
    std::lock_guard<std::mutex> lock(gMu);
    gStats.skippedThumb++;
}

CopyStats cudaCopyStats()
{
    std::lock_guard<std::mutex> lock(gMu);
    for (SlotRec* r : gSlots) harvestOne(r);
    return gStats;
}

void cudaCopyResetIntervalMax()
{
    std::lock_guard<std::mutex> lock(gMu);
    gStats.gpuMsMax = gStats.latMsMax = gStats.submitMsMax = 0;
}

bool cudaReadbackBlocking(void* stream, const void* src, size_t srcRowBytes, size_t widthBytes, size_t height, void* dst)
{
    cudaStream_t S = (cudaStream_t)stream;
    bool ok = check(cudaMemcpy2DAsync(dst, widthBytes, src, srcRowBytes, widthBytes, height, cudaMemcpyDeviceToHost, S), "readback");
    ok &= check(cudaStreamSynchronize(S), "readback sync");
    return ok;
}

}  // namespace sc
