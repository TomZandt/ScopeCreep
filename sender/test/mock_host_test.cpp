// Drives the built Sender plugin through a minimal fake OFX host that renders on CUDA, the way Resolve does,
// and checks what comes out of the Feed with the C reader. No Resolve needed; an NVIDIA GPU is.
//
//   mock_host_test <path to scopecreep_sender.ofx>
//
// This checks the Sender against the Feed protocol and the rules of ADR 0003. It is not evidence about how
// Resolve itself behaves.
#include "ofxCore.h"
#include "ofxGPURender.h"
#include "ofxImageEffect.h"
#include "ofxImageEffectExt.h"
#include "ofxParam.h"
#include "ofxProperty.h"

#include "scopecreep_feed.h"

#include <cuda_runtime.h>
#include <dlfcn.h>
#include <signal.h>
#include <sys/wait.h>

#include <chrono>
#include <cstdarg>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <string>
#include <thread>
#include <vector>

// ---------------------------------------------------------------- a tiny OFX host

struct Val { std::string s; int i = 0; double d = 0; void* p = nullptr; };
struct Props { std::map<std::string, std::vector<Val>> m; };

static Val& at(Props* p, const char* n, int i)
{
    auto& v = p->m[n];
    if ((int)v.size() <= i) v.resize((size_t)i + 1);
    return v[(size_t)i];
}
static OfxStatus find(OfxPropertySetHandle h, const char* n, int i, Val** out)
{
    auto* p = (Props*)h;
    auto it = p->m.find(n);
    if (it == p->m.end() || (int)it->second.size() <= i) return kOfxStatErrUnknown;
    *out = &it->second[(size_t)i];
    return kOfxStatOK;
}
static OfxStatus setS(OfxPropertySetHandle h, const char* n, int i, const char* v) { at((Props*)h, n, i).s = v; return kOfxStatOK; }
static OfxStatus setP(OfxPropertySetHandle h, const char* n, int i, void* v) { at((Props*)h, n, i).p = v; return kOfxStatOK; }
static OfxStatus setD(OfxPropertySetHandle h, const char* n, int i, double v) { at((Props*)h, n, i).d = v; return kOfxStatOK; }
static OfxStatus setI(OfxPropertySetHandle h, const char* n, int i, int v) { at((Props*)h, n, i).i = v; return kOfxStatOK; }
static OfxStatus getS(OfxPropertySetHandle h, const char* n, int i, char** v) { Val* x; auto s = find(h, n, i, &x); if (s == kOfxStatOK) *v = (char*)x->s.c_str(); return s; }
static OfxStatus getP(OfxPropertySetHandle h, const char* n, int i, void** v) { Val* x; auto s = find(h, n, i, &x); if (s == kOfxStatOK) *v = x->p; return s; }
static OfxStatus getD(OfxPropertySetHandle h, const char* n, int i, double* v) { Val* x; auto s = find(h, n, i, &x); if (s == kOfxStatOK) *v = x->d; return s; }
static OfxStatus getI(OfxPropertySetHandle h, const char* n, int i, int* v) { Val* x; auto s = find(h, n, i, &x); if (s == kOfxStatOK) *v = x->i; return s; }

static std::map<std::string, Props*> gClips;
static Props gSrcImage, gDstImage, gEffectProps;

// Parameters: what the plugin defined (type and properties) and the value the "user" set (bool or choice index).
struct Param { std::string type; Props props; int value = 0; };
static std::map<std::string, Param> gParams;
static Props gParamSetProps;

static OfxStatus paramGetValue(OfxParamHandle h, ...)
{
    va_list ap;
    va_start(ap, h);
    int* out = va_arg(ap, int*);
    va_end(ap);
    *out = ((Param*)h)->value;
    return kOfxStatOK;
}

static const void* fetchSuite(OfxPropertySetHandle, const char* name, int)
{
    static OfxPropertySuiteV1 ps;
    static OfxImageEffectSuiteV1 es;
    static OfxParameterSuiteV1 prm;
    prm.paramDefine = [](OfxParamSetHandle, const char* type, const char* n, OfxPropertySetHandle* p) {
        gParams[n].type = type;
        if (p) *p = (OfxPropertySetHandle)&gParams[n].props;
        return kOfxStatOK;
    };
    prm.paramGetHandle = [](OfxParamSetHandle, const char* n, OfxParamHandle* h, OfxPropertySetHandle* p) {
        auto it = gParams.find(n);
        if (it == gParams.end()) return kOfxStatErrUnknown;
        *h = (OfxParamHandle)&it->second;
        if (p) *p = (OfxPropertySetHandle)&it->second.props;
        return kOfxStatOK;
    };
    prm.paramSetGetPropertySet = [](OfxParamSetHandle, OfxPropertySetHandle* p) { *p = (OfxPropertySetHandle)&gParamSetProps; return kOfxStatOK; };
    prm.paramGetValue = paramGetValue;
    prm.paramGetPropertySet = [](OfxParamHandle h, OfxPropertySetHandle* p) { *p = (OfxPropertySetHandle)&((Param*)h)->props; return kOfxStatOK; };
    es.getParamSet = [](OfxImageEffectHandle, OfxParamSetHandle* p) { *p = (OfxParamSetHandle)&gParamSetProps; return kOfxStatOK; };
    ps.propSetString = setS; ps.propSetPointer = setP; ps.propSetDouble = setD; ps.propSetInt = setI;
    ps.propGetString = getS; ps.propGetPointer = getP; ps.propGetDouble = getD; ps.propGetInt = getI;
    es.getPropertySet = [](OfxImageEffectHandle, OfxPropertySetHandle* p) { *p = (OfxPropertySetHandle)&gEffectProps; return kOfxStatOK; };
    es.clipDefine = [](OfxImageEffectHandle, const char* n, OfxPropertySetHandle* p) { gClips[n] = new Props; *p = (OfxPropertySetHandle)gClips[n]; return kOfxStatOK; };
    es.clipGetHandle = [](OfxImageEffectHandle, const char* n, OfxImageClipHandle* c, OfxPropertySetHandle* p) {
        *c = (OfxImageClipHandle)gClips[n];
        if (p) *p = (OfxPropertySetHandle)gClips[n];
        return kOfxStatOK;
    };
    es.clipGetImage = [](OfxImageClipHandle c, OfxTime, const OfxRectD*, OfxPropertySetHandle* p) {
        *p = (OfxPropertySetHandle)(c == (OfxImageClipHandle)gClips[kOfxImageEffectSimpleSourceClipName] ? &gSrcImage : &gDstImage);
        return kOfxStatOK;
    };
    es.clipReleaseImage = [](OfxPropertySetHandle) { return kOfxStatOK; };
    if (!std::strcmp(name, kOfxPropertySuite)) return &ps;
    if (!std::strcmp(name, kOfxImageEffectSuite)) return &es;
    if (!std::strcmp(name, kOfxParameterSuite)) return &prm;
    return nullptr;
}

// The index of `option` in a choice parameter the plugin defined, or -1.
static int choiceIndex(const char* param, const char* option)
{
    auto& opts = gParams[param].props.m[kOfxParamPropChoiceOption];
    for (size_t i = 0; i < opts.size(); ++i)
        if (opts[i].s == option) return (int)i;
    return -1;
}

// ---------------------------------------------------------------------- the test

static int gFails = 0;
#define CHECK(cond, what) do { if (cond) std::printf("  ok    %s\n", what); else { std::printf("  FAIL  %s  (%s:%d)\n", what, __FILE__, __LINE__); ++gFails; } } while (0)

static OfxPlugin* gPlugin = nullptr;
static OfxImageEffectHandle gEffect = (OfxImageEffectHandle)&gEffectProps;
static cudaStream_t gHostStream = nullptr;
static double nowMs() { return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count(); }

// Is a parameter enabled (editable) in the host's UI? kOfxParamPropEnabled defaults to 1.
static bool enabled(const char* param)
{
    auto& v = gParams[param].props.m[kOfxParamPropEnabled];
    return v.empty() || v[0].i != 0;
}

// The user changes a parameter in the host's UI: the host stores the value, then tells the plugin.
static OfxStatus userSets(const char* param, int value)
{
    gParams[param].value = value;
    Props in;
    setS((OfxPropertySetHandle)&in, kOfxPropType, 0, kOfxTypeParameter);
    setS((OfxPropertySetHandle)&in, kOfxPropName, 0, param);
    setS((OfxPropertySetHandle)&in, kOfxPropChangeReason, 0, kOfxChangeUserEdited);
    setD((OfxPropertySetHandle)&in, kOfxPropTime, 0, 0.0);
    return gPlugin->mainEntry(kOfxActionInstanceChanged, gEffect, (OfxPropertySetHandle)&in, nullptr);
}

// A frame "Resolve rendered": on the GPU, rows `pitch` bytes apart (wider than the picture, as hosts may do).
struct GpuFrame {
    int w, h;
    size_t pitch;
    void *src = nullptr, *dst = nullptr;
    std::vector<float> host;  // what the source holds, tightly packed

    GpuFrame(int w_, int h_) : w(w_), h(h_), pitch((size_t)w_ * 16 + 64), host((size_t)w_ * h_ * 4)
    {
        if (cudaMalloc(&src, pitch * (size_t)h) != cudaSuccess || cudaMalloc(&dst, pitch * (size_t)h) != cudaSuccess) {
            std::printf("cudaMalloc failed: is the GPU out of memory?\n");
            std::exit(2);
        }
    }
    void fill(unsigned seed)
    {
        for (size_t i = 0; i < host.size(); ++i) host[i] = (float)((i * 2654435761u + seed * 40503u) % 100003u) / 50000.0f - 0.5f;
        host[0] = -0.5f; host[1] = 7.25f; host[2] = 1e-40f; host[3] = 65504.0f;  // negative, over-range, denormal, huge
        cudaMemcpy2D(src, pitch, host.data(), (size_t)w * 16, (size_t)w * 16, (size_t)h, cudaMemcpyHostToDevice);
        cudaMemset(dst, 0, pitch * (size_t)h);
        cudaDeviceSynchronize();  // the host stream is non-blocking: it does not wait for the legacy stream by itself
    }
    bool passedThrough()
    {
        std::vector<float> out(host.size());
        cudaStreamSynchronize(gHostStream);
        cudaMemcpy2D(out.data(), (size_t)w * 16, dst, pitch, (size_t)w * 16, (size_t)h, cudaMemcpyDeviceToHost);
        return std::memcmp(out.data(), host.data(), host.size() * 4) == 0;
    }
};

static void describeImages(void* src, void* dst, int w, int h, size_t pitch)
{
    for (Props* im : {&gSrcImage, &gDstImage}) {
        auto p = (OfxPropertySetHandle)im;
        setP(p, kOfxImagePropData, 0, im == &gSrcImage ? src : dst);
        const int b[4] = {0, 0, w, h};
        for (int i = 0; i < 4; ++i) setI(p, kOfxImagePropBounds, i, b[i]);
        setI(p, kOfxImagePropRowBytes, 0, (int)pitch);
        setS(p, kOfxImageEffectPropPixelDepth, 0, kOfxBitDepthFloat);
        setS(p, kOfxImageEffectPropComponents, 0, kOfxImageComponentRGBA);
    }
}

// One render call, as the host makes it. Returns the time spent inside the plugin in ms.
static double render(void* src, void* dst, int w, int h, size_t pitch, double time, bool cuda, bool thumbnail = false)
{
    describeImages(src, dst, w, h, pitch);
    setI((OfxPropertySetHandle)gClips[kOfxImageEffectSimpleSourceClipName], kOfxImageClipPropThumbnail, 0, thumbnail ? 1 : 0);
    Props in;
    setD((OfxPropertySetHandle)&in, kOfxPropTime, 0, time);
    const int win[4] = {0, 0, w, h};
    for (int i = 0; i < 4; ++i) setI((OfxPropertySetHandle)&in, kOfxImageEffectPropRenderWindow, i, win[i]);
    setI((OfxPropertySetHandle)&in, kOfxImageEffectPropCudaEnabled, 0, cuda ? 1 : 0);
    if (cuda) setP((OfxPropertySetHandle)&in, kOfxImageEffectPropCudaStream, 0, gHostStream);
    const double t0 = nowMs();
    const OfxStatus st = gPlugin->mainEntry(kOfxImageEffectActionRender, gEffect, (OfxPropertySetHandle)&in, nullptr);
    const double ms = nowMs() - t0;
    if (st != kOfxStatOK) { std::printf("  FAIL  render returned %d\n", st); ++gFails; }
    return ms;
}
static double render(GpuFrame& f, double time, bool thumbnail = false) { return render(f.src, f.dst, f.w, f.h, f.pitch, time, true, thumbnail); }

static bool isIdentity()
{
    Props in, out;
    setD((OfxPropertySetHandle)&in, kOfxPropTime, 0, 5.0);
    const OfxStatus st = gPlugin->mainEntry(kOfxImageEffectActionIsIdentity, gEffect, (OfxPropertySetHandle)&in, (OfxPropertySetHandle)&out);
    return st == kOfxStatOK && out.m[kOfxPropName][0].s == kOfxImageEffectSimpleSourceClipName && out.m[kOfxPropTime][0].d == 5.0;
}

// The Feed header as anyone can read it, without attaching as a reader.
static scfeed_header peek(const char* path)
{
    scfeed_header h;
    std::memset(&h, 0, sizeof h);
    if (FILE* f = std::fopen(path, "rb")) {
        if (std::fread(&h, 1, sizeof h, f) != sizeof h) std::memset(&h, 0, sizeof h);
        std::fclose(f);
    }
    return h;
}

// Wait (the test may wait; the Sender may not) for the next frame in the Feed. Returns the slot or -1.
static int nextFrame(scfeed_reader& r, int timeoutMs = 2000)
{
    const double end = nowMs() + timeoutMs;
    while (nowMs() < end) {
        scfeed_reader_heartbeat(&r, scfeed_now_ns());
        const int i = scfeed_reader_claim(&r);
        if (i >= 0) return i;
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    return -1;
}

// Wait (the test may wait; a render call may not) until `done()` holds. The Sender's worker thread does the slow
// work (building a ring, looking at another Resolve's Feed) after the render call has returned.
template <class F>
static bool waitFor(F done, int timeoutMs = 3000)
{
    const double end = nowMs() + timeoutMs;
    while (nowMs() < end) {
        if (done()) return true;
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    return done();
}

// Re-open the Feed after the Sender replaced it (as the app does when it sees `retired`).
static bool reopen(scfeed_reader& r, const char* path)
{
    scfeed_reader_close(&r);
    return scfeed_reader_open(&r, path) == SCFEED_OK;
}

// Run as a second Resolve: another live process already owns the Feed. The Sender must join it as a guest
// (registered, no frames, no counters), and take the Feed over once the owner has quit.
static int guestTest(const char* feed)
{
    const pid_t other = fork();
    if (other == 0) { pause(); _exit(0); }  // "the first Resolve": alive until killed
    scfeed_writer ow;
    if (scfeed_writer_create(&ow, feed, 4, 64 * 36 * 16, "other", "Resolve A") != SCFEED_OK) return 2;
    ow.hdr->sender_pid = (uint64_t)other;
    scfeed_registry_add(ow.hdr, (uint64_t)other, 1, 25.0, "Resolve A");
    scfeed_reader r;
    scfeed_reader_open(&r, feed);

    std::printf("a second Resolve joins the Feed as a guest\n");
    CHECK(gPlugin->mainEntry(kOfxActionLoad, nullptr, nullptr, nullptr) == kOfxStatOK, "load");
    setD((OfxPropertySetHandle)&gEffectProps, kOfxImageEffectPropFrameRate, 0, 50.0);
    CHECK(gPlugin->mainEntry(kOfxActionCreateInstance, gEffect, nullptr, nullptr) == kOfxStatOK, "create instance");
    CHECK(scfeed_load32(&r.hdr->retired) == 0 && r.hdr->sender_pid == (uint64_t)other, "the Feed is not replaced");
    GpuFrame f(64, 36);
    f.fill(1);
    render(f, 7.0);
    CHECK(f.passedThrough(), "the picture passes through unchanged");
    std::this_thread::sleep_for(std::chrono::milliseconds(50));
    CHECK(r.hdr->next_sequence == 0 && scfeed_reader_claim(&r) < 0, "the guest copies nothing");
    CHECK(r.hdr->renders_total == 0, "and does not touch the owner's counters");
    const int e = scfeed_registry_find(r.hdr, (uint64_t)getpid(), 1);
    CHECK(e >= 0 && r.hdr->registry[e].renders == 1 && r.hdr->registry[e].frame_rate == 50.0, "its instance and render are registered");

    std::printf("the first Resolve quits\n");
    kill(other, SIGKILL);
    waitpid(other, nullptr, 0);
    std::this_thread::sleep_for(std::chrono::milliseconds(1100));
    f.fill(2);
    const double takeoverCall = render(f, 8.0);
    CHECK(takeoverCall < 5.0, "the render call that notices the owner has gone returns at once: the Feed is not looked at on the render thread");
    CHECK(waitFor([&] { return scfeed_load32(&r.hdr->retired) == 1; }), "within a second the guest takes the Feed over");
    CHECK(reopen(r, feed), "the app re-attaches");
    CHECK(r.hdr->sender_pid == (uint64_t)getpid(), "the Feed is now this Resolve's");
    CHECK(scfeed_registry_find(r.hdr, (uint64_t)getpid(), 1) >= 0 && scfeed_registry_find(r.hdr, (uint64_t)other, 1) < 0, "listing only its own instances");
    f.fill(3);
    render(f, 9.0);  // asks for the ring
    CHECK(waitFor([&] { return scfeed_load32(&r.hdr->retired) == 1; }), "the first frame makes the ring");
    reopen(r, feed);
    f.fill(4);
    render(f, 10.0);
    const int i = nextFrame(r);
    CHECK(i >= 0 && scfeed_slot_at(r.hdr, (uint32_t)i)->frame_time == 10.0, "and it sends frames");
    scfeed_reader_close(&r);
    gPlugin->mainEntry(kOfxActionDestroyInstance, gEffect, nullptr, nullptr);
    scfeed_writer_close(&ow);
    unlink(feed);
    std::printf(gFails ? "\nSender guest test: %d FAILED\n" : "\nSender guest test: all passed\n", gFails);
    return gFails ? 1 : 0;
}

int main(int argc, char** argv)
{
    setvbuf(stdout, nullptr, _IOLBF, 0);
    if (argc < 2) { std::printf("usage: mock_host_test <path to .ofx> [--guest]\n"); return 2; }
    const bool guest = argc > 2 && !std::strcmp(argv[2], "--guest");
    char feed[128];
    std::snprintf(feed, sizeof feed, "/dev/shm/scopecreep-sender-test-%d", (int)getpid());
    setenv(SCFEED_PATH_ENV, feed, 1);
    unlink(feed);
    if (cudaStreamCreateWithFlags(&gHostStream, cudaStreamNonBlocking) != cudaSuccess) {
        std::printf("CUDA is not available (is the GPU out of memory?): cannot run the Sender test\n");
        return 2;
    }

    void* lib = dlopen(argv[1], RTLD_NOW);
    if (!lib) { std::printf("dlopen: %s\n", dlerror()); return 2; }
    auto count = (int (*)())dlsym(lib, "OfxGetNumberOfPlugins");
    auto get = (OfxPlugin * (*)(int))dlsym(lib, "OfxGetPlugin");

    std::printf("the plugin identifies itself\n");
    CHECK(count && count() >= 1, "the bundle exports a plugin");
    gPlugin = get(0);
    CHECK(std::string(gPlugin->pluginIdentifier) == "com.scopecreep.sender", "plugin identifier com.scopecreep.sender");
    Props hostProps;
    setS((OfxPropertySetHandle)&hostProps, kOfxPropName, 0, "MockHost");
    setS((OfxPropertySetHandle)&hostProps, kOfxPropVersionLabel, 0, "9.9");
    OfxHost host{(OfxPropertySetHandle)&hostProps, fetchSuite};
    gPlugin->setHost(&host);
    if (guest) {
        gPlugin->mainEntry(kOfxActionDescribe, gEffect, nullptr, nullptr);
        Props gctx;
        gPlugin->mainEntry(kOfxImageEffectActionDescribeInContext, gEffect, (OfxPropertySetHandle)&gctx, nullptr);
        return guestTest(feed);
    }
    CHECK(gPlugin->mainEntry(kOfxActionLoad, nullptr, nullptr, nullptr) == kOfxStatOK, "load");
    CHECK(gPlugin->mainEntry(kOfxActionDescribe, gEffect, nullptr, nullptr) == kOfxStatOK, "describe");
    CHECK(gEffectProps.m[kOfxPropLabel][0].s == "scopecreep Sender", "label is 'scopecreep Sender'");
    CHECK(gEffectProps.m[kOfxImageEffectPropCudaRenderSupported][0].s == "true", "declares Resolve's CUDA render path");
    CHECK(gEffectProps.m[kOfxImageEffectPropCudaStreamSupported][0].s == "true", "declares CUDA stream support");
    Props ctx;
    gPlugin->mainEntry(kOfxImageEffectActionDescribeInContext, gEffect, (OfxPropertySetHandle)&ctx, nullptr);
    setD((OfxPropertySetHandle)&gEffectProps, kOfxImageEffectPropFrameRate, 0, 24.0);
    CHECK(gPlugin->mainEntry(kOfxActionCreateInstance, gEffect, nullptr, nullptr) == kOfxStatOK, "create instance");

    GpuFrame f(64, 36);

    std::printf("with no app attached the Sender does no work\n");
    scfeed_header h = peek(feed);
    CHECK(h.magic == SCFEED_MAGIC && h.version_major == SCFEED_VERSION_MAJOR, "a Feed is published as soon as the Sender is loaded");
    CHECK(std::string(h.sender_name) == "scopecreep Sender", "the Feed names the Sender");
    CHECK(std::string(h.host_name) == "MockHost 9.9", "the Feed names the host");
    CHECK(h.slot_count == 0, "no ring has been allocated");
    CHECK(isIdentity(), "the Sender reports itself as an identity");
    f.fill(1);
    render(f, 10.0);  // a host that renders anyway
    CHECK(f.passedThrough(), "the picture passes through unchanged");
    h = peek(feed);
    CHECK(h.slot_count == 0 && h.frames_published == 0 && h.next_sequence == 0, "nothing was copied and no ring was made");
    CHECK(h.renders_idle == 1 && h.renders_total == 1, "the render was counted as idle");

    std::printf("an app attaches\n");
    scfeed_reader r;
    CHECK(scfeed_reader_open(&r, feed) == SCFEED_OK, "the reader opens the Feed");
    CHECK(!isIdentity(), "the Sender stops reporting identity");
    f.fill(2);
    render(f, 11.0, true);
    CHECK(f.passedThrough(), "a thumbnail render passes through");
    h = peek(feed);
    CHECK(h.renders_thumbnail == 1 && h.slot_count == 0 && h.next_sequence == 0, "a thumbnail render is ignored");
    f.fill(3);
    const double firstCall = render(f, 12.0);
    CHECK(f.passedThrough(), "the picture passes through unchanged while the ring is made");
    CHECK(firstCall < 5.0, "the render call that needs a ring does not build it (no cudaMalloc, no pinning on the render thread)");
    CHECK(waitFor([&] { return scfeed_load32(&r.hdr->retired) == 1; }), "the first frame makes the ring: the old Feed file is marked retired");
    CHECK(reopen(r, feed), "the reader re-attaches to the new Feed");
    CHECK(r.hdr->slot_count == 4 && r.hdr->slot_capacity >= 64 * 36 * 16, "the new Feed has a ring that fits the frame");
    waitFor([&] { return r.hdr->renders_total == 3; });  // the counters are carried over when the new Feed becomes current
    CHECK(r.hdr->renders_total == 3 && r.hdr->renders_idle == 1 && r.hdr->renders_thumbnail == 1, "the counters carry over to the new Feed");
    {
        const int e = scfeed_registry_find(r.hdr, (uint64_t)getpid(), 1);
        CHECK(e >= 0, "the instance is listed in the registry, and the listing carries over to the new Feed");
        CHECK(e >= 0 && r.hdr->registry[e].frame_rate == 24.0 && std::string(r.hdr->registry[e].host_name) == "MockHost 9.9", "with its timeline rate and host");
        CHECK(e >= 0 && r.hdr->registry[e].renders == 2 && r.hdr->registry[e].last_frame_time == 12.0, "every render but the thumbnail is recorded against it");
    }

    std::printf("frames reach the Feed bit for bit\n");
    bool exact = true, meta = true, through = true;
    for (int k = 0; k < 3; ++k) {
        f.fill(10 + (unsigned)k);
        render(f, 100.0 + k);
        through &= f.passedThrough();
        const int i = nextFrame(r);
        if (i < 0) { exact = meta = false; break; }
        scfeed_slot* s = scfeed_slot_at(r.hdr, (uint32_t)i);
        meta &= s->sequence == (uint64_t)k + 1 && s->width == 64 && s->height == 36 && s->row_bytes == 64 * 16 && s->row_order == SCFEED_ROWS_BOTTOM_UP &&
                s->pixel_format == SCFEED_PIXEL_RGBA32F && s->data_bytes == 64 * 36 * 16 && s->frame_time == 100.0 + k && s->frame_rate == 24.0 &&
                s->sender_instance == 1 && s->render_ns != 0 && s->publish_ns >= s->render_ns;
        exact &= std::memcmp(scfeed_slot_pixels(r.hdr, (uint32_t)i), f.host.data(), f.host.size() * 4) == 0;
        scfeed_reader_release(&r, i);
    }
    CHECK(exact, "three frames arrive with every value identical (negative, over-range, denormal included)");
    CHECK(meta, "each frame carries its size, format, frame number, frame rate, instance, sequence and times");
    CHECK(through, "the picture still passes through unchanged");

    std::printf("a Colour Setup set by hand on the Sender node travels with each frame\n");
    CHECK(gParams["colourSetupManual"].type == kOfxParamTypeBoolean, "a 'Set Colour Setup manually' switch is defined");
    CHECK(gParams["colourSetupManual"].props.m[kOfxParamPropDefault].size() == 1 && gParams["colourSetupManual"].props.m[kOfxParamPropDefault][0].i == 0, "it is off by default: the project's Colour Setup is used");
    CHECK(gParams["timelineColourSpace"].type == kOfxParamTypeChoice && choiceIndex("timelineColourSpace", "DaVinci WG/Intermediate") >= 0 &&
              choiceIndex("timelineColourSpace", "ACEScct") >= 0,
          "a Timeline Colour Space choice is defined, with Resolve's names");
    CHECK(gParams["outputColourSpace"].type == kOfxParamTypeChoice && choiceIndex("outputColourSpace", "P3-D65") >= 0 &&
              choiceIndex("outputColourSpace", "Rec.709 Gamma 2.4") >= 0,
          "an Output Colour Space choice is defined");
    CHECK(choiceIndex("levels", "Video") >= 0 && choiceIndex("levels", "Full") >= 0, "a Levels choice is defined");
    CHECK(choiceIndex("acesVersion", "ACES 2.0") == 0 && choiceIndex("acesVersion", "ACES 1.3") >= 0, "an ACES version choice is defined, 2.0 first");
    const char* const manualControls[] = {"timelineColourSpace", "outputColourSpace", "levels", "acesVersion"};
    auto controlsEnabled = [&](bool want) {
        bool ok = true;
        for (const char* c : manualControls) ok &= enabled(c) == want;
        return ok;
    };
    CHECK(controlsEnabled(false), "switched off: the manual controls are greyed out from the start");
    CHECK(userSets("colourSetupManual", 1) == kOfxStatOK && controlsEnabled(true), "switched on: the manual controls can be edited");
    CHECK(userSets("colourSetupManual", 0) == kOfxStatOK && controlsEnabled(false), "switched off again: greyed out again");
    {
        auto valuesAre = [&](const char* tl, const char* out, const char* lev, const char* aces) {
            return gParams["timelineColourSpace"].value == choiceIndex("timelineColourSpace", tl) &&
                   gParams["outputColourSpace"].value == choiceIndex("outputColourSpace", out) && gParams["levels"].value == choiceIndex("levels", lev) &&
                   gParams["acesVersion"].value == choiceIndex("acesVersion", aces);
        };
        userSets("colourSetupManual", 1);
        userSets("timelineColourSpace", choiceIndex("timelineColourSpace", "ACEScct"));
        userSets("outputColourSpace", choiceIndex("outputColourSpace", "Rec.2100 ST2084"));
        userSets("levels", choiceIndex("levels", "Full"));
        userSets("acesVersion", choiceIndex("acesVersion", "ACES 1.3"));
        userSets("colourSetupManual", 0);
        CHECK(valuesAre("ACEScct", "Rec.2100 ST2084", "Full", "ACES 1.3"), "switched off: the values set by hand are kept, only greyed out");
        userSets("colourSetupManual", 1);
        CHECK(valuesAre("ACEScct", "Rec.2100 ST2084", "Full", "ACES 1.3") && controlsEnabled(true), "switched on again: the same values come back, not defaults");
        // The project is closed and opened again: the host restores the saved values, then creates the instance.
        gPlugin->mainEntry(kOfxActionDestroyInstance, gEffect, nullptr, nullptr);
        for (const char* c : manualControls) gParams[c].props.m.erase(kOfxParamPropEnabled);
        CHECK(gPlugin->mainEntry(kOfxActionCreateInstance, gEffect, nullptr, nullptr) == kOfxStatOK, "the project is opened again");
        CHECK(valuesAre("ACEScct", "Rec.2100 ST2084", "Full", "ACES 1.3") && controlsEnabled(true), "the saved values and switch are left as saved, editable");
        userSets("colourSetupManual", 0);
        gPlugin->mainEntry(kOfxActionDestroyInstance, gEffect, nullptr, nullptr);
        for (const char* c : manualControls) gParams[c].props.m.erase(kOfxParamPropEnabled);
        gPlugin->mainEntry(kOfxActionCreateInstance, gEffect, nullptr, nullptr);
        CHECK(valuesAre("ACEScct", "Rec.2100 ST2084", "Full", "ACES 1.3") && controlsEnabled(false), "saved with the switch off: values kept, greyed out from the start");
        for (const char* c : {"timelineColourSpace", "outputColourSpace", "levels", "acesVersion"}) gParams[c].value = 0;
    }
    auto frameColour = [&](double t, uint32_t* provenance) {
        f.fill(30);
        render(f, t);
        const int k = nextFrame(r);
        std::string text;
        *provenance = 99;
        if (k >= 0) {
            const scfeed_slot* s = scfeed_slot_at(r.hdr, (uint32_t)k);
            *provenance = s->colour_setup_provenance;
            text.assign((const char*)s->colour_setup, s->colour_setup_bytes);
            scfeed_reader_release(&r, k);
        }
        return text;
    };
    uint32_t prov = 0;
    std::string text = frameColour(150.0, &prov);
    CHECK(prov == SCFEED_PROVENANCE_NONE && text.empty(), "switched off: the frame carries no Colour Setup of the Sender's own");
    gParams["colourSetupManual"].value = 1;
    gParams["timelineColourSpace"].value = choiceIndex("timelineColourSpace", "ACEScct");
    gParams["outputColourSpace"].value = choiceIndex("outputColourSpace", "P3-D65");
    gParams["levels"].value = choiceIndex("levels", "Full");
    gParams["acesVersion"].value = choiceIndex("acesVersion", "ACES 1.3");
    text = frameColour(151.0, &prov);
    CHECK(prov == SCFEED_PROVENANCE_MANUAL, "switched on: the frame's Colour Setup is marked manual");
    CHECK(text == "timeline=ACEScct\noutput=P3-D65\nlevels=full\naces_version=1.3\n", "and says exactly what was set");
    gParams["timelineColourSpace"].value = choiceIndex("timelineColourSpace", "DaVinci WG/Intermediate");
    gParams["outputColourSpace"].value = choiceIndex("outputColourSpace", "Rec.709 Gamma 2.4");
    gParams["levels"].value = choiceIndex("levels", "Video");
    text = frameColour(152.0, &prov);
    CHECK(text == "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\nlevels=video\n", "a non-ACES timeline sends no ACES version");
    gParams["colourSetupManual"].value = 0;
    text = frameColour(153.0, &prov);
    CHECK(prov == SCFEED_PROVENANCE_NONE && text.empty(), "switched off again: back to none");

    std::printf("the Sender never waits for a slow app\n");
    const uint64_t pub0 = scfeed_load64(&r.hdr->frames_published), busy0 = scfeed_load64(&r.hdr->drops_busy), over0 = scfeed_load64(&r.hdr->drops_overwritten);
    double worst = 0;
    for (int k = 0; k < 60; ++k) {  // the app reads nothing
        const double ms = render(f, 200.0 + k);
        if (ms > worst) worst = ms;
        scfeed_reader_heartbeat(&r, scfeed_now_ns());
    }
    cudaStreamSynchronize(gHostStream);
    std::this_thread::sleep_for(std::chrono::milliseconds(300));
    const uint64_t pub = scfeed_load64(&r.hdr->frames_published) - pub0, busy = scfeed_load64(&r.hdr->drops_busy) - busy0, over = scfeed_load64(&r.hdr->drops_overwritten) - over0;
    std::printf("        60 renders: worst call %.3f ms, %llu copied, %llu overwritten unread, %llu skipped busy\n", worst, (unsigned long long)pub,
                (unsigned long long)over, (unsigned long long)busy);
    CHECK(worst < 5.0, "no render call took as long as 5 ms");
    CHECK(pub + busy == 60, "every frame was either copied or counted as skipped");
    CHECK(over >= pub - 4, "unread frames were overwritten and counted");
    int waiting = 0, i;
    double newest = 0;
    while ((i = scfeed_reader_claim(&r)) >= 0) {
        newest = scfeed_slot_at(r.hdr, (uint32_t)i)->frame_time;
        scfeed_reader_release(&r, i);
        ++waiting;
    }
    CHECK(waiting >= 1 && waiting <= 4 && newest >= 200.0, "the ring holds the newest frames");

    std::printf("a larger frame replaces the ring\n");
    GpuFrame big(128, 72);
    big.fill(50);
    const uint64_t busyBefore = scfeed_load64(&r.hdr->drops_busy);
    const double growCall = render(big, 300.0);
    CHECK(growCall < 5.0, "the render call that sees a larger frame returns at once: the new ring is built off the render thread");
    CHECK(waitFor([&] { return scfeed_load32(&r.hdr->retired) == 1; }), "the Feed is replaced");
    CHECK(big.passedThrough(), "the larger picture passes through while the ring is built");
    CHECK(scfeed_load64(&r.hdr->drops_busy) == busyBefore + 1, "the frame that could not be fed is counted as dropped (Sender busy)");
    CHECK(reopen(r, feed) && r.hdr->slot_capacity >= 128 * 72 * 16, "the new ring fits the larger frame");
    waitFor([&] { return scfeed_load64(&r.hdr->drops_busy) > busyBefore; });  // the new Feed is current once the counters have carried over
    big.fill(51);
    render(big, 301.0);
    i = nextFrame(r);
    CHECK(i >= 0 && scfeed_slot_at(r.hdr, (uint32_t)i)->width == 128 && std::memcmp(scfeed_slot_pixels(r.hdr, (uint32_t)i), big.host.data(), big.host.size() * 4) == 0,
          "the larger frame arrives bit for bit");
    if (i >= 0) scfeed_reader_release(&r, i);
    CHECK(big.passedThrough(), "the larger picture passes through unchanged");

    std::printf("a render that is not on CUDA is passed through, not fed\n");
    std::vector<float> cpuSrc((size_t)64 * 36 * 4, 0.25f), cpuDst(cpuSrc.size(), 0.0f);
    render(cpuSrc.data(), cpuDst.data(), 64, 36, 64 * 16, 400.0, false);
    CHECK(cpuSrc == cpuDst, "the picture passes through unchanged");
    CHECK(scfeed_load64(&r.hdr->renders_unsupported) == 1, "the render is counted as unsupported");
    CHECK(std::strstr(r.hdr->sender_error, "CUDA") != nullptr, "the Feed says why no frames are sent");
    big.fill(52);
    render(big, 401.0);
    i = nextFrame(r);
    CHECK(i >= 0 && r.hdr->sender_error[0] == 0, "the message clears when frames flow again");
    if (i >= 0) scfeed_reader_release(&r, i);

    std::printf("the GPU has no memory for a larger frame's staging buffer\n");
    {
        GpuFrame huge(512, 288);  // 2.4 MB a frame
        setenv("SCOPECREEP_SENDER_GPU_LIMIT_MB", "1", 1);
        huge.fill(60);
        const double call = render(huge, 450.0);
        CHECK(call < 5.0, "the render call returns at once");
        CHECK(huge.passedThrough(), "the picture passes through unchanged: Resolve is unaffected");
        CHECK(waitFor([&] { return std::strstr(r.hdr->sender_error, "Not enough GPU memory") != nullptr; }), "the Feed says the Sender is short of GPU memory");
        CHECK(std::strstr(r.hdr->sender_error, "needs about 3 MB") != nullptr, "and roughly how much it needs");
        CHECK(scfeed_load32(&r.hdr->retired) == 0, "the Feed it had stays in place");
        for (int k = 0; k < 5; ++k) render(huge, 451.0 + k);
        CHECK(huge.passedThrough() && scfeed_reader_claim(&r) < 0, "later frames pass through, unfed, and no ring is tried within the retry time");
        unsetenv("SCOPECREEP_SENDER_GPU_LIMIT_MB");  // memory frees up
        for (int k = 0; k < 51; ++k) {  // the app stays attached meanwhile
            scfeed_reader_heartbeat(&r, scfeed_now_ns());
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }
        huge.fill(61);
        render(huge, 460.0);
        CHECK(waitFor([&] { return scfeed_load32(&r.hdr->retired) == 1; }), "after a few seconds the Sender tries again and makes the ring");
        CHECK(reopen(r, feed) && r.hdr->sender_error[0] == 0, "the new Feed has no error");
        waitFor([&] { return scfeed_load64(&r.hdr->renders_total) > 0; });
        huge.fill(62);
        render(huge, 461.0);
        i = nextFrame(r);
        CHECK(i >= 0 && std::memcmp(scfeed_slot_pixels(r.hdr, (uint32_t)i), huge.host.data(), huge.host.size() * 4) == 0, "and frames flow again, bit for bit");
        if (i >= 0) scfeed_reader_release(&r, i);
    }

    std::printf("the app quits\n");
    scfeed_reader_close(&r);
    CHECK(isIdentity(), "the Sender reports identity again");
    h = peek(feed);
    const uint64_t publishedBefore = h.frames_published, idleBefore = h.renders_idle;
    big.fill(53);
    for (int k = 0; k < 5; ++k) render(big, 500.0 + k);
    cudaStreamSynchronize(gHostStream);
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    h = peek(feed);
    CHECK(h.frames_published == publishedBefore, "nothing is copied");
    CHECK(h.renders_idle == idleBefore + 5, "the renders are counted as idle");
    CHECK(big.passedThrough(), "the picture passes through unchanged");

    std::printf("the app is killed while holding a frame\n");
    CHECK(scfeed_reader_open(&r, feed) == SCFEED_OK, "an app attaches again");
    big.fill(54);
    render(big, 600.0);
    i = nextFrame(r);
    CHECK(i >= 0, "it is reading a frame");
    scfeed_store64(&r.hdr->reader_heartbeat_ns, scfeed_now_ns() - 2 * SCFEED_READER_TIMEOUT_NS);  // no more heartbeats, no clean exit
    CHECK(isIdentity(), "within a second the Sender is an identity again");
    worst = 0;
    for (int k = 0; k < 20; ++k) {
        const double ms = render(big, 601.0 + k);
        if (ms > worst) worst = ms;
    }
    CHECK(worst < 5.0 && big.passedThrough(), "renders carry on, quickly, with the picture unchanged");
    scfeed_reader r2;
    CHECK(scfeed_reader_open(&r2, feed) == SCFEED_OK, "a restarted app attaches");
    int got = 0;
    for (int k = 0; k < 8; ++k) {  // at playback pace, reading each frame: the ring cycles through all four slots
        big.fill(55 + (unsigned)k);
        render(big, 700.0 + k);
        i = nextFrame(r2, 500);
        if (i >= 0 && scfeed_slot_at(r2.hdr, (uint32_t)i)->frame_time == 700.0 + k) ++got;
        if (i >= 0) scfeed_reader_release(&r2, i);
    }
    CHECK(got == 8, "and gets every frame through every slot, including the one the killed app held");
    scfeed_reader_close(&r2);

    gPlugin->mainEntry(kOfxActionDestroyInstance, gEffect, nullptr, nullptr);
    unlink(feed);
    std::printf(gFails ? "\nSender test: %d FAILED\n" : "\nSender test: all passed\n", gFails);
    return gFails ? 1 : 0;
}
