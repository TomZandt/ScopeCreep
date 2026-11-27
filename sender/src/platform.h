// What the Sender needs from the operating system beyond standard C++ and the Feed header's scfeed_os_* layer
// (mapping, clock, process id): Linux, macOS and Windows. A port adds a branch here, not at the call sites.
#ifndef SCOPECREEP_SENDER_PLATFORM_H
#define SCOPECREEP_SENDER_PLATFORM_H

#include "scopecreep_feed.h"  // windows.h on Windows

#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <string>

// Which GPU API this build moves frames with, the one Resolve offers on the platform: Metal on a Mac (Resolve there
// gives plugins Metal buffers and no CUDA), CUDA on Linux and Windows. One Sender build, one GPU path.
#ifdef __APPLE__
#define SCOPECREEP_METAL 1
#else
#define SCOPECREEP_CUDA 1
#endif

// Marks the plugin's two OFX entry points as exported from the .ofx (a shared object, or a DLL on Windows).
#ifdef _WIN32
#define SENDER_EXPORT __declspec(dllexport)
#else
#define SENDER_EXPORT __attribute__((visibility("default")))
#endif

namespace scopecreep {

// An environment variable of the process, or "" when unset. On Windows the plugin has its own static C runtime,
// whose copy of the environment is taken when the plugin loads, so the process's own is read instead.
inline std::string envValue(const char* name)
{
#ifdef _WIN32
    // GetEnvironmentVariableA returns the length copied, or, when the buffer is too small, the size needed
    // (with the nul); the value may change between calls, so ask until it fits.
    std::string value(256, '\0');
    for (;;) {
        const DWORD n = GetEnvironmentVariableA(name, &value[0], (DWORD)value.size());
        if (n == 0) return std::string();  // unset (or set to "")
        if (n < value.size()) return value.substr(0, n);
        value.assign(n, '\0');
    }
#else
    const char* v = std::getenv(name);
    return v ? v : "";
#endif
}

// Why the last Feed call failed, in words: errno on Linux, GetLastError on Windows.
inline std::string lastOsError()
{
#ifdef _WIN32
    const DWORD code = GetLastError();
    char buf[256] = {0};
    DWORD n = FormatMessageA(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS, nullptr, code, 0, buf, sizeof buf, nullptr);
    while (n > 0 && (buf[n - 1] == '\r' || buf[n - 1] == '\n' || buf[n - 1] == ' ' || buf[n - 1] == '.')) buf[--n] = 0;
    return n ? std::string(buf, n) : "error " + std::to_string(code);
#else
    return std::strerror(errno);
#endif
}

}  // namespace scopecreep

#endif
