// PROTOTYPE (throwaway): Stage 0 check 6 (issue #12). Plain C++ interface to the CUDA copy code
// (cuda_copy.cu), so the OFX plugin source needs no CUDA headers.
#pragma once
#include <cstddef>
#include <string>

namespace sc {

struct CopyConfig {
    int nslots = 4;                     // host shm slots
    int nstaging = 2;                   // device staging buffers (staged mode)
    size_t slotBytes = 3840ull * 2160 * 16;
    bool staged = true;                 // true: D2D into private staging on Resolve's stream, D2H on our own stream
                                        // false: D2H straight from Resolve's image on Resolve's stream
    bool checksum = false;              // GPU checksum of every copied frame (verification runs)
    std::string shmName = "/scopecreep-probe-ring";
};

struct CopyStats {
    unsigned long long started = 0, completed = 0, harvested = 0;
    double gpuMsSum = 0, gpuMsMax = 0;          // cudaEvent time of the D2H copy
    double latMsSum = 0, latMsMax = 0;          // wall: submission -> completion seen by host callback
    double submitMsSum = 0, submitMsMax = 0;    // wall: time spent inside cudaCopyFrame() on the render thread
    unsigned long long dropOverwrite = 0, dropNoSlot = 0, dropNoStaging = 0, dropOversize = 0, skippedThumb = 0;
};

// First call creates the shm ring, registers it with CUDA and creates stream/events/staging.
// Not thread-safe with itself (call under a lock). Returns "" or an error message.
std::string cudaCopyInit(const CopyConfig& cfg);
bool cudaCopyReady();

// Passthrough on the host's stream: dst = src (device pointers). Never blocks.
bool cudaPassthrough(void* stream, void* dst, size_t dstRowBytes, const void* src, size_t srcRowBytes,
                     size_t widthBytes, size_t height);

// Start the async copy of a float RGBA frame (device pointer) into a shm slot and return immediately.
// Returns false (and counts a drop) if the frame was skipped. Never waits on the copy or on a reader.
bool cudaCopyFrame(void* stream, const void* src, size_t srcRowBytes, int width, int height, double time, int inst);

void cudaCopyCountThumb();
// Non-blocking: collect GPU timings of completed copies, then return cumulative stats.
CopyStats cudaCopyStats();
std::string cudaCopyLastError();

// Blocking diagnostic (dump trigger only): copy a device frame to host memory.
bool cudaReadbackBlocking(void* stream, const void* src, size_t srcRowBytes, size_t widthBytes, size_t height, void* dst);

}  // namespace sc
