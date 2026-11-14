// PROTOTYPE (throwaway). A minimal fake OFX host that loads the built probe
// bundle and drives it through load/describe/create/render, so passthrough,
// logging and the trigger-file frame dump can be checked without Resolve.
// This is NOT evidence about what Resolve itself reports.
#include "ofxCore.h"
#include "ofxImageEffect.h"
#include "ofxImageEffectExt.h"
#include "ofxColour.h"
#include "ofxProperty.h"

#include <dlfcn.h>
#include <unistd.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <map>
#include <sstream>
#include <string>
#include <vector>

struct Val { std::string s; int i = 0; double d = 0; void* p = nullptr; };
struct Props { std::map<std::string, std::vector<Val>> m; };

static Val& slot(Props* p, const char* n, int i) {
    auto& v = p->m[n];
    if ((int)v.size() <= i) v.resize(i + 1);
    return v[i];
}
static OfxStatus get(OfxPropertySetHandle h, const char* n, int i, Val** out) {
    auto* p = (Props*)h;
    auto it = p->m.find(n);
    if (it == p->m.end() || (int)it->second.size() <= i) return kOfxStatErrUnknown;
    *out = &it->second[i];
    return kOfxStatOK;
}
static OfxStatus setS(OfxPropertySetHandle h, const char* n, int i, const char* v) { slot((Props*)h, n, i).s = v; return kOfxStatOK; }
static OfxStatus setP(OfxPropertySetHandle h, const char* n, int i, void* v) { slot((Props*)h, n, i).p = v; return kOfxStatOK; }
static OfxStatus setD(OfxPropertySetHandle h, const char* n, int i, double v) { slot((Props*)h, n, i).d = v; return kOfxStatOK; }
static OfxStatus setI(OfxPropertySetHandle h, const char* n, int i, int v) { slot((Props*)h, n, i).i = v; return kOfxStatOK; }
static OfxStatus getS(OfxPropertySetHandle h, const char* n, int i, char** v) { Val* x; auto s = get(h, n, i, &x); if (s == kOfxStatOK) *v = (char*)x->s.c_str(); return s; }
static OfxStatus getP(OfxPropertySetHandle h, const char* n, int i, void** v) { Val* x; auto s = get(h, n, i, &x); if (s == kOfxStatOK) *v = x->p; return s; }
static OfxStatus getD(OfxPropertySetHandle h, const char* n, int i, double* v) { Val* x; auto s = get(h, n, i, &x); if (s == kOfxStatOK) *v = x->d; return s; }
static OfxStatus getI(OfxPropertySetHandle h, const char* n, int i, int* v) { Val* x; auto s = get(h, n, i, &x); if (s == kOfxStatOK) *v = x->i; return s; }
static OfxStatus dim(OfxPropertySetHandle h, const char* n, int* c) { auto* p = (Props*)h; auto it = p->m.find(n); if (it == p->m.end()) return kOfxStatErrUnknown; *c = (int)it->second.size(); return kOfxStatOK; }

static std::map<std::string, Props*> gClips;
static Props gSrcImg, gDstImg;
static OfxPropertySetHandle gEffectProps;

static const void* fetchSuite(OfxPropertySetHandle, const char* name, int) {
    static OfxPropertySuiteV1 ps;
    static OfxImageEffectSuiteV1 es;
    ps.propSetString = setS; ps.propSetPointer = setP; ps.propSetDouble = setD; ps.propSetInt = setI;
    ps.propGetString = getS; ps.propGetPointer = getP; ps.propGetDouble = getD; ps.propGetInt = getI;
    ps.propGetDimension = dim;
    es.getPropertySet = [](OfxImageEffectHandle, OfxPropertySetHandle* p) { *p = gEffectProps; return kOfxStatOK; };
    es.clipDefine = [](OfxImageEffectHandle, const char* n, OfxPropertySetHandle* p) { gClips[n] = new Props; *p = (OfxPropertySetHandle)gClips[n]; return kOfxStatOK; };
    es.clipGetHandle = [](OfxImageEffectHandle, const char* n, OfxImageClipHandle* c, OfxPropertySetHandle* p) { *c = (OfxImageClipHandle)gClips[n]; *p = (OfxPropertySetHandle)gClips[n]; return kOfxStatOK; };
    es.clipGetImage = [](OfxImageClipHandle c, OfxTime, const OfxRectD*, OfxPropertySetHandle* p) { *p = (OfxPropertySetHandle)(c == (OfxImageClipHandle)gClips["Source"] ? &gSrcImg : &gDstImg); return kOfxStatOK; };
    es.clipReleaseImage = [](OfxPropertySetHandle) { return kOfxStatOK; };
    if (!std::strcmp(name, kOfxPropertySuite)) return &ps;
    if (!std::strcmp(name, kOfxImageEffectSuite)) return &es;
    return nullptr;
}

static int fails = 0;
#define CHECK(c, msg) do { if (!(c)) { std::printf("FAIL: %s\n", msg); ++fails; } } while (0)

int main(int argc, char** argv)
{
    if (argc < 2) { std::printf("usage: mock_host_test <path to .ofx>\n"); return 2; }
    setenv("SCOPECREEP_PROBE_DIR", "/tmp/scopecreep-probe-mocktest", 1);
    system("rm -rf /tmp/scopecreep-probe-mocktest");
    void* lib = dlopen(argv[1], RTLD_NOW);
    if (!lib) { std::printf("dlopen: %s\n", dlerror()); return 2; }
    auto getN = (int (*)())dlsym(lib, "OfxGetNumberOfPlugins");
    auto getP_ = (OfxPlugin * (*)(int))dlsym(lib, "OfxGetPlugin");
    CHECK(getN && getN() == 1, "one plugin");
    OfxPlugin* pl = getP_(0);
    CHECK(std::string(pl->pluginIdentifier) == "com.scopecreep.probesender", "plugin id");

    Props hostProps, effect, inArgs, outArgs;
    setS((OfxPropertySetHandle)&hostProps, kOfxPropName, 0, "MockHost");
    setS((OfxPropertySetHandle)&hostProps, kOfxPropLabel, 0, "Mock Host Label");
    setI((OfxPropertySetHandle)&hostProps, kOfxPropVersion, 0, 21);
    setS((OfxPropertySetHandle)&hostProps, kOfxImageEffectPropColourManagementStyle, 0, kOfxImageEffectColourManagementOCIO);
    OfxHost host{(OfxPropertySetHandle)&hostProps, fetchSuite};
    gEffectProps = (OfxPropertySetHandle)&effect;
    pl->setHost(&host);
    OfxImageEffectHandle eh = (OfxImageEffectHandle)&effect;

    CHECK(pl->mainEntry(kOfxActionLoad, nullptr, nullptr, nullptr) == kOfxStatOK, "load");
    CHECK(pl->mainEntry(kOfxActionDescribe, eh, nullptr, nullptr) == kOfxStatOK, "describe");
    CHECK(effect.m[kOfxImageEffectPropColourManagementStyle][0].s == kOfxImageEffectColourManagementOCIO, "declared OCIO style");
    pl->mainEntry(kOfxImageEffectActionDescribeInContext, eh, (OfxPropertySetHandle)&inArgs, nullptr);
    setS((OfxPropertySetHandle)gClips["Source"], kOfxImageClipPropColourspace, 0, "ACEScct");
    setI((OfxPropertySetHandle)gClips["Source"], kOfxImageClipPropThumbnail, 0, 0);
    setS((OfxPropertySetHandle)&inArgs, kOfxImageEffectPropResolvePage, 0, "Color");
    CHECK(pl->mainEntry(kOfxActionCreateInstance, eh, (OfxPropertySetHandle)&inArgs, nullptr) == kOfxStatOK, "create");
    pl->mainEntry(kOfxImageEffectActionGetClipPreferences, eh, nullptr, (OfxPropertySetHandle)&outArgs);

    const int w = 8, h = 4, rowBytes = w * 16 + 32;
    std::vector<char> srcBuf((size_t)rowBytes * h, 0), dstBuf((size_t)rowBytes * h, 0);
    for (size_t i = 0; i < srcBuf.size(); ++i) srcBuf[i] = (char)(i * 37 + 11);
    // a few awkward floats
    float* f = (float*)srcBuf.data(); f[0] = -0.5f; f[1] = 7.25f; f[2] = 1e-40f; f[3] = 1.f;
    for (Props* im : {&gSrcImg, &gDstImg}) {
        auto h_ = (OfxPropertySetHandle)im;
        setP(h_, kOfxImagePropData, 0, im == &gSrcImg ? srcBuf.data() : dstBuf.data());
        int b[4] = {0, 0, w, h};
        for (int i = 0; i < 4; ++i) setI(h_, kOfxImagePropBounds, i, b[i]);
        setI(h_, kOfxImagePropRowBytes, 0, rowBytes);
        setS(h_, kOfxImageEffectPropPixelDepth, 0, kOfxBitDepthFloat);
        setS(h_, kOfxImageEffectPropComponents, 0, kOfxImageComponentRGBA);
    }
    Props rinArgs;
    setD((OfxPropertySetHandle)&rinArgs, kOfxPropTime, 0, 42);
    int rw[4] = {0, 0, w, h};
    for (int i = 0; i < 4; ++i) setI((OfxPropertySetHandle)&rinArgs, kOfxImageEffectPropRenderWindow, i, rw[i]);

    system("touch /tmp/scopecreep-probe-mocktest/dump.trigger");
    CHECK(pl->mainEntry(kOfxImageEffectActionRender, eh, (OfxPropertySetHandle)&rinArgs, nullptr) == kOfxStatOK, "render");
    bool same = true;
    for (int y = 0; y < h; ++y) same &= std::memcmp(&srcBuf[(size_t)y * rowBytes], &dstBuf[(size_t)y * rowBytes], w * 16) == 0;
    CHECK(same, "picture passes through unchanged");
    CHECK(access("/tmp/scopecreep-probe-mocktest/dump.trigger", F_OK) != 0, "trigger consumed");
    CHECK(access("/tmp/scopecreep-probe-mocktest/frame_inst1_t42.000_8x4.exr", F_OK) == 0, "EXR written");
    pl->mainEntry(kOfxImageEffectActionRender, eh, (OfxPropertySetHandle)&rinArgs, nullptr);
    CHECK(system("ls /tmp/scopecreep-probe-mocktest/*.exr | wc -l | grep -qx 1") == 0, "only one dump per trigger");
    pl->mainEntry(kOfxActionDestroyInstance, eh, nullptr, nullptr);

    std::ifstream lf("/tmp/scopecreep-probe-mocktest/probe.log");
    std::stringstream ss; ss << lf.rdbuf();
    std::string log = ss.str();
    for (const char* needle : {"name=MockHost", "page=Color", "colourspace=ACEScct", "thumbnail=0", "bounds=0,0,8,4", "depth=OfxBitDepthFloat", "dump: wrote"})
        CHECK(log.find(needle) != std::string::npos, needle);
    std::printf(fails ? "mock host test: %d FAILED\n" : "mock host test: PASS\n", fails);
    return fails ? 1 : 0;
}
