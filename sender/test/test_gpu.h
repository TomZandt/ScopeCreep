// What the fake-host test needs from the GPU API its platform gives Resolve: CUDA (test_gpu_cuda.cpp) on Linux and
// Windows, Metal (test_gpu_metal.mm) on macOS. The test itself (mock_host_test.cpp) stays the same on all of them;
// the Makefile or CMake picks the file that implements this.
//
// A "buffer" is whatever Resolve hands a plugin as an image: a device pointer on CUDA, an id<MTLBuffer> (private
// storage, so the CPU cannot read it) on Metal, passed as void*. Only these functions touch one from the host.
#ifndef SCOPECREEP_SENDER_TEST_GPU_H
#define SCOPECREEP_SENDER_TEST_GPU_H

#include <cstddef>
#include <string>

namespace testgpu {

// The GPU, and the host's stream (CUDA) or command queue (Metal). False and `error` when there is none.
bool init(std::string& error);
// What the host gives the plugin to render on: the CUDA stream, or the Metal command queue.
void* hostHandle();

// The OFX render arguments that say "this call is on the GPU" and carry hostHandle().
extern const char* const kEnabledProp;
extern const char* const kHandleProp;
// The OFX effect property a plugin sets to declare it can render on this GPU API.
extern const char* const kSupportedProp;
// The word the Sender's "not rendering on the GPU" message uses.
extern const char* const kApiName;
// Whether the Sender has a staging buffer that SCOPECREEP_SENDER_GPU_LIMIT_MB can make fail (CUDA only).
extern const bool kHasStagingLimit;

// A buffer of `bytes` bytes in the GPU's memory. Never freed (the test is short).
void* alloc(size_t bytes);
// Fill `rows` rows of `rowBytes` bytes of `host` into the buffer, rows `pitch` bytes apart. Returns when done.
void upload(void* buf, size_t pitch, const void* host, size_t rowBytes, int rows);
// Set `bytes` bytes of the buffer to zero. Returns when done.
void zero(void* buf, size_t bytes);
// The reverse of upload. First waits for everything queued on the host's stream or queue, so a passthrough the
// plugin queued has finished.
void download(void* buf, size_t pitch, void* host, size_t rowBytes, int rows);
// Wait for all GPU work of the host's stream or queue.
void syncHost();
// Queue about 100 ms of GPU work on the host's stream or queue and return at once, as a busy Resolve would.
void keepBusy();

}  // namespace testgpu

#endif
