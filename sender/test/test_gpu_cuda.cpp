// The CUDA side of the fake-host test; see test_gpu.h.
#include "test_gpu.h"

#include "ofxGPURender.h"

#include <cuda_runtime.h>

#include <cstdio>
#include <cstdlib>

namespace testgpu {

namespace {
cudaStream_t gHostStream = nullptr;
}

const char* const kEnabledProp = kOfxImageEffectPropCudaEnabled;
const char* const kHandleProp = kOfxImageEffectPropCudaStream;
const char* const kSupportedProp = kOfxImageEffectPropCudaRenderSupported;
const char* const kApiName = "CUDA";
const bool kHasStagingLimit = true;

bool init(std::string& error)
{
    if (cudaStreamCreateWithFlags(&gHostStream, cudaStreamNonBlocking) != cudaSuccess) {
        error = "CUDA is not available (is the GPU out of memory?): cannot run the Sender test";
        return false;
    }
    return true;
}

void* hostHandle() { return gHostStream; }

void* alloc(size_t bytes)
{
    void* p = nullptr;
    if (cudaMalloc(&p, bytes) != cudaSuccess) {
        std::printf("cudaMalloc failed: is the GPU out of memory?\n");
        std::exit(2);
    }
    return p;
}

void upload(void* buf, size_t pitch, const void* host, size_t rowBytes, int rows)
{
    cudaMemcpy2D(buf, pitch, host, rowBytes, rowBytes, (size_t)rows, cudaMemcpyHostToDevice);
    cudaDeviceSynchronize();  // the host stream is non-blocking: it does not wait for the legacy stream by itself
}

void zero(void* buf, size_t bytes)
{
    cudaMemset(buf, 0, bytes);
    cudaDeviceSynchronize();
}

void download(void* buf, size_t pitch, void* host, size_t rowBytes, int rows)
{
    cudaStreamSynchronize(gHostStream);
    cudaMemcpy2D(host, rowBytes, buf, pitch, rowBytes, (size_t)rows, cudaMemcpyDeviceToHost);
}

void syncHost() { cudaStreamSynchronize(gHostStream); }

void keepBusy()
{
    static void* a = alloc((size_t)128 << 20);
    static void* b = alloc((size_t)128 << 20);
    for (int i = 0; i < 400; ++i) cudaMemcpyAsync(b, a, (size_t)128 << 20, cudaMemcpyDeviceToDevice, gHostStream);  // about 0.5 ms each
}

}  // namespace testgpu
