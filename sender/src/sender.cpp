// The scopecreep Sender: an OFX plugin for DaVinci Resolve (Linux, CUDA) that passes the picture through
// untouched and hands each frame Resolve renders to the scopecreep app through the Feed.
//
// What it does, in order of importance (ADR 0003: Resolve never waits for scopecreep):
//  - with no app attached it does nothing: it tells Resolve the effect is an identity, so Resolve does not even
//    call render; if Resolve renders anyway, the picture is passed through and nothing else happens;
//  - thumbnail renders (Resolve flags them) are never fed;
//  - the Colour Setup can be set by hand on the node (ADR 0006): it wins over the project's, which the app reads by
//    script, and travels with every frame (Feed protocol 1.1);
//  - every other render on Resolve's CUDA path starts an asynchronous copy into the Feed and returns
//    (feed_copy.cpp). Background cache renders and repeat renders cannot be told apart here (Resolve does not
//    flag them and a plugin cannot see the playhead), so frames are tagged with the frame number, the timeline
//    rate, the time of the render call and the instance, and the app decides (app/src/viewer_gate.rs).
//
// Raw C OFX API, no support library and no other dependency: the Sender must stay small, and must never link
// OpenEXR, Imath or anything else Resolve bundles its own copy of (ADR 0002).
#include "ofxCore.h"
#include "ofxImageEffect.h"
#include "ofxImageEffectExt.h"
#include "ofxGPURender.h"
#include "ofxParam.h"
#include "ofxProperty.h"

#include "feed_copy.h"
#include "scopecreep_feed.h"

#include <algorithm>
#include <atomic>
#include <cstddef>
#include <cstdio>
#include <cstring>
#include <string>
#include <time.h>

#define SENDER_ID "com.scopecreep.sender"
#define SENDER_LABEL "scopecreep Sender"
#define SENDER_GROUP "scopecreep"

namespace {

OfxHost* gHost = nullptr;
OfxPropertySuiteV1* gProp = nullptr;
OfxImageEffectSuiteV1* gEffect = nullptr;
OfxParameterSuiteV1* gParam = nullptr;  // optional: without it there is no manual override
std::atomic<uint32_t> gInstances{0};

uint64_t nowNs()
{
    timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

// ------------------------------------------------------------- the manual Colour Setup
//
// Choice values are Resolve's own names, written into the Feed as they are. Resolve saves a choice as its index,
// so options may only ever be appended, never reordered or removed.
const char* const kTimelineSpaces[] = {"DaVinci WG/Intermediate", "ACEScct", "ACEScc", "Rec.709 Gamma 2.4", "Rec.2020 Gamma 2.4"};
const char* const kOutputSpaces[] = {"Rec.709 Gamma 2.4", "P3-D65", "Rec.2100 ST2084", "Rec.2100 HLG", "Rec.2020 Gamma 2.4", "sRGB",
                                     "P3-DCI", "Rec.709 BT.1886", "Rec.2100 ST2084 (1000 nit)"};
const char* const kLevels[] = {"Video", "Full"};
const char* const kLevelsFeed[] = {"video", "full"};
const char* const kAcesVersions[] = {"ACES 2.0", "ACES 1.3"};
const char* const kAcesVersionsFeed[] = {"2.0", "1.3"};
#define PARAM_GROUP "colourSetup"
#define PARAM_MANUAL "colourSetupManual"
#define PARAM_TIMELINE "timelineColourSpace"
#define PARAM_OUTPUT "outputColourSpace"
#define PARAM_LEVELS "levels"
#define PARAM_ACES "acesVersion"

template <size_t N>
const char* pick(const char* const (&options)[N], int i)
{
    return options[i >= 0 && (size_t)i < N ? i : 0];
}

// What identifies a Sender instance. Resolve reports no project, timeline or page to a plugin (Stage 0), so
// this is all there is: an ordinal within this Resolve process, and the timeline frame rate.
struct Instance {
    uint32_t id = 0;
    double frameRate = 0;
    OfxParamHandle manual = nullptr, timeline = nullptr, output = nullptr, levels = nullptr, aces = nullptr;
};

int paramInt(OfxParamHandle h)
{
    int v = 0;
    if (gParam && h) gParam->paramGetValue(h, &v);
    return v;
}

// The manual Colour Setup as Feed protocol 1.1 text into `buf`; returns its length, or 0 when switched off.
size_t manualColourSetup(const Instance* inst, char* buf, size_t cap)
{
    if (!paramInt(inst->manual)) return 0;
    const char* timeline = pick(kTimelineSpaces, paramInt(inst->timeline));
    int n = std::snprintf(buf, cap, "timeline=%s\noutput=%s\nlevels=%s\n", timeline, pick(kOutputSpaces, paramInt(inst->output)),
                          pick(kLevelsFeed, paramInt(inst->levels)));
    if (n > 0 && !std::strncmp(timeline, "ACES", 4) && (size_t)n < cap)
        n += std::snprintf(buf + n, cap - (size_t)n, "aces_version=%s\n", pick(kAcesVersionsFeed, paramInt(inst->aces)));
    return n > 0 && (size_t)n < cap ? (size_t)n : 0;
}

Instance* instanceOf(OfxImageEffectHandle effect)
{
    OfxPropertySetHandle props = nullptr;
    void* data = nullptr;
    if (gEffect->getPropertySet(effect, &props) == kOfxStatOK) gProp->propGetPointer(props, kOfxPropInstanceData, 0, &data);
    return (Instance*)data;
}

std::string hostName()
{
    char* name = nullptr;
    char* version = nullptr;
    gProp->propGetString(gHost->host, kOfxPropName, 0, &name);
    gProp->propGetString(gHost->host, kOfxPropVersionLabel, 0, &version);
    std::string s = name ? name : "unknown host";
    if (version && *version) s += std::string(" ") + version;
    return s;
}

struct Image {
    OfxPropertySetHandle props = nullptr;
    void* data = nullptr;
    int bounds[4] = {0, 0, 0, 0};
    int rowBytes = 0;
    bool floatRgba = false;

    bool fetch(OfxImageClipHandle clip, double time)
    {
        if (gEffect->clipGetImage(clip, time, nullptr, &props) != kOfxStatOK || !props) { props = nullptr; return false; }
        gProp->propGetPointer(props, kOfxImagePropData, 0, &data);
        for (int i = 0; i < 4; ++i) gProp->propGetInt(props, kOfxImagePropBounds, i, &bounds[i]);
        gProp->propGetInt(props, kOfxImagePropRowBytes, 0, &rowBytes);
        char* depth = nullptr;
        char* components = nullptr;
        gProp->propGetString(props, kOfxImageEffectPropPixelDepth, 0, &depth);
        gProp->propGetString(props, kOfxImageEffectPropComponents, 0, &components);
        floatRgba = depth && components && !std::strcmp(depth, kOfxBitDepthFloat) && !std::strcmp(components, kOfxImageComponentRGBA);
        return data != nullptr;
    }
    // Address of pixel (x, y). Rows are stored bottom first; rowBytes may be negative.
    char* at(int x, int y) const { return (char*)data + (ptrdiff_t)(y - bounds[1]) * rowBytes + (ptrdiff_t)(x - bounds[0]) * 16; }
    ~Image()
    {
        if (props) gEffect->clipReleaseImage(props);
    }
};

// ------------------------------------------------------------------ actions

OfxStatus describe(OfxImageEffectHandle effect, const char* label)
{
    OfxPropertySetHandle p = nullptr;
    gEffect->getPropertySet(effect, &p);
    gProp->propSetString(p, kOfxPropLabel, 0, label);
    gProp->propSetString(p, kOfxImageEffectPluginPropGrouping, 0, SENDER_GROUP);
    gProp->propSetString(p, kOfxPropPluginDescription, 0,
                         "Sends the picture to the scopecreep app. Put it on the last timeline node. The picture passes through unchanged.");
    gProp->propSetString(p, kOfxImageEffectPropSupportedContexts, 0, kOfxImageEffectContextFilter);
    gProp->propSetString(p, kOfxImageEffectPropSupportedContexts, 1, kOfxImageEffectContextGeneral);
    gProp->propSetString(p, kOfxImageEffectPropSupportedPixelDepths, 0, kOfxBitDepthFloat);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsMultipleClipDepths, 0, 0);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsTiles, 0, 0);
    gProp->propSetInt(p, kOfxImageEffectPropSupportsMultiResolution, 0, 0);
    gProp->propSetString(p, kOfxImageEffectPluginRenderThreadSafety, 0, kOfxImageEffectRenderFullySafe);
    // Resolve's CUDA render path: frames arrive as GPU pointers, so Resolve does not copy them to host memory
    // itself (that copy slowed UHD playback at 50 and 60 fps to about 30 fps in Stage 0).
    gProp->propSetString(p, kOfxImageEffectPropCudaRenderSupported, 0, "true");
    gProp->propSetString(p, kOfxImageEffectPropCudaStreamSupported, 0, "true");
    return kOfxStatOK;
}

template <size_t N>
void defineChoice(OfxParamSetHandle ps, const char* name, const char* label, const char* hint, const char* const (&options)[N])
{
    OfxPropertySetHandle p = nullptr;
    if (gParam->paramDefine(ps, kOfxParamTypeChoice, name, &p) != kOfxStatOK || !p) return;
    gProp->propSetString(p, kOfxPropLabel, 0, label);
    gProp->propSetString(p, kOfxParamPropHint, 0, hint);
    gProp->propSetString(p, kOfxParamPropParent, 0, PARAM_GROUP);
    gProp->propSetInt(p, kOfxParamPropAnimates, 0, 0);
    gProp->propSetInt(p, kOfxParamPropDefault, 0, 0);
    for (size_t i = 0; i < N; ++i) gProp->propSetString(p, kOfxParamPropChoiceOption, (int)i, options[i]);
}

// The manual override: for Resolve's free version (no scripting), or to say what the project cannot (ACES 1.3).
void defineColourSetupParams(OfxImageEffectHandle effect)
{
    OfxParamSetHandle ps = nullptr;
    if (!gParam || gEffect->getParamSet(effect, &ps) != kOfxStatOK || !ps) return;
    OfxPropertySetHandle p = nullptr;
    if (gParam->paramDefine(ps, kOfxParamTypeGroup, PARAM_GROUP, &p) == kOfxStatOK && p) gProp->propSetString(p, kOfxPropLabel, 0, "Colour Setup");
    if (gParam->paramDefine(ps, kOfxParamTypeBoolean, PARAM_MANUAL, &p) == kOfxStatOK && p) {
        gProp->propSetString(p, kOfxPropLabel, 0, "Set Colour Setup manually");
        gProp->propSetString(p, kOfxParamPropHint, 0,
                             "Off: scopecreep reads the Colour Setup from the project (Resolve Studio, external scripting set to Local). "
                             "On: these settings are used instead.");
        gProp->propSetString(p, kOfxParamPropParent, 0, PARAM_GROUP);
        gProp->propSetInt(p, kOfxParamPropAnimates, 0, 0);
        gProp->propSetInt(p, kOfxParamPropDefault, 0, 0);
    }
    defineChoice(ps, PARAM_TIMELINE, "Timeline Colour Space", "The colour space the project grades in.", kTimelineSpaces);
    defineChoice(ps, PARAM_OUTPUT, "Output Colour Space", "What Resolve delivers to the monitor. For ACES, the output transform.", kOutputSpaces);
    defineChoice(ps, PARAM_LEVELS, "Levels", "Video (legal range) or Full (data levels), as set in Resolve.", kLevels);
    defineChoice(ps, PARAM_ACES, "ACES version", "Only used with an ACES timeline. Resolve does not tell scripts which ACES version a project uses.", kAcesVersions);
}

OfxStatus describeInContext(OfxImageEffectHandle effect)
{
    defineColourSetupParams(effect);
    for (const char* name : {kOfxImageEffectSimpleSourceClipName, kOfxImageEffectOutputClipName}) {
        OfxPropertySetHandle clip = nullptr;
        gEffect->clipDefine(effect, name, &clip);
        gProp->propSetString(clip, kOfxImageEffectPropSupportedComponents, 0, kOfxImageComponentRGBA);
        gProp->propSetInt(clip, kOfxImageEffectPropSupportsTiles, 0, 0);
    }
    return kOfxStatOK;
}

OfxStatus createInstance(OfxImageEffectHandle effect)
{
    scopecreep::feedOpen(hostName().c_str());
    Instance* inst = new Instance();
    inst->id = gInstances.fetch_add(1) + 1;
    OfxPropertySetHandle props = nullptr;
    gEffect->getPropertySet(effect, &props);
    gProp->propGetDouble(props, kOfxImageEffectPropFrameRate, 0, &inst->frameRate);
    OfxParamSetHandle ps = nullptr;
    if (gParam && gEffect->getParamSet(effect, &ps) == kOfxStatOK && ps) {
        gParam->paramGetHandle(ps, PARAM_MANUAL, &inst->manual, nullptr);
        gParam->paramGetHandle(ps, PARAM_TIMELINE, &inst->timeline, nullptr);
        gParam->paramGetHandle(ps, PARAM_OUTPUT, &inst->output, nullptr);
        gParam->paramGetHandle(ps, PARAM_LEVELS, &inst->levels, nullptr);
        gParam->paramGetHandle(ps, PARAM_ACES, &inst->aces, nullptr);
    }
    gProp->propSetPointer(props, kOfxPropInstanceData, 0, inst);
    return kOfxStatOK;
}

OfxStatus destroyInstance(OfxImageEffectHandle effect)
{
    delete instanceOf(effect);
    return kOfxStatOK;
}

// With no app attached the Sender is an identity: Resolve can skip it altogether.
OfxStatus isIdentity(OfxPropertySetHandle inArgs, OfxPropertySetHandle outArgs)
{
    if (scopecreep::feedReaderPresent()) return kOfxStatReplyDefault;
    double time = 0;
    gProp->propGetDouble(inArgs, kOfxPropTime, 0, &time);
    gProp->propSetString(outArgs, kOfxPropName, 0, kOfxImageEffectSimpleSourceClipName);
    gProp->propSetDouble(outArgs, kOfxPropTime, 0, time);
    return kOfxStatOK;
}

OfxStatus render(OfxImageEffectHandle effect, OfxPropertySetHandle inArgs)
{
    const uint64_t t0 = nowNs();
    Instance* inst = instanceOf(effect);
    if (!inst) return kOfxStatFailed;

    double time = 0;
    gProp->propGetDouble(inArgs, kOfxPropTime, 0, &time);
    int win[4] = {0, 0, 0, 0};
    for (int i = 0; i < 4; ++i) gProp->propGetInt(inArgs, kOfxImageEffectPropRenderWindow, i, &win[i]);
    int cudaEnabled = 0;
    void* stream = nullptr;
    gProp->propGetInt(inArgs, kOfxImageEffectPropCudaEnabled, 0, &cudaEnabled);
    if (cudaEnabled) gProp->propGetPointer(inArgs, kOfxImageEffectPropCudaStream, 0, &stream);

    OfxImageClipHandle srcClip = nullptr, dstClip = nullptr;
    OfxPropertySetHandle srcClipProps = nullptr, dstClipProps = nullptr;
    gEffect->clipGetHandle(effect, kOfxImageEffectSimpleSourceClipName, &srcClip, &srcClipProps);
    gEffect->clipGetHandle(effect, kOfxImageEffectOutputClipName, &dstClip, &dstClipProps);
    Image src, dst;
    if (!src.fetch(srcClip, time) || !dst.fetch(dstClip, time)) return kOfxStatFailed;

    // The part of the render window both images cover.
    const int x1 = std::max(win[0], std::max(src.bounds[0], dst.bounds[0])), x2 = std::min(win[2], std::min(src.bounds[2], dst.bounds[2]));
    const int y1 = std::max(win[1], std::max(src.bounds[1], dst.bounds[1])), y2 = std::min(win[3], std::min(src.bounds[3], dst.bounds[3]));
    const int width = x2 - x1, height = y2 - y1;
    if (!src.floatRgba || !dst.floatRgba || width <= 0 || height <= 0) return kOfxStatFailed;

    int thumbnail = 0;
    gProp->propGetInt(srcClipProps, kOfxImageClipPropThumbnail, 0, &thumbnail);

    scopecreep::Render what = scopecreep::Render::Idle;
    if (cudaEnabled) {
        if (!scopecreep::cudaPassthrough(stream, dst.at(x1, y1), (size_t)dst.rowBytes, src.at(x1, y1), (size_t)src.rowBytes, (size_t)width * 16, (size_t)height))
            return kOfxStatFailed;
        if (thumbnail) {
            what = scopecreep::Render::Thumbnail;
        } else if (scopecreep::feedReaderPresent()) {
            scopecreep::FrameInfo info;
            info.width = width;
            info.height = height;
            info.frameTime = time;
            info.frameRate = inst->frameRate;
            info.instance = inst->id;
            info.renderNs = t0;
            char colour[512];
            info.colourSetupBytes = manualColourSetup(inst, colour, sizeof colour);
            info.colourSetup = colour;
            info.colourProvenance = info.colourSetupBytes ? SCFEED_PROVENANCE_MANUAL : SCFEED_PROVENANCE_NONE;
            scopecreep::feedSubmitCuda(stream, src.at(x1, y1), (size_t)src.rowBytes, info);
            what = scopecreep::Render::Fed;
        }
    } else {
        // Resolve is rendering on the CPU or another GPU API. Pass the picture through; frames are not fed:
        // copying them from here would make Resolve wait.
        for (int y = y1; y < y2; ++y) std::memcpy(dst.at(x1, y), src.at(x1, y), (size_t)width * 16);
        if (thumbnail) {
            what = scopecreep::Render::Thumbnail;
        } else if (scopecreep::feedReaderPresent()) {
            what = scopecreep::Render::Unsupported;
            scopecreep::feedSetError("Resolve is not rendering this effect with CUDA, so no frames can be sent. Set GPU processing mode to CUDA in Resolve's preferences.");
        }
    }
    scopecreep::feedCount(what, t0, nowNs() - t0);
    return kOfxStatOK;
}

OfxStatus mainEntry(const char* action, const void* handle, OfxPropertySetHandle inArgs, OfxPropertySetHandle outArgs, const char* label)
{
    OfxImageEffectHandle effect = (OfxImageEffectHandle)handle;
    if (!std::strcmp(action, kOfxImageEffectActionRender)) return render(effect, inArgs);
    if (!std::strcmp(action, kOfxImageEffectActionIsIdentity)) return isIdentity(inArgs, outArgs);
    if (!std::strcmp(action, kOfxActionLoad)) {
        if (!gProp || !gEffect) return kOfxStatErrMissingHostFeature;
        scopecreep::feedOpen(hostName().c_str());
        return kOfxStatOK;
    }
    if (!std::strcmp(action, kOfxActionDescribe)) return describe(effect, label);
    if (!std::strcmp(action, kOfxImageEffectActionDescribeInContext)) return describeInContext(effect);
    if (!std::strcmp(action, kOfxActionCreateInstance)) return createInstance(effect);
    if (!std::strcmp(action, kOfxActionDestroyInstance)) return destroyInstance(effect);
    return kOfxStatReplyDefault;
}

void setHost(OfxHost* host)
{
    gHost = host;
    if (!host) return;
    gProp = (OfxPropertySuiteV1*)host->fetchSuite(host->host, kOfxPropertySuite, 1);
    gEffect = (OfxImageEffectSuiteV1*)host->fetchSuite(host->host, kOfxImageEffectSuite, 1);
    gParam = (OfxParameterSuiteV1*)host->fetchSuite(host->host, kOfxParameterSuite, 1);
}

OfxStatus senderEntry(const char* action, const void* handle, OfxPropertySetHandle inArgs, OfxPropertySetHandle outArgs)
{
    return mainEntry(action, handle, inArgs, outArgs, SENDER_LABEL);
}
OfxPlugin gSender = {kOfxImageEffectPluginApi, 1, SENDER_ID, 0, 1, setHost, senderEntry};

#ifdef SCOPECREEP_DEV_PROBE_ALIAS
// Development only (make PROBE_ALIAS=1): also answer to the Stage 0 probe's identifier, so the test projects,
// which have the probe on a timeline node that scripts cannot change, load the real Sender in its place.
OfxStatus aliasEntry(const char* action, const void* handle, OfxPropertySetHandle inArgs, OfxPropertySetHandle outArgs)
{
    return mainEntry(action, handle, inArgs, outArgs, SENDER_LABEL " (standing in for the probe)");
}
OfxPlugin gAlias = {kOfxImageEffectPluginApi, 1, "com.scopecreep.probesender", 0, 1, setHost, aliasEntry};
#endif

}  // namespace

extern "C" {
__attribute__((visibility("default"))) int OfxGetNumberOfPlugins(void)
{
#ifdef SCOPECREEP_DEV_PROBE_ALIAS
    return 2;
#else
    return 1;
#endif
}
__attribute__((visibility("default"))) OfxPlugin* OfxGetPlugin(int nth)
{
    if (nth == 0) return &gSender;
#ifdef SCOPECREEP_DEV_PROBE_ALIAS
    if (nth == 1) return &gAlias;
#endif
    return nullptr;
}
}
