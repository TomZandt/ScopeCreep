// PROTOTYPE (throwaway): Stage 0 check 6 (issue #12). Trivial reader process, runs OUTSIDE Resolve.
// Reads frames from the shared-memory ring written by the probe plugin and verifies they are intact:
//   --verify         recompute the checksum on the CPU and compare with the GPU-computed one in the slot
//                    (plugin must run with SCOPECREEP_PROBE_CHECKSUM=1). Catches torn/partial/wrong copies.
//   --exr-dir DIR    compare slots bit-exactly with frame_inst*_t<time>_WxH.exr dumps (written by the plugin's
//                    dump trigger) whose <time> equals the slot's frame time. This is the "known frame" check:
//                    the dump is Resolve's own frame as the plugin received it.
//   --sleep-ms N     pretend to be a slow consumer (hold each slot N ms) to exercise drop counting
//   --seconds N      run N seconds (default 10; 0 = until killed)
// Build: g++ -O2 -std=c++17 src/probe_reader.cpp -o build/probe_reader -lrt
#include "ring_layout.h"

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <dirent.h>
#include <fcntl.h>
#include <map>
#include <string>
#include <sys/mman.h>
#include <sys/stat.h>
#include <thread>
#include <unistd.h>
#include <vector>

using namespace sc;

static double nowMs()
{
    return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

struct Exr {
    int w = 0, h = 0;
    std::vector<float> planar;  // A,B,G,R planes per scanline: [y][c][x], c: 0=A 1=B 2=G 3=R
};

static bool loadExr(const std::string& path, Exr& e)
{
    FILE* f = std::fopen(path.c_str(), "rb");
    if (!f) return false;
    std::vector<char> all;
    char buf[1 << 16];
    size_t n;
    while ((n = std::fread(buf, 1, sizeof buf, f)) > 0) all.insert(all.end(), buf, buf + n);
    std::fclose(f);
    if (all.size() < 8) return false;
    size_t p = 8;
    int x1 = 0, y1 = 0;
    while (p < all.size() && all[p] != 0) {
        std::string name = &all[p]; p += name.size() + 1;
        std::string type = &all[p]; p += type.size() + 1;
        int sz; std::memcpy(&sz, &all[p], 4); p += 4;
        if (name == "dataWindow") { std::memcpy(&x1, &all[p + 8], 4); std::memcpy(&y1, &all[p + 12], 4); }
        p += (size_t)sz;
    }
    p += 1;  // header terminator
    e.w = x1 + 1; e.h = y1 + 1;
    const size_t table = p, first = table + (size_t)e.h * 8;
    const size_t block = 8 + (size_t)e.w * 16;
    if (all.size() < first + block * (size_t)e.h) return false;
    e.planar.resize((size_t)e.w * 4 * e.h);
    for (int y = 0; y < e.h; ++y)
        std::memcpy(&e.planar[(size_t)y * e.w * 4], &all[first + block * (size_t)y + 8], (size_t)e.w * 16);
    return true;
}

// time encoded in the dump file name: frame_inst<N>_t<time>_<W>x<H>.exr
static std::map<std::string, std::string> scanDumps(const std::string& dir)
{
    std::map<std::string, std::string> out;  // "time_WxH" -> path
    DIR* d = opendir(dir.c_str());
    if (!d) return out;
    while (dirent* de = readdir(d)) {
        const char* n = de->d_name;
        const char* t = std::strstr(n, "_t");
        if (std::strncmp(n, "frame_inst", 10) != 0 || !t) continue;
        std::string key = t + 2;  // "<time>_WxH.exr"
        if (key.size() > 4) key.resize(key.size() - 4);
        out[key] = dir + "/" + n;
    }
    closedir(d);
    return out;
}

int main(int argc, char** argv)
{
    std::string shm = "/scopecreep-probe-ring", exrDir;
    bool verify = false;
    double seconds = 10, sleepMs = 0;
    for (int i = 1; i < argc; ++i) {
        std::string a = argv[i];
        if (a == "--verify") verify = true;
        else if (a == "--exr-dir" && i + 1 < argc) exrDir = argv[++i];
        else if (a == "--sleep-ms" && i + 1 < argc) sleepMs = std::atof(argv[++i]);
        else if (a == "--seconds" && i + 1 < argc) seconds = std::atof(argv[++i]);
        else if (a == "--shm" && i + 1 < argc) shm = argv[++i];
        else { std::fprintf(stderr, "usage: %s [--verify] [--exr-dir DIR] [--sleep-ms N] [--seconds N] [--shm NAME]\n", argv[0]); return 2; }
    }
    int fd = -1;
    for (int i = 0; i < 300 && fd < 0; ++i) { fd = shm_open(shm.c_str(), O_RDWR, 0); if (fd < 0) usleep(100000); }
    if (fd < 0) { std::perror("shm_open (is the plugin running with SCOPECREEP_PROBE_CUDA=1 and has it rendered a frame?)"); return 1; }
    struct stat st;
    fstat(fd, &st);
    void* base = mmap(nullptr, (size_t)st.st_size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (base == MAP_FAILED) { std::perror("mmap"); return 1; }
    RingHeader* hdr = (RingHeader*)base;
    if (hdr->magic != kRingMagic || hdr->version != kRingVersion) { std::fprintf(stderr, "bad ring header\n"); return 1; }
    const uint32_t n = hdr->nslots;
    const uint64_t slotBytes = hdr->slotBytes;
    std::fprintf(stderr, "ring: %u slots x %llu bytes, %zu bytes mapped\n", n, (unsigned long long)slotBytes, (size_t)st.st_size);

    auto dumps = scanDumps(exrDir.empty() ? "" : exrDir);
    std::map<std::string, Exr> exrCache;

    unsigned long long frames = 0, csOk = 0, csBad = 0, csNone = 0, exrOk = 0, exrBad = 0, exrNone = 0, gaps = 0, nonMonotonic = 0, blank = 0;
    double latSum = 0, latMax = 0, readMsSum = 0;
    uint64_t lastSeq = 0;
    bool haveLast = false;
    const double t0 = nowMs();
    double lastPrint = t0;
    unsigned long long printedFrames = 0;
    while (seconds == 0 || nowMs() - t0 < seconds * 1000) {
        int pick = -1;
        uint64_t best = ~0ull;
        for (uint32_t i = 0; i < n; ++i) {
            SlotHeader* sh = slotHeader(base, i, slotBytes);
            if (sh->state.load(std::memory_order_acquire) == kReady && sh->seq < best) { best = sh->seq; pick = (int)i; }
        }
        if (pick < 0) { usleep(500); }
        else {
            SlotHeader* sh = slotHeader(base, (uint32_t)pick, slotBytes);
            uint32_t exp = kReady;
            if (sh->state.compare_exchange_strong(exp, kReading, std::memory_order_acq_rel)) {
                const double r0 = nowMs();
                const uint32_t w = sh->width, h = sh->height;
                const uint32_t* words = (const uint32_t*)slotData(base, (uint32_t)pick, slotBytes);
                const size_t nwords = (size_t)w * h * 4;
                if (haveLast) {
                    if (sh->seq <= lastSeq) nonMonotonic++;
                    else gaps += sh->seq - lastSeq - 1;
                }
                lastSeq = sh->seq; haveLast = true;
                bool allZero = true;
                for (size_t i = 0; i < nwords && i < 4096; ++i) if (words[(nwords / 2 + i) % nwords]) { allZero = false; break; }
                if (allZero) blank++;
                if (verify) {
                    if (!sh->hasChecksum) csNone++;
                    else if (frameChecksum(words, nwords) == sh->checksum) csOk++;
                    else { csBad++; std::fprintf(stderr, "CHECKSUM MISMATCH seq=%llu time=%.3f\n", (unsigned long long)sh->seq, sh->time); }
                }
                if (!exrDir.empty()) {
                    char key[64];
                    std::snprintf(key, sizeof key, "%.3f_%ux%u", sh->time, w, h);
                    auto it = dumps.find(key);
                    if (it == dumps.end()) { dumps = scanDumps(exrDir); it = dumps.find(key); }
                    if (it == dumps.end()) exrNone++;
                    else {
                        Exr& e = exrCache[key];
                        if (e.planar.empty()) loadExr(it->second, e);
                        bool same = e.w == (int)w && e.h == (int)h && !e.planar.empty();
                        for (uint32_t y = 0; y < h && same; ++y) {
                            const float* srow = (const float*)words + (size_t)(h - 1 - y) * w * 4;  // EXR is top-down, OFX bottom-up
                            const float* er = &e.planar[(size_t)y * w * 4];
                            for (uint32_t x = 0; x < w; ++x) {
                                const float px[4] = {er[3 * w + x], er[2 * w + x], er[1 * w + x], er[0 * w + x]};  // R,G,B,A
                                if (std::memcmp(px, srow + (size_t)x * 4, 16) != 0) { same = false; break; }
                            }
                        }
                        if (same) exrOk++; else { exrBad++; std::fprintf(stderr, "EXR MISMATCH seq=%llu time=%.3f vs %s\n", (unsigned long long)sh->seq, sh->time, it->second.c_str()); }
                    }
                }
                const double lat = sh->completeWallMs - sh->submitWallMs;
                latSum += lat; if (lat > latMax) latMax = lat;
                frames++;
                if (sleepMs > 0) std::this_thread::sleep_for(std::chrono::duration<double, std::milli>(sleepMs));
                readMsSum += nowMs() - r0;
                sh->state.store(kFree, std::memory_order_release);
            }
        }
        const double now = nowMs();
        if (now - lastPrint >= 2000) {
            std::fprintf(stderr, "[reader +%.0fs] frames=%llu (+%.1f/s) gaps=%llu ring: published=%llu dropOverwrite=%llu dropNoSlot=%llu dropNoStaging=%llu\n",
                         (now - t0) / 1000, frames, (frames - printedFrames) / ((now - lastPrint) / 1000), gaps,
                         (unsigned long long)hdr->published.load(), (unsigned long long)hdr->dropOverwrite.load(),
                         (unsigned long long)hdr->dropNoSlot.load(), (unsigned long long)hdr->dropNoStaging.load());
            printedFrames = frames; lastPrint = now;
        }
    }
    std::printf("READER SUMMARY frames=%llu seq_gaps=%llu non_monotonic=%llu blank_frames=%llu\n", frames, gaps, nonMonotonic, blank);
    if (verify) std::printf("  checksum: ok=%llu BAD=%llu not_present=%llu\n", csOk, csBad, csNone);
    if (!exrDir.empty()) std::printf("  exr compare (bit-exact vs Resolve frame dump): ok=%llu BAD=%llu no_dump_for_frame=%llu\n", exrOk, exrBad, exrNone);
    std::printf("  copy wall latency (submit->completion callback) mean=%.2f ms max=%.2f ms; reader hold mean=%.2f ms\n",
                frames ? latSum / frames : 0, latMax, frames ? readMsSum / frames : 0);
    std::printf("  ring counters: published=%llu dropOverwrite=%llu dropNoSlot=%llu dropNoStaging=%llu dropOversize=%llu skippedThumb=%llu\n",
                (unsigned long long)hdr->published.load(), (unsigned long long)hdr->dropOverwrite.load(), (unsigned long long)hdr->dropNoSlot.load(),
                (unsigned long long)hdr->dropNoStaging.load(), (unsigned long long)hdr->dropOversize.load(), (unsigned long long)hdr->skippedThumb.load());
    return (csBad || exrBad || nonMonotonic) ? 3 : 0;
}
