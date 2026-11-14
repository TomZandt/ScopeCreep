// PROTOTYPE (throwaway): Stage 0 probe Sender, issue #4.
//
// Smallest possible OFX plugin: passes the picture through untouched and logs
// what Resolve tells it (host identity, page, thumbnail flag, OFX colour
// management, image format, render timing). On request (trigger file) it saves
// the next received source frame as a float EXR, exactly as received.
//
// Raw C OFX API, no support library: we want every property read and every
// action visible. Default: CPU render path. With SCOPECREEP_PROBE_CUDA=1 the plugin
// opts into Resolve's CUDA render path (images arrive as device pointers) and, on every
// non-thumbnail render, starts an ASYNCHRONOUS copy of the whole frame into a shared-memory
// ring (issue #12, see cuda_copy.cu / ring_layout.h) and returns without waiting.
//
// Env: SCOPECREEP_PROBE_DIR  (default /tmp/scopecreep-probe) log + dumps + trigger
//      SCOPECREEP_PROBE_STYLE (ocio|full|core|basic|none; default ocio) colour
//                              management style declared to the host
//      SCOPECREEP_PROBE_CUDA=1  declare CUDA render + CUDA stream support (default: off, CPU path)
//      SCOPECREEP_PROBE_COPY=0  start with the frame copy switched off (default on, CUDA path only)
//      <DIR>/copy.off           while this file exists the copy is off (switch at run time; passthrough continues)
//      SCOPECREEP_PROBE_COPY_MODE=staged|direct   staged (default): D2D to private staging on Resolve's stream,
//                              D2H on our own stream; direct: D2H straight on Resolve's stream
//      SCOPECREEP_PROBE_SLOTS (default 4) host shm slots; SCOPECREEP_PROBE_STAGING (default 2) device staging buffers
//      SCOPECREEP_PROBE_SLOT_BYTES (default 3840*2160*16); SCOPECREEP_PROBE_SHM (default /scopecreep-probe-ring)
//      SCOPECREEP_PROBE_CHECKSUM=1  GPU checksum of every copied frame (verification runs; costs GPU time)
//      SCOPECREEP_PROBE_QUIET=1  no per-render log lines, only a COPYSTATS line each second
//      SCOPECREEP_PROBE_TRACE=1  buffer one CSV line per render in memory, appended to <DIR>/render_trace.csv once a second
#include "ofxCore.h"
#include "ofxImageEffect.h"
#include "ofxImageEffectExt.h"
#include "ofxGPURender.h"
#include "ofxColour.h"
#include "ofxProperty.h"

#include "exr_dump.h"
#include "cuda_copy.h"

#include <chrono>
#include <cstdarg>
#include <cstdio>
#include <cstdlib>
#include <atomic>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

#define PLUGIN_ID "com.scopecreep.probesender"

static OfxHost* gHost = nullptr;
static OfxPropertySuiteV1* gProp = nullptr;
static OfxImageEffectSuiteV1* gEffect = nullptr;
static std::mutex gLogMutex;
static std::chrono::steady_clock::time_point gLastRender;
static bool gHaveLastRender = false;
static int gNextInstance = 1;

// ---------------------------------------------------------------- logging

static std::string probeDir()
{
    const char* d = std::getenv("SCOPECREEP_PROBE_DIR");
    return d && *d ? d : "/tmp/scopecreep-probe";
}

static void logf(const char* fmt, ...)
{
    std::lock_guard<std::mutex> lock(gLogMutex);
    const std::string dir = probeDir();
    mkdir(dir.c_str(), 0755);
    FILE* f = std::fopen((dir + "/probe.log").c_str(), "a");
    if (!f) return;
    auto now = std::chrono::system_clock::now().time_since_epoch();
    long long ms = std::chrono::duration_cast<std::chrono::milliseconds>(now).count();
    std::fprintf(f, "[%lld.%03lld pid=%d tid=%ld] ", ms / 1000, ms % 1000, (int)getpid(), (long)syscall(SYS_gettid));
    va_list ap;
    va_start(ap, fmt);
    std::vfprintf(f, fmt, ap);
    va_end(ap);
    std::fputc('\n', f);
    std::fclose(f);
}

// ------------------------------------------------------- property helpers

static std::string str(OfxPropertySetHandle p, const char* name, int idx = 0)
{
    if (!p) return "<null set>";
    char* v = nullptr;
    OfxStatus s = gProp->propGetString(p, name, idx, &v);
    if (s != kOfxStatOK) return "<unset:" + std::to_string(s) + ">";
    return v ? v : "<null>";
}

// All values of a string property, joined with '|'.
static std::string strAll(OfxPropertySetHandle p, const char* name)
{
    if (!p) return "<null set>";
    int n = 0;
    if (gProp->propGetDimension(p, name, &n) != kOfxStatOK) return "<unset>";
    std::string out;
    for (int i = 0; i < n; ++i) out += (i ? "|" : "") + str(p, name, i);
    return out.empty() ? "<empty>" : out;
}

static std::string ints(OfxPropertySetHandle p, const char* name, int count)
{
    if (!p) return "<null set>";
    std::string out;
    for (int i = 0; i < count; ++i) {
        int v = 0;
        OfxStatus s = gProp->propGetInt(p, name, i, &v);
        if (s != kOfxStatOK) return i == 0 ? "<unset:" + std::to_string(s) + ">" : out;
        out += (i ? "," : "") + std::to_string(v);
    }
    return out;
}

static std::string dbl(OfxPropertySetHandle p, const char* name)
{
    if (!p) return "<null set>";
    double v = 0;
    OfxStatus s = gProp->propGetDouble(p, name, 0, &v);
    if (s != kOfxStatOK) return "<unset:" + std::to_string(s) + ">";
    char b[64];
    std::snprintf(b, sizeof b, "%.6f", v);
    return b;
}

static bool envFlag(const char* name, bool def)
{
    const char* v = std::getenv(name);
    if (!v || !*v) return def;
    return std::strcmp(v, "0") != 0;
}
static long envLong(const char* name, long def)
{
    const char* v = std::getenv(name);
    return v && *v ? std::atol(v) : def;
}
static bool useCuda() { return envFlag("SCOPECREEP_PROBE_CUDA", false); }
static bool quiet() { static const bool q = envFlag("SCOPECREEP_PROBE_QUIET", false); return q; }

static double steadyMs()
{
    return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

// Per-render time inside onRender, all instances, and the periodic COPYSTATS line.
static std::mutex gStatMutex;
static double gRenderMsSum = 0, gRenderMsMax = 0;
static unsigned long long gRenderCount = 0, gCudaRenders = 0, gCopyOffRenders = 0;
static double gLastStatsMs = 0;
static sc::CopyStats gPrevStats;
static unsigned long long gPrevRenderCount = 0;
static double gPrevRenderMsSum = 0;

static std::string gTrace;
static bool traceOn() { static const bool t = envFlag("SCOPECREEP_PROBE_TRACE", false); return t; }

static void noteRender(double ms, bool cuda, bool copyWasOn, int inst, double frameTime, int thumb, int copyResult, int tid)
{
    std::lock_guard<std::mutex> l(gStatMutex);
    if (traceOn()) {
        char b[160];
        std::snprintf(b, sizeof b, "%.3f,%d,%d,%.3f,%.4f,%d,%d,%d\n", steadyMs(), inst, tid, frameTime, ms, thumb, (int)copyWasOn, copyResult);
        gTrace += b;
    }
    gRenderCount++;
    gRenderMsSum += ms;
    if (ms > gRenderMsMax) gRenderMsMax = ms;
    if (cuda) { gCudaRenders++; if (!copyWasOn) gCopyOffRenders++; }
    const double now = steadyMs();
    if (gLastStatsMs == 0) { gLastStatsMs = now; return; }
    if (now - gLastStatsMs < 1000) return;
    const double dtS = (now - gLastStatsMs) / 1000.0;
    gLastStatsMs = now;
    if (traceOn() && !gTrace.empty()) {
        const std::string path = probeDir() + "/render_trace.csv";
        bool fresh = access(path.c_str(), F_OK) != 0;
        if (FILE* f = std::fopen(path.c_str(), "a")) {
            if (fresh) std::fputs("steady_ms,inst,tid,frame_time,render_ms,thumbnail,copy_on,copy_result\n", f);
            std::fwrite(gTrace.data(), 1, gTrace.size(), f);
            std::fclose(f);
        }
        gTrace.clear();
    }
    sc::CopyStats cs = sc::cudaCopyReady() ? sc::cudaCopyStats() : sc::CopyStats();
    const unsigned long long dn = gRenderCount - gPrevRenderCount;
    const unsigned long long dstart = cs.started - gPrevStats.started, dcomp = cs.completed - gPrevStats.completed,
                             dharv = cs.harvested - gPrevStats.harvested;
    logf("COPYSTATS renders=%llu renders_per_s=%.1f render_ms_mean=%.3f render_ms_max=%.3f cuda_renders=%llu "
         "copy_off_renders=%llu started=%llu completed=%llu started_per_s=%.1f gpu_copy_ms_mean=%.3f gpu_copy_ms_max=%.3f "
         "wall_latency_ms_mean=%.3f wall_latency_ms_max=%.3f submit_ms_mean=%.3f submit_ms_max=%.3f "
         "drop_overwrite=%llu drop_noslot=%llu drop_nostaging=%llu drop_oversize=%llu skipped_thumb=%llu last_err=%s",
         dn, dn / dtS, dn ? (gRenderMsSum - gPrevRenderMsSum) / dn : 0.0, gRenderMsMax, gCudaRenders, gCopyOffRenders,
         cs.started, cs.completed, dstart / dtS,
         dharv ? (cs.gpuMsSum - gPrevStats.gpuMsSum) / dharv : 0.0, cs.gpuMsMax,
         dcomp ? (cs.latMsSum - gPrevStats.latMsSum) / dcomp : 0.0, cs.latMsMax,
         dstart ? (cs.submitMsSum - gPrevStats.submitMsSum) / dstart : 0.0, cs.submitMsMax,
         cs.dropOverwrite, cs.dropNoSlot, cs.dropNoStaging, cs.dropOversize, cs.skippedThumb,
         sc::cudaCopyLastError().c_str());
    gPrevStats = cs;
    if (sc::cudaCopyReady()) sc::cudaCopyResetIntervalMax();
    gPrevRenderCount = gRenderCount;
    gPrevRenderMsSum = gRenderMsSum;
    gRenderMsMax = 0;
}

static std::mutex gCopyInitMutex;
static bool gCopyInitTried = false;
static bool gCopyInitOk = false;
static bool ensureCopyInit()
{
    std::lock_guard<std::mutex> l(gCopyInitMutex);
    if (gCopyInitTried) return gCopyInitOk;
    gCopyInitTried = true;
    sc::CopyConfig cfg;
    cfg.nslots = (int)envLong("SCOPECREEP_PROBE_SLOTS", 4);
    cfg.nstaging = (int)envLong("SCOPECREEP_PROBE_STAGING", 2);
    cfg.slotBytes = (size_t)envLong("SCOPECREEP_PROBE_SLOT_BYTES", 3840L * 2160 * 16);
    const char* m = std::getenv("SCOPECREEP_PROBE_COPY_MODE");
    cfg.staged = !(m && std::strcmp(m, "direct") == 0);
    cfg.checksum = envFlag("SCOPECREEP_PROBE_CHECKSUM", false);
    if (const char* n = std::getenv("SCOPECREEP_PROBE_SHM")) cfg.shmName = n;
    const double t0 = steadyMs();
    std::string err = sc::cudaCopyInit(cfg);
    logf("copy init: %s in %.1f ms (slots=%d slot_bytes=%zu staging=%d mode=%s checksum=%d shm=%s)",
         err.empty() ? "OK" : ("FAILED " + err).c_str(), steadyMs() - t0, cfg.nslots, cfg.slotBytes, cfg.nstaging,
         cfg.staged ? "staged" : "direct", (int)cfg.checksum, cfg.shmName.c_str());
    gCopyInitOk = err.empty();
    return gCopyInitOk;
}

static bool copySwitchedOn()
{
    static const bool envOn = envFlag("SCOPECREEP_PROBE_COPY", true);
    if (!envOn) return false;
    const std::string off = probeDir() + "/copy.off";
    return access(off.c_str(), F_OK) != 0;
}

// ------------------------------------------------------------ instance data

struct Instance {
    int id;
    OfxImageEffectHandle effect;
    OfxPropertySetHandle effectProps;
    std::string page;
    long renders = 0;
};

static Instance* getInstance(OfxImageEffectHandle h)
{
    OfxPropertySetHandle props = nullptr;
    gEffect->getPropertySet(h, &props);
    void* data = nullptr;
    gProp->propGetPointer(props, kOfxPropInstanceData, 0, &data);
    return (Instance*)data;
}

static void logColour(const char* where, Instance* inst, OfxImageEffectHandle effect)
{
    OfxPropertySetHandle ep = nullptr;
    gEffect->getPropertySet(effect, &ep);
    logf("inst=%d %s colour-mgmt: style=%s config=%s ocio_config=%s display_cs=%s ocio_display=%s ocio_view=%s",
         inst ? inst->id : -1, where,
         str(ep, kOfxImageEffectPropColourManagementStyle).c_str(),
         str(ep, kOfxImageEffectPropColourManagementConfig).c_str(),
         str(ep, kOfxImageEffectPropOCIOConfig).c_str(),
         str(ep, kOfxImageEffectPropDisplayColourspace).c_str(),
         str(ep, kOfxImageEffectPropOCIODisplay).c_str(),
         str(ep, kOfxImageEffectPropOCIOView).c_str());
    for (const char* cn : {kOfxImageEffectSimpleSourceClipName, kOfxImageEffectOutputClipName}) {
        OfxImageClipHandle clip = nullptr;
        OfxPropertySetHandle cp = nullptr;
        if (gEffect->clipGetHandle(effect, cn, &clip, &cp) != kOfxStatOK) continue;
        logf("inst=%d %s clip=%s colourspace=%s preferred=%s thumbnail=%s components=%s depth=%s pixel_aspect=%s",
             inst ? inst->id : -1, where, cn,
             str(cp, kOfxImageClipPropColourspace).c_str(),
             strAll(cp, kOfxImageClipPropPreferredColourspaces).c_str(),
             ints(cp, kOfxImageClipPropThumbnail, 1).c_str(),
             str(cp, kOfxImageEffectPropComponents).c_str(),
             str(cp, kOfxImageEffectPropPixelDepth).c_str(),
             dbl(cp, kOfxImagePropPixelAspectRatio).c_str());
    }
}

// ------------------------------------------------------------------ actions

static OfxStatus onLoad()
{
    OfxPropertySetHandle hp = gHost->host;
    logf("=== LOAD host: name=%s label=%s version=%s version_label=%s api_version=%s",
         str(hp, kOfxPropName).c_str(), str(hp, kOfxPropLabel).c_str(),
         ints(hp, kOfxPropVersion, 4).c_str(), str(hp, kOfxPropVersionLabel).c_str(),
         ints(hp, kOfxPropAPIVersion, 2).c_str());
    logf("host colour-mgmt: style=%s available_configs=%s",
         str(hp, kOfxImageEffectPropColourManagementStyle).c_str(),
         strAll(hp, kOfxImageEffectPropColourManagementAvailableConfigs).c_str());
    logf("host misc: is_background=%s supports_tiles=%s cuda_render=%s",
         ints(hp, kOfxImageEffectHostPropIsBackground, 1).c_str(),
         ints(hp, kOfxImageEffectPropSupportsTiles, 1).c_str(),
         str(hp, kOfxImageEffectPropCudaRenderSupported).c_str());
    return kOfxStatOK;
}

static const char* declaredStyle()
{
    const char* s = std::getenv("SCOPECREEP_PROBE_STYLE");
    std::string v = s ? s : "ocio";
    if (v == "full") return kOfxImageEffectColourManagementFull;
    if (v == "core") return kOfxImageEffectColourManagementCore;
    if (v == "basic") return kOfxImageEffectColourManagementBasic;
    if (v == "none") return kOfxImageEffectColourManagementNone;
    return kOfxImageEffectColourManagementOCIO;
}

static OfxStatus onDescribe(OfxImageEffectHandle effect)
{
    OfxPropertySetHandle p = nullptr;
    gEffect->getPropertySet(effect, &p);
    gProp->propSetString(p, kOfxPropLabel, 0, "scopecreep Probe Sender");
    gProp->propSetString(p, kOfxImageEffectPluginPropGrouping, 0, "scopecreep");
    gProp->propSetString(p, kOfxPropPluginDescription, 0,
                         "Throwaway Stage 0 probe: passes the picture through and logs what the host reports.");
    gProp->propSetString(p, kOfxImageEffectPropSupportedContexts, 0, kOfxImageEffectContextFilter);
    gProp->propSetString(p, kOfxImageEffectPropSupportedContexts, 1, kOfxImageEffectContextGeneral);
    gProp->propSetString(p, kOfxImageEffectPropSupportedPixelDepths, 0, kOfxBitDepthFloat);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsMultipleClipDepths, 0, 0);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsTiles, 0, 0);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsMultiResolution, 0, 0);
    gProp->propSetString(p, kOfxImageEffectPluginRenderThreadSafety, 0, kOfxImageEffectRenderFullySafe);
    if (useCuda()) {
        gProp->propSetString(p, kOfxImageEffectPropCudaRenderSupported, 0, "true");
        gProp->propSetString(p, kOfxImageEffectPropCudaStreamSupported, 0, "true");
        logf("describe: declared CUDA render + CUDA stream support");
    }
    // Colour management: declare the richest style, let the host negotiate down.
    gProp->propSetString(p, kOfxImageEffectPropColourManagementStyle, 0, declaredStyle());
    gProp->propSetString(p, kOfxImageEffectPropColourManagementAvailableConfigs, 0,
                         "ofx-native-v1.5_aces-v1.3_ocio-v2.3");
    logf("describe: declared colour-mgmt style=%s", declaredStyle());
    return kOfxStatOK;
}

static OfxStatus onDescribeInContext(OfxImageEffectHandle effect, OfxPropertySetHandle inArgs)
{
    logf("describeInContext: context=%s", str(inArgs, kOfxImageEffectPropContext).c_str());
    for (const char* name : {kOfxImageEffectSimpleSourceClipName, kOfxImageEffectOutputClipName}) {
        OfxPropertySetHandle cp = nullptr;
        gEffect->clipDefine(effect, name, &cp);
        gProp->propSetString(cp, kOfxImageEffectPropSupportedComponents, 0, kOfxImageComponentRGBA);
        gProp->propSetInt(cp, kOfxImageEffectPropSupportsTiles, 0, 0);
    }
    return kOfxStatOK;
}

static OfxStatus onCreateInstance(OfxImageEffectHandle effect, OfxPropertySetHandle inArgs)
{
    Instance* inst = new Instance();
    inst->id = gNextInstance++;
    inst->effect = effect;
    gEffect->getPropertySet(effect, &inst->effectProps);
    inst->page = str(inArgs, kOfxImageEffectPropResolvePage);
    gProp->propSetPointer(inst->effectProps, kOfxPropInstanceData, 0, inst);

    logf("=== CREATE inst=%d page=%s context=%s is_interactive=%s frame_rate=%s src_file=%s",
         inst->id, inst->page.c_str(),
         str(inst->effectProps, kOfxImageEffectPropContext).c_str(),
         ints(inst->effectProps, kOfxPropIsInteractive, 1).c_str(),
         dbl(inst->effectProps, kOfxImageEffectPropFrameRate).c_str(),
         str(inst->effectProps, kOfxImageEffectPropSrcFilePath).c_str());
    logColour("create", inst, effect);
    return kOfxStatOK;
}

static OfxStatus onDestroyInstance(OfxImageEffectHandle effect)
{
    Instance* inst = getInstance(effect);
    if (inst) {
        logf("=== DESTROY inst=%d renders=%ld", inst->id, inst->renders);
        delete inst;
    }
    return kOfxStatOK;
}

static OfxStatus onGetClipPreferences(OfxImageEffectHandle effect, OfxPropertySetHandle outArgs)
{
    (void)outArgs;
    Instance* inst = getInstance(effect);
    // Express no preference: we accept whatever colourspace the host has.
    logColour("clip-prefs", inst, effect);
    return kOfxStatReplyDefault;
}

static OfxStatus onGetOutputColourspace(OfxImageEffectHandle effect, OfxPropertySetHandle inArgs,
                                        OfxPropertySetHandle outArgs)
{
    Instance* inst = getInstance(effect);
    logf("inst=%d getOutputColourspace: host preferred=%s", inst ? inst->id : -1,
         strAll(inArgs, kOfxImageClipPropPreferredColourspaces).c_str());
    // Pass-through: output colourspace equals the source clip's.
    gProp->propSetString(outArgs, kOfxImageClipPropColourspace, 0, "OfxColourspace_Source");
    return kOfxStatOK;
}

struct ImageInfo {
    OfxPropertySetHandle props = nullptr;
    void* data = nullptr;
    int bounds[4] = {0, 0, 0, 0};
    int rowBytes = 0;
    std::string depth, components;
};

static bool readImage(OfxPropertySetHandle p, ImageInfo& im)
{
    im.props = p;
    gProp->propGetPointer(p, kOfxImagePropData, 0, &im.data);
    for (int i = 0; i < 4; ++i) gProp->propGetInt(p, kOfxImagePropBounds, i, &im.bounds[i]);
    gProp->propGetInt(p, kOfxImagePropRowBytes, 0, &im.rowBytes);
    im.depth = str(p, kOfxImageEffectPropPixelDepth);
    im.components = str(p, kOfxImageEffectPropComponents);
    return im.data != nullptr;
}

static const char* rowPtr(const ImageInfo& im, int y)
{
    return (const char*)im.data + (long long)(y - im.bounds[1]) * im.rowBytes;
}

static void maybeDumpFrame(Instance* inst, const ImageInfo& src, double frame, bool cuda, void* stream)
{
    const std::string dir = probeDir();
    const std::string trigger = dir + "/dump.trigger";
    if (access(trigger.c_str(), F_OK) != 0) return;
    if (src.depth != kOfxBitDepthFloat || src.components != kOfxImageComponentRGBA) {
        logf("dump: skipped, source is %s %s, not float RGBA", src.depth.c_str(), src.components.c_str());
        return;
    }
    if (unlink(trigger.c_str()) != 0) return;  // another render thread took it
    logf("dump: trigger taken, src data=%p bounds=%d,%d,%d,%d rowbytes=%d depth=%s", src.data, src.bounds[0], src.bounds[1], src.bounds[2], src.bounds[3], (int)src.rowBytes, src.depth.c_str());
    const int w = src.bounds[2] - src.bounds[0], h = src.bounds[3] - src.bounds[1];
    std::vector<float> contiguous((size_t)w * h * 4);
    if (cuda) {
        // diagnostic only: this one DOES block (dump trigger); the normal copy path never does
        if (!sc::cudaReadbackBlocking(stream, src.data, (size_t)src.rowBytes, (size_t)w * 16, (size_t)h, contiguous.data())) {
            logf("dump: CUDA readback failed: %s", sc::cudaCopyLastError().c_str());
            return;
        }
    } else {
        for (int y = 0; y < h; ++y)
            std::memcpy(&contiguous[(size_t)y * w * 4], rowPtr(src, src.bounds[1] + y), (size_t)w * 16);
    }
    char name[256];
    std::snprintf(name, sizeof name, "/frame_inst%d_t%.3f_%dx%d.exr", inst->id, frame, w, h);
    const std::string path = dir + name;
    logf("dump: copied rows, writing %s", path.c_str());
    std::string err = writeExrFloatRgba(path, contiguous.data(), w, h, w * 4);
    logf("dump: %s -> %s", err.empty() ? "wrote" : ("FAILED " + err).c_str(), path.c_str());
}

static OfxStatus onRender(OfxImageEffectHandle effect, OfxPropertySetHandle inArgs)
{
    auto t0 = std::chrono::steady_clock::now();
    Instance* inst = getInstance(effect);
    if (!inst) return kOfxStatFailed;
    inst->renders++;

    double time = 0;
    gProp->propGetDouble(inArgs, kOfxPropTime, 0, &time);
    int rw[4] = {0, 0, 0, 0};
    for (int i = 0; i < 4; ++i) gProp->propGetInt(inArgs, kOfxImageEffectPropRenderWindow, i, &rw[i]);

    OfxImageClipHandle srcClip = nullptr, dstClip = nullptr;
    OfxPropertySetHandle srcClipProps = nullptr, dstClipProps = nullptr;
    gEffect->clipGetHandle(effect, kOfxImageEffectSimpleSourceClipName, &srcClip, &srcClipProps);
    gEffect->clipGetHandle(effect, kOfxImageEffectOutputClipName, &dstClip, &dstClipProps);

    OfxPropertySetHandle srcP = nullptr, dstP = nullptr;
    ImageInfo src, dst;
    bool haveSrc = gEffect->clipGetImage(srcClip, time, nullptr, &srcP) == kOfxStatOK && readImage(srcP, src);
    bool haveDst = gEffect->clipGetImage(dstClip, time, nullptr, &dstP) == kOfxStatOK && readImage(dstP, dst);

    // Gap since the previous render of any instance: a rough playback-rate hint.
    double gapMs = -1;
    {
        std::lock_guard<std::mutex> lock(gLogMutex);
        if (gHaveLastRender)
            gapMs = std::chrono::duration<double, std::milli>(t0 - gLastRender).count();
        gLastRender = t0;
        gHaveLastRender = true;
    }

    const bool cuda = ints(inArgs, kOfxImageEffectPropCudaEnabled, 1) == "1";
    void* cudaStream = nullptr;
    if (cuda) gProp->propGetPointer(inArgs, kOfxImageEffectPropCudaStream, 0, &cudaStream);
    bool copyOn = false;
    int copyResult = -1;  // -1 not attempted, 0 skipped/dropped, 1 started
    const bool floatRgba = haveSrc && haveDst && src.depth == kOfxBitDepthFloat && dst.depth == kOfxBitDepthFloat &&
                           src.components == kOfxImageComponentRGBA && dst.components == kOfxImageComponentRGBA;
    if (cuda) {
        if (floatRgba) {
            const int w = rw[2] - rw[0], h = rw[3] - rw[1];
            // device pointers: addresses of the render window's first pixel (rows are bottom-up, bounds[1] = lowest row)
            const char* sp = (const char*)src.data + (long long)(rw[1] - src.bounds[1]) * src.rowBytes + (long long)(rw[0] - src.bounds[0]) * 16;
            char* dp = (char*)dst.data + (long long)(rw[1] - dst.bounds[1]) * dst.rowBytes + (long long)(rw[0] - dst.bounds[0]) * 16;
            if (!sc::cudaCopyReady()) ensureCopyInit();  // first CUDA render pins the ring (one-off, hundreds of ms)
            sc::cudaPassthrough(cudaStream, dp, (size_t)dst.rowBytes, sp, (size_t)src.rowBytes, (size_t)w * 16, (size_t)h);
            copyOn = copySwitchedOn() && sc::cudaCopyReady();
            if (copyOn) {
                if (ints(srcClipProps, kOfxImageClipPropThumbnail, 1) == "1") { sc::cudaCopyCountThumb(); copyResult = 0; }
                else copyResult = sc::cudaCopyFrame(cudaStream, sp, (size_t)src.rowBytes, w, h, time, inst->id) ? 1 : 0;
            }
        }
    } else if (floatRgba) {
        const int x1 = rw[0], x2 = rw[2];
        for (int y = rw[1]; y < rw[3]; ++y) {
            if (y < src.bounds[1] || y >= src.bounds[3] || y < dst.bounds[1] || y >= dst.bounds[3]) continue;
            const char* s = rowPtr(src, y) + (long long)(x1 - src.bounds[0]) * 16;
            char* d = (char*)rowPtr(dst, y) + (long long)(x1 - dst.bounds[0]) * 16;
            std::memcpy(d, s, (size_t)(x2 - x1) * 16);
        }
    }
    if (haveSrc) maybeDumpFrame(inst, src, time, cuda, cudaStream);

    double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();
    noteRender(ms, cuda, copyOn, inst->id, time, ints(srcClipProps, kOfxImageClipPropThumbnail, 1) == "1" ? 1 : 0, copyResult, (int)syscall(SYS_gettid));
    if (!quiet()) logf("RENDER inst=%d page=%s time=%.3f src_frame=%s render_window=%d,%d,%d,%d render_scale=%s,%s "
         "field=%s sequential=%s interactive=%s draft=%s thumbnail=%s",
         inst->id, inst->page.c_str(), time, ints(inArgs, kOfxImageEffectPropSrcFrame, 1).c_str(),
         rw[0], rw[1], rw[2], rw[3],
         dbl(inArgs, kOfxImageEffectPropRenderScale).c_str(), "-",
         str(inArgs, kOfxImageEffectPropFieldToRender).c_str(),
         str(inArgs, kOfxImageEffectPropSequentialRenderStatus).c_str(),
         str(inArgs, kOfxImageEffectPropInteractiveRenderStatus).c_str(),
         str(inArgs, kOfxImageEffectPropRenderQualityDraft).c_str(),
         ints(srcClipProps, kOfxImageClipPropThumbnail, 1).c_str());
    if (!quiet()) logf("RENDER inst=%d   extra: scale_xy=%s|%s opencl=%s cuda=%s metal=%s opengl=%s abort=%s interactive_effect=%s "
         "frame_range=%s|%s inst_changed_since_create=%s",
         inst->id, dbl(inArgs, kOfxImageEffectPropRenderScale).c_str(), ints(inArgs, kOfxImageEffectPropRenderScale, 2).c_str(),
         ints(inArgs, kOfxImageEffectPropOpenCLEnabled, 1).c_str(), ints(inArgs, kOfxImageEffectPropCudaEnabled, 1).c_str(),
         ints(inArgs, kOfxImageEffectPropMetalEnabled, 1).c_str(), ints(inArgs, kOfxImageEffectPropOpenGLEnabled, 1).c_str(),
         ints(inArgs, kOfxImageEffectPropRenderQualityDraft, 1).c_str(),
         ints(inst->effectProps, kOfxPropIsInteractive, 1).c_str(),
         dbl(inst->effectProps, kOfxImageEffectPropFrameRange).c_str(), "-", "-");
    if (!quiet()) logf("RENDER inst=%d   src: depth=%s components=%s bounds=%d,%d,%d,%d rowbytes=%d  dst: depth=%s components=%s "
         "bounds=%d,%d,%d,%d  render_page_arg=%s  copy_ms=%.3f gap_since_last_render_ms=%.3f cuda=%d copy_result=%d",
         inst->id, src.depth.c_str(), src.components.c_str(), src.bounds[0], src.bounds[1], src.bounds[2],
         src.bounds[3], src.rowBytes, dst.depth.c_str(), dst.components.c_str(), dst.bounds[0], dst.bounds[1],
         dst.bounds[2], dst.bounds[3], str(inArgs, kOfxImageEffectPropResolvePage).c_str(), ms, gapMs, (int)cuda, copyResult);
    // Colour management as seen at render time (first renders of an instance only, to keep the log readable).
    if (!quiet() && inst->renders <= 3) logColour("render", inst, effect);

    if (srcP) gEffect->clipReleaseImage(srcP);
    if (dstP) gEffect->clipReleaseImage(dstP);
    return (haveSrc && haveDst) ? kOfxStatOK : kOfxStatFailed;
}

// -------------------------------------------------------------------- entry

static OfxStatus mainEntry(const char* action, const void* handle, OfxPropertySetHandle inArgs,
                           OfxPropertySetHandle outArgs)
{
    OfxImageEffectHandle effect = (OfxImageEffectHandle)handle;
    if (std::strcmp(action, kOfxActionLoad) == 0) return onLoad();
    if (std::strcmp(action, kOfxActionDescribe) == 0) return onDescribe(effect);
    if (std::strcmp(action, kOfxImageEffectActionDescribeInContext) == 0) return onDescribeInContext(effect, inArgs);
    if (std::strcmp(action, kOfxActionCreateInstance) == 0) return onCreateInstance(effect, inArgs);
    if (std::strcmp(action, kOfxActionDestroyInstance) == 0) return onDestroyInstance(effect);
    if (std::strcmp(action, kOfxImageEffectActionRender) == 0) return onRender(effect, inArgs);
    if (std::strcmp(action, kOfxImageEffectActionGetClipPreferences) == 0) return onGetClipPreferences(effect, outArgs);
    if (std::strcmp(action, kOfxImageEffectActionGetOutputColourspace) == 0)
        return onGetOutputColourspace(effect, inArgs, outArgs);
    if (std::strcmp(action, kOfxImageEffectActionBeginSequenceRender) == 0 ||
        std::strcmp(action, kOfxImageEffectActionEndSequenceRender) == 0) {
        Instance* i = getInstance(effect);
        logf("inst=%d ACTION %s range=%s|%s step=%s interactive=%s sequential=%s page=%s", i ? i->id : -1, action,
             dbl(inArgs, kOfxImageEffectPropFrameRange).c_str(), ints(inArgs, kOfxImageEffectPropFrameRange, 2).c_str(),
             dbl(inArgs, kOfxImageEffectPropFrameStep).c_str(), str(inArgs, kOfxImageEffectPropInteractiveRenderStatus).c_str(),
             str(inArgs, kOfxImageEffectPropSequentialRenderStatus).c_str(), str(inArgs, kOfxImageEffectPropResolvePage).c_str());
        return kOfxStatReplyDefault;
    }
    if (std::strcmp(action, kOfxActionUnload) == 0 || std::strcmp(action, kOfxActionBeginInstanceChanged) == 0 ||
        std::strcmp(action, kOfxActionEndInstanceChanged) == 0)
        return kOfxStatReplyDefault;
    logf("unhandled action: %s", action);
    return kOfxStatReplyDefault;
}

static void setHost(OfxHost* host)
{
    gHost = host;
    if (!host) return;
    gProp = (OfxPropertySuiteV1*)host->fetchSuite(host->host, kOfxPropertySuite, 1);
    gEffect = (OfxImageEffectSuiteV1*)host->fetchSuite(host->host, kOfxImageEffectSuite, 1);
}

static OfxPlugin gPlugin = {kOfxImageEffectPluginApi, 1, PLUGIN_ID, 0, 1, setHost, mainEntry};

extern "C" {
__attribute__((visibility("default"))) int OfxGetNumberOfPlugins(void) { return 1; }
__attribute__((visibility("default"))) OfxPlugin* OfxGetPlugin(int nth) { return nth == 0 ? &gPlugin : nullptr; }
}
