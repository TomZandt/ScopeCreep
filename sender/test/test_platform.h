// What the fake-host test needs from the operating system: Linux, macOS and Windows. The test itself stays the same
// on all of them; a port adds a branch here.
#ifndef SCOPECREEP_SENDER_TEST_PLATFORM_H
#define SCOPECREEP_SENDER_TEST_PLATFORM_H

#include "ofxCore.h"
#include "scopecreep_feed.h"  // windows.h on Windows

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

#ifndef _WIN32
#include <dlfcn.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

namespace testos {

// stdout a line at a time, so a crash leaves every finished check printed. (MSVC has no line buffering.)
inline void unbufferedLines()
{
#ifdef _WIN32
    std::setvbuf(stdout, nullptr, _IONBF, 0);
#else
    std::setvbuf(stdout, nullptr, _IOLBF, 0);
#endif
}

// A Feed path of this run's own, so the test never touches a real Feed.
inline std::string privateFeedPath()
{
#ifdef _WIN32
    return "Local\\scopecreep-sender-test-" + std::to_string(scfeed_pid());
#elif defined(__APPLE__)
    return "/scf-snd-" + std::to_string(scfeed_pid());  // a shared-memory name holds 31 bytes, the Feed's own `.N` included
#else
    return "/dev/shm/scopecreep-sender-test-" + std::to_string(scfeed_pid());
#endif
}

// Set (value non-null) or unset an environment variable of the process, where the plugin reads it.
inline void setEnv(const char* name, const char* value)
{
#ifdef _WIN32
    SetEnvironmentVariableA(name, value);
#else
    if (value) setenv(name, value, 1);
    else unsetenv(name);
#endif
}

// Remove a Feed left at `path`. Windows: nothing to do, a mapping goes when its last handle closes. macOS: shared
// memory lasts until unlinked, so the locator and every Feed made under it go.
inline void removeFeed(const char* path)
{
#ifdef _WIN32
    (void)path;
#elif defined(__APPLE__)
    int err = 0;
    if (uint64_t* locator = scfeed_os_locator(path, 0, &err)) {
        const uint64_t last = scfeed_load64(&locator[1]);
        munmap(locator, SCFEED_LOCATOR_BYTES);
        for (uint64_t n = 1; n <= last; ++n) {
            char name[64];
            scfeed_os_name(name, sizeof name, path, n);
            shm_unlink(name);
        }
    }
    shm_unlink(path);
#else
    unlink(path);
#endif
}

// The plugin's two OFX entry points, from the built .ofx.
struct Plugin {
    int (*count)() = nullptr;
    OfxPlugin* (*get)(int) = nullptr;
};
inline bool loadPlugin(const char* path, Plugin& p, std::string& error)
{
#ifdef _WIN32
    HMODULE lib = LoadLibraryA(path);
    if (!lib) { error = "LoadLibrary failed with error " + std::to_string(GetLastError()); return false; }
    p.count = reinterpret_cast<int (*)()>(reinterpret_cast<void*>(GetProcAddress(lib, "OfxGetNumberOfPlugins")));
    p.get = reinterpret_cast<OfxPlugin* (*)(int)>(reinterpret_cast<void*>(GetProcAddress(lib, "OfxGetPlugin")));
#else
    void* lib = dlopen(path, RTLD_NOW);
    if (!lib) { error = std::string("dlopen: ") + dlerror(); return false; }
    p.count = (int (*)())dlsym(lib, "OfxGetNumberOfPlugins");
    p.get = (OfxPlugin * (*)(int)) dlsym(lib, "OfxGetPlugin");
#endif
    return true;
}

// Another running process, standing in for a first Resolve that owns the Feed: alive until kill().
// On Windows it is this test run again with kChildArg; main() hands such a run to childMain().
class OtherProcess {
public:
    static constexpr const char* kChildArg = "--stand-in-process";
    static bool isChild(int argc, char** argv) { return argc > 1 && !std::strcmp(argv[1], kChildArg); }
    static int childMain()
    {
#ifdef _WIN32
        Sleep(120000);  // killed long before; bounded so a crashed test leaves nothing running for long
#endif
        return 0;
    }

    bool start()
    {
#ifdef _WIN32
        char exe[MAX_PATH];
        if (!GetModuleFileNameA(nullptr, exe, sizeof exe)) return false;
        std::string cmd = std::string("\"") + exe + "\" " + kChildArg;
        STARTUPINFOA si;
        std::memset(&si, 0, sizeof si);
        si.cb = sizeof si;
        std::memset(&pi_, 0, sizeof pi_);
        if (!CreateProcessA(exe, &cmd[0], nullptr, nullptr, FALSE, 0, nullptr, nullptr, &si, &pi_)) return false;
        pid_ = pi_.dwProcessId;
        return true;
#else
        const pid_t child = fork();
        if (child < 0) return false;
        if (child == 0) { pause(); _exit(0); }
        pid_ = (uint64_t)child;
        return true;
#endif
    }
    uint64_t pid() const { return pid_; }
    // Kill it and wait until it has gone.
    void kill()
    {
#ifdef _WIN32
        TerminateProcess(pi_.hProcess, 1);
        WaitForSingleObject(pi_.hProcess, INFINITE);
        CloseHandle(pi_.hThread);
        CloseHandle(pi_.hProcess);
#else
        ::kill((pid_t)pid_, SIGKILL);
        waitpid((pid_t)pid_, nullptr, 0);
#endif
    }

private:
    uint64_t pid_ = 0;
#ifdef _WIN32
    PROCESS_INFORMATION pi_;
#endif
};

}  // namespace testos

#endif
