// The CUDA half of the Sender's frame transport (Linux and Windows); see feed_core.h for the shared half. Rules for
// this file (ADR 0003, Resolve never waits):
//  - no cudaStreamSynchronize / cudaEventSynchronize / cudaDeviceSynchronize, anywhere;
//  - the completion callback (a CUDA thread) only publishes the slot, which touches only atomics;
//  - everything that takes real time (cudaMalloc, pinning the Feed's memory, creating the stream) runs on the
//    worker thread through Backend::reserve and Backend::attach, never on a render thread.
#include "feed_core.h"

#include "platform.h"
#include "scopecreep_feed.h"

#include <cstdlib>
#include <string>
#include <vector>

#include <cuda_runtime.h>

namespace scopecreep {
namespace {

cudaStream_t gStream = nullptr;  // the Sender's own stream: the copies to host memory run here, not on Resolve's
cudaEvent_t gStaged = nullptr;   // "the frame is in staging", recorded on Resolve's stream

int stagingCount()
{
    const std::string v = envValue("SCOPECREEP_SENDER_STAGING");
    const int n = !v.empty() ? std::atoi(v.c_str()) : 2;  // Stage 0 ran with two; graphics memory is tight, so no more
    return n < 1 ? 1 : (n > 4 ? 4 : n);
}

std::string cudaError(const char* what, cudaError_t e) { return std::string(what) + ": " + cudaGetErrorString(e); }

// Test hook: a staging buffer bigger than SCOPECREEP_SENDER_GPU_LIMIT_MB fails as if the GPU were full, so running
// short can be checked without filling the GPU Resolve is using. Read on the worker thread only.
bool overGpuLimit(size_t bytes)
{
    const std::string v = envValue("SCOPECREEP_SENDER_GPU_LIMIT_MB");
    return !v.empty() && bytes > (size_t)std::strtoull(v.c_str(), nullptr, 10) * 1000000ull;
}

// What the app shows (after "Sender: ") when a staging buffer of `bytes` could not be had.
std::string outOfGpuMemory(size_t bytes)
{
    return "Not enough GPU memory in Resolve to send frames (needs about " + std::to_string((bytes + 999999) / 1000000) +
           " MB). Close another GPU app; the Sender tries again every few seconds.";
}

// Runs on a CUDA thread when the copy into a slot has finished: publish the frame.
void CUDART_CB onCopied(void* arg) { feedSlotDone((SlotRef*)arg); }

struct CudaRing : GpuRing {
    bool pinned = false;
    std::vector<void*> staging;            // device buffers, each slot_capacity bytes
    std::vector<cudaEvent_t> stagingFree;  // complete when the copy out of that staging buffer has finished
    int nextStaging = 0;
    ~CudaRing() override
    {
        for (void* p : staging) cudaFree(p);
        for (cudaEvent_t e : stagingFree) cudaEventDestroy(e);
    }
};

class CudaBackend : public Backend {
public:
    GpuRing* reserve(size_t capacity, std::string& err) override
    {
        if (!gStream) {
            cudaError_t e = cudaStreamCreateWithFlags(&gStream, cudaStreamNonBlocking);
            if (e != cudaSuccess) { gStream = nullptr; err = cudaError("cudaStreamCreate", e); return nullptr; }
            e = cudaEventCreateWithFlags(&gStaged, cudaEventDisableTiming);
            if (e != cudaSuccess) { err = cudaError("cudaEventCreate", e); return nullptr; }
        }
        CudaRing* r = new CudaRing();
        cudaError_t lastErr = cudaSuccess;
        for (int i = 0; i < stagingCount(); ++i) {
            void* p = nullptr;
            cudaEvent_t ev = nullptr;
            lastErr = overGpuLimit(capacity) ? cudaErrorMemoryAllocation : cudaMalloc(&p, capacity);
            if (lastErr != cudaSuccess) break;
            lastErr = cudaEventCreateWithFlags(&ev, cudaEventDisableTiming);
            if (lastErr != cudaSuccess) { cudaFree(p); break; }
            cudaEventRecord(ev, gStream);  // free from the start
            r->staging.push_back(p);
            r->stagingFree.push_back(ev);
        }
        if (r->staging.empty()) {
            // The picture has already passed through on Resolve's stream; only the Feed goes without.
            err = lastErr == cudaErrorMemoryAllocation ? outOfGpuMemory(capacity)
                                                       : "cannot make a staging buffer for the Feed (" + cudaError("cudaMalloc", lastErr) + ")";
            cudaGetLastError();  // clear the sticky error so it is not blamed on Resolve
            delete r;
            return nullptr;
        }
        return r;
    }

    std::string attach(GpuRing* gpu, void* base, size_t bytes) override
    {
        const cudaError_t e = cudaHostRegister(base, bytes, cudaHostRegisterDefault);
        if (e != cudaSuccess) {
            cudaGetLastError();
            return "cannot pin the Feed's memory (" + cudaError("cudaHostRegister", e) + ")";
        }
        static_cast<CudaRing*>(gpu)->pinned = true;
        return "";
    }

    void detach(GpuRing* gpu, void* base) override
    {
        CudaRing* r = static_cast<CudaRing*>(gpu);
        if (r->pinned) cudaHostUnregister(base);
        r->pinned = false;
    }
};

// One frame: the staging buffer picked in ready(), then the three queued steps in start().
class CudaCopier : public Copier {
public:
    CudaCopier(void* hostStream, const void* src, size_t srcRowBytes) : host_((cudaStream_t)hostStream), src_(src), srcRowBytes_(srcRowBytes) {}

    Ready ready(GpuRing* gpu) override
    {
        CudaRing* r = static_cast<CudaRing*>(gpu);
        const int n = (int)r->staging.size();
        for (int k = 0; k < n; ++k) {
            const int c = (r->nextStaging + k) % n;
            if (cudaEventQuery(r->stagingFree[(size_t)c]) == cudaSuccess) { staging_ = c; break; }  // a query, not a wait
        }
        cudaGetLastError();  // cudaEventQuery reports "not ready" as an error; do not leave it for Resolve to find
        if (staging_ < 0) return Ready::Busy;  // every staging buffer still has a copy in flight
        r->nextStaging = (staging_ + 1) % n;
        return Ready::Yes;
    }

    std::string start(GpuRing* gpu, const FrameCopy& copy) override
    {
        CudaRing* r = static_cast<CudaRing*>(gpu);
        void* staging = r->staging[(size_t)staging_];
        cudaError_t e;
        // 1. On Resolve's stream: a private copy of the frame, so the copy to host memory can outlive this call.
        e = cudaMemcpy2DAsync(staging, copy.rowBytes, src_, srcRowBytes_, copy.rowBytes, (size_t)copy.height, cudaMemcpyDeviceToDevice, host_);
        if (e == cudaSuccess) e = cudaEventRecord(gStaged, host_);
        // 2. On the Sender's stream, ordered after (1) by the event (the GPU waits, this thread does not).
        if (e == cudaSuccess) e = cudaStreamWaitEvent(gStream, gStaged, 0);
        if (e == cudaSuccess) e = cudaMemcpyAsync(copy.dstPixels, staging, copy.frameBytes, cudaMemcpyDeviceToHost, gStream);
        if (e == cudaSuccess) e = cudaEventRecord(r->stagingFree[(size_t)staging_], gStream);
        // 3. When the copy has finished, a callback on the Sender's stream publishes the slot.
        if (e == cudaSuccess) e = cudaLaunchHostFunc(gStream, onCopied, copy.done);
        if (e == cudaSuccess) return "";
        cudaGetLastError();
        return cudaError("the frame copy could not be queued", e);
    }

private:
    cudaStream_t host_;
    const void* src_;
    size_t srcRowBytes_;
    int staging_ = -1;
};

}  // namespace

Backend& backend()
{
    static CudaBackend b;
    return b;
}

bool cudaPassthrough(void* hostStream, void* dst, size_t dstRowBytes, const void* src, size_t srcRowBytes, size_t widthBytes, size_t height)
{
    return cudaMemcpy2DAsync(dst, dstRowBytes, src, srcRowBytes, widthBytes, height, cudaMemcpyDeviceToDevice, (cudaStream_t)hostStream) == cudaSuccess;
}

bool feedSubmitCuda(void* hostStream, const void* src, size_t srcRowBytes, const FrameInfo& info)
{
    CudaCopier copier(hostStream, src, srcRowBytes);
    return feedSubmit(info, copier);
}

}  // namespace scopecreep
