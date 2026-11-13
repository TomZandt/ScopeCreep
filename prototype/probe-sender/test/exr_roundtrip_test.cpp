// PROTOTYPE (throwaway). Writes awkward float values through the dump path,
// reads them back with OpenEXR, and demands identical bit patterns.
#include "../src/exr_dump.h"

#include <OpenEXR/ImfChannelList.h>
#include <OpenEXR/ImfFrameBuffer.h>
#include <OpenEXR/ImfInputFile.h>

#include <cmath>
#include <cstdio>
#include <cstring>
#include <limits>
#include <vector>

int main()
{
    const int w = 37, h = 19, rowFloats = w * 4 + 8;  // padded rows, like real hosts
    std::vector<float> src((size_t)rowFloats * h, 0.f);
    const float specials[] = {0.f, -0.f, 1.f, -1.f, 65504.f, 65520.f, 1e-45f, 1e-38f,
                              3.4028235e38f, -3.4028235e38f, 0.1f, 1.0000001f,
                              std::numeric_limits<float>::infinity(),
                              -std::numeric_limits<float>::infinity(),
                              std::numeric_limits<float>::quiet_NaN()};
    const int ns = sizeof(specials) / sizeof(float);
    unsigned s = 12345;
    for (int y = 0; y < h; ++y)
        for (int i = 0; i < w * 4; ++i) {
            s = s * 1664525u + 1013904223u;
            float v;
            if ((s >> 24) % 5 == 0) v = specials[(s >> 8) % ns];
            else { unsigned bits = s; std::memcpy(&v, &bits, 4); if (std::isnan(v)) v = 0.5f; }
            src[(size_t)y * rowFloats + i] = v;
        }

    const char* path = "/tmp/scopecreep_exr_roundtrip.exr";
    std::string err = writeExrFloatRgba(path, src.data(), w, h, rowFloats);
    if (!err.empty()) { std::printf("FAIL write: %s\n", err.c_str()); return 1; }

    Imf::InputFile in(path);
    Imath::Box2i dw = in.header().dataWindow();
    if (dw.max.x - dw.min.x + 1 != w || dw.max.y - dw.min.y + 1 != h) { std::printf("FAIL size\n"); return 1; }
    std::vector<float> back((size_t)w * h * 4);
    Imf::FrameBuffer fb;
    const char* names[4] = {"R", "G", "B", "A"};
    for (int c = 0; c < 4; ++c) {
        if (in.header().channels().findChannel(names[c])->type != Imf::FLOAT) { std::printf("FAIL channel type\n"); return 1; }
        fb.insert(names[c], Imf::Slice(Imf::FLOAT, (char*)(back.data() + c), 16, 16 * (size_t)w));
    }
    in.setFrameBuffer(fb);
    in.readPixels(dw.min.y, dw.max.y);

    long bad = 0;
    for (int y = 0; y < h; ++y)  // file row y == OFX row h-1-y
        for (int i = 0; i < w * 4; ++i)
            if (std::memcmp(&back[(size_t)y * w * 4 + i], &src[(size_t)(h - 1 - y) * rowFloats + i], 4) != 0) ++bad;
    if (bad) { std::printf("FAIL: %ld mismatching floats\n", bad); return 1; }
    std::printf("PASS: %d x %d float RGBA bit-exact (incl. NaN/inf/denormal/-0)\n", w, h);
    return 0;
}
