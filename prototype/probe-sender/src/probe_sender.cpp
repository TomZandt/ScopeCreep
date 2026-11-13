// PROTOTYPE (throwaway): Stage 0 probe Sender, issue #4.
//
// Smallest possible OFX plugin: passes the picture through untouched and logs
// what Resolve tells it (host identity, page, thumbnail flag, OFX colour
// management, image format, render timing). On request (trigger file) it saves
// the next received source frame as a float EXR, exactly as received.
//
// Raw C OFX API, no support library: we want every property read and every
// action visible. CPU render path (no CUDA): this probe copies and logs, it
// does not need the GPU.
//
// Env: SCOPECREEP_PROBE_DIR  (default /tmp/scopecreep-probe) log + dumps + trigger
//      SCOPECREEP_PROBE_STYLE (ocio|full|core|basic|none; default ocio) colour
//                              management style declared to the host
#include "ofxCore.h"
#include "ofxImageEffect.h"
#include "ofxImageEffectExt.h"
#include "ofxGPURender.h"
#include "ofxColour.h"
#include "ofxProperty.h"

#include "exr_dump.h"

#include <chrono>
#include <cstdarg>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>
#include <sys/stat.h>
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
    std::fprintf(f, "[%lld.%03lld pid=%d] ", ms / 1000, ms % 1000, (int)getpid());
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

static void maybeDumpFrame(Instance* inst, const ImageInfo& src, double frame)
{
    const std::string dir = probeDir();
    const std::string trigger = dir + "/dump.trigger";
    if (access(trigger.c_str(), F_OK) != 0) return;
    if (src.depth != kOfxBitDepthFloat || src.components != kOfxImageComponentRGBA) {
        logf("dump: skipped, source is %s %s, not float RGBA", src.depth.c_str(), src.components.c_str());
        return;
    }
    if (unlink(trigger.c_str()) != 0) return;  // another render thread took it
    const int w = src.bounds[2] - src.bounds[0], h = src.bounds[3] - src.bounds[1];
    std::vector<float> contiguous((size_t)w * h * 4);
    for (int y = 0; y < h; ++y)
        std::memcpy(&contiguous[(size_t)y * w * 4], rowPtr(src, src.bounds[1] + y), (size_t)w * 16);
    char name[256];
    std::snprintf(name, sizeof name, "/frame_inst%d_t%.3f_%dx%d.exr", inst->id, frame, w, h);
    const std::string path = dir + name;
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

    if (haveSrc && haveDst && src.depth == kOfxBitDepthFloat && dst.depth == kOfxBitDepthFloat &&
        src.components == kOfxImageComponentRGBA && dst.components == kOfxImageComponentRGBA) {
        const int x1 = rw[0], x2 = rw[2];
        for (int y = rw[1]; y < rw[3]; ++y) {
            if (y < src.bounds[1] || y >= src.bounds[3] || y < dst.bounds[1] || y >= dst.bounds[3]) continue;
            const char* s = rowPtr(src, y) + (long long)(x1 - src.bounds[0]) * 16;
            char* d = (char*)rowPtr(dst, y) + (long long)(x1 - dst.bounds[0]) * 16;
            std::memcpy(d, s, (size_t)(x2 - x1) * 16);
        }
    }
    if (haveSrc) maybeDumpFrame(inst, src, time);

    double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();

    logf("RENDER inst=%d page=%s time=%.3f src_frame=%s render_window=%d,%d,%d,%d render_scale=%s,%s "
         "field=%s sequential=%s interactive=%s draft=%s thumbnail=%s",
         inst->id, inst->page.c_str(), time, ints(inArgs, kOfxImageEffectPropSrcFrame, 1).c_str(),
         rw[0], rw[1], rw[2], rw[3],
         dbl(inArgs, kOfxImageEffectPropRenderScale).c_str(), "-",
         str(inArgs, kOfxImageEffectPropFieldToRender).c_str(),
         str(inArgs, kOfxImageEffectPropSequentialRenderStatus).c_str(),
         str(inArgs, kOfxImageEffectPropInteractiveRenderStatus).c_str(),
         str(inArgs, kOfxImageEffectPropRenderQualityDraft).c_str(),
         ints(srcClipProps, kOfxImageClipPropThumbnail, 1).c_str());
    logf("RENDER inst=%d   src: depth=%s components=%s bounds=%d,%d,%d,%d rowbytes=%d  dst: depth=%s components=%s "
         "bounds=%d,%d,%d,%d  render_page_arg=%s  copy_ms=%.3f gap_since_last_render_ms=%.3f",
         inst->id, src.depth.c_str(), src.components.c_str(), src.bounds[0], src.bounds[1], src.bounds[2],
         src.bounds[3], src.rowBytes, dst.depth.c_str(), dst.components.c_str(), dst.bounds[0], dst.bounds[1],
         dst.bounds[2], dst.bounds[3], str(inArgs, kOfxImageEffectPropResolvePage).c_str(), ms, gapMs);
    // Colour management as seen at render time (first renders of an instance only, to keep the log readable).
    if (inst->renders <= 3) logColour("render", inst, effect);

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
