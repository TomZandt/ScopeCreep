// PROTOTYPE (throwaway): offline test of the async copy + shm ring without Resolve (issue #12).
// Plays the part of Resolve: allocates a UHD float frame on the GPU, mutates it each "render", calls the same
// cudaCopyFrame() the plugin calls, at a given pace, and reports how long each call took on the caller thread.
//   cuda_copy_test [fps (0 = flat out)] [seconds] [mode staged|direct] [shm name]
// Run with probe_reader against the same shm name (see `make cuda-test`).
#include "../src/cuda_copy.h"
#include "../src/ring_layout.h"

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <thread>
#include <vector>
#include <cuda_runtime.h>

static double nowMs() { return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count(); }

__global__ void kFill(float* p, size_t n, unsigned seed)
{
    for (size_t i = (size_t)blockIdx.x * blockDim.x + threadIdx.x; i < n; i += (size_t)gridDim.x * blockDim.x) {
        unsigned x = (unsigned)i * 2654435761u ^ seed * 40503u;
        x ^= x >> 13; x *= 0x5bd1e995u; x ^= x >> 15;
        p[i] = (x & 0xffffff) / 16777216.0f;  // 0..1 floats, different every frame
    }
}

int main(int argc, char** argv)
{
    const double fps = argc > 1 ? std::atof(argv[1]) : 24;
    const double seconds = argc > 2 ? std::atof(argv[2]) : 5;
    const bool staged = !(argc > 3 && std::strcmp(argv[3], "direct") == 0);
    sc::CopyConfig cfg;
    cfg.staged = staged;
    cfg.checksum = true;
    if (argc > 4) cfg.shmName = argv[4];
    const int W = 3840, H = 2160;
    const size_t bytes = (size_t)W * H * 16;
    void *src = nullptr, *dst = nullptr;
    cudaStream_t S;
    cudaStreamCreateWithFlags(&S, cudaStreamNonBlocking);
    if (cudaMalloc(&src, bytes) || cudaMalloc(&dst, bytes)) { std::fprintf(stderr, "cudaMalloc failed\n"); return 1; }
    double t = nowMs();
    std::string err = sc::cudaCopyInit(cfg);
    std::printf("init: %s (%.0f ms)\n", err.empty() ? "OK" : err.c_str(), nowMs() - t);
    if (!err.empty()) return 1;

    double maxCall = 0, sumCall = 0;
    long calls = 0, started = 0;
    const double t0 = nowMs();
    double next = t0;
    for (unsigned i = 0; nowMs() - t0 < seconds * 1000; ++i) {
        kFill<<<2048, 256, 0, S>>>((float*)src, (size_t)W * H * 4, i + 1);  // "Resolve renders the frame" on its stream
        double a = nowMs();
        sc::cudaPassthrough(S, dst, W * 16, src, W * 16, (size_t)W * 16, H);
        bool ok = sc::cudaCopyFrame(S, src, W * 16, W, H, (double)i, 1);
        double d = nowMs() - a;
        calls++; started += ok; sumCall += d; if (d > maxCall) maxCall = d;
        if (fps > 0) {
            next += 1000.0 / fps;
            double wait = next - nowMs();
            if (wait > 0) std::this_thread::sleep_for(std::chrono::duration<double, std::milli>(wait));
        } else {
            // flat out: the host thread only runs ahead of the GPU by a few frames (queue depth limit), like a render thread
            if (i % 4 == 3) cudaStreamSynchronize(S);
        }
    }
    cudaStreamSynchronize(S);
    std::this_thread::sleep_for(std::chrono::milliseconds(300));
    sc::CopyStats cs = sc::cudaCopyStats();
    std::printf("WRITER SUMMARY mode=%s fps=%.0f calls=%ld started=%ld completed=%llu call_ms mean=%.3f max=%.3f gpu_copy_ms mean=%.2f max=%.2f "
                "latency_ms mean=%.2f max=%.2f drops: overwrite=%llu noslot=%llu nostaging=%llu\n",
                staged ? "staged" : "direct", fps, calls, started, cs.completed, sumCall / calls, maxCall,
                cs.harvested ? cs.gpuMsSum / cs.harvested : 0, cs.gpuMsMax, cs.completed ? cs.latMsSum / cs.completed : 0, cs.latMsMax,
                cs.dropOverwrite, cs.dropNoSlot, cs.dropNoStaging);
    return 0;
}
