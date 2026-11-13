// PROTOTYPE (throwaway): Stage 0 probe sender, issue #4.
// Dependency-free EXR writer. The plugin must NOT link the system OpenEXR:
// Resolve bundles its own (older) OpenEXR in-process and the two clash,
// which crashed Resolve on the first frame dump.
// Format: OpenEXR 2 single-part scanline, NO_COMPRESSION, FLOAT A,B,G,R.
#include "exr_dump.h"

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

namespace {
void put(std::vector<char>& b, const void* p, size_t n) { b.insert(b.end(), (const char*)p, (const char*)p + n); }
void putI32(std::vector<char>& b, int32_t v) { put(b, &v, 4); }
void putStr(std::vector<char>& b, const char* s) { put(b, s, std::strlen(s) + 1); }
void attr(std::vector<char>& b, const char* name, const char* type, const std::vector<char>& v)
{
    putStr(b, name); putStr(b, type); putI32(b, (int32_t)v.size()); put(b, v.data(), v.size());
}
}

std::string writeExrFloatRgba(const std::string& path, const float* rgba,
                              int width, int height, int rowFloats)
{
    if (width <= 0 || height <= 0) return "bad size";
    std::vector<char> hdr;
    const char magic[8] = {0x76, 0x2f, 0x31, 0x01, 2, 0, 0, 0};
    put(hdr, magic, 8);

    std::vector<char> ch;
    for (const char* n : {"A", "B", "G", "R"}) {
        putStr(ch, n); putI32(ch, 2 /*FLOAT*/); putI32(ch, 0 /*pLinear+reserved*/);
        putI32(ch, 1); putI32(ch, 1);
    }
    ch.push_back(0);
    attr(hdr, "channels", "chlist", ch);

    attr(hdr, "compression", "compression", std::vector<char>(1, 0));
    std::vector<char> box;
    putI32(box, 0); putI32(box, 0); putI32(box, width - 1); putI32(box, height - 1);
    attr(hdr, "dataWindow", "box2i", box);
    attr(hdr, "displayWindow", "box2i", box);
    attr(hdr, "lineOrder", "lineOrder", std::vector<char>(1, 0));
    std::vector<char> f;
    float one = 1.f, zero = 0.f;
    put(f, &one, 4);
    attr(hdr, "pixelAspectRatio", "float", f);
    std::vector<char> v2; put(v2, &zero, 4); put(v2, &zero, 4);
    attr(hdr, "screenWindowCenter", "v2f", v2);
    attr(hdr, "screenWindowWidth", "float", f);
    hdr.push_back(0);

    const uint64_t lineBytes = (uint64_t)width * 16;
    const uint64_t blockBytes = 8 + lineBytes;
    const uint64_t firstBlock = hdr.size() + (uint64_t)height * 8;

    const std::string tmp = path + ".tmp";
    FILE* fp = std::fopen(tmp.c_str(), "wb");
    if (!fp) return "cannot open output";
    bool ok = std::fwrite(hdr.data(), 1, hdr.size(), fp) == hdr.size();
    for (int y = 0; y < height && ok; ++y) {
        uint64_t off = firstBlock + (uint64_t)y * blockBytes;
        ok = std::fwrite(&off, 8, 1, fp) == 1;
    }
    std::vector<float> line((size_t)width * 4);
    for (int y = 0; y < height && ok; ++y) {
        const float* s = rgba + (size_t)(height - 1 - y) * rowFloats;  // OFX is bottom-up
        for (int x = 0; x < width; ++x) {
            line[(size_t)0 * width + x] = s[x * 4 + 3];  // A
            line[(size_t)1 * width + x] = s[x * 4 + 2];  // B
            line[(size_t)2 * width + x] = s[x * 4 + 1];  // G
            line[(size_t)3 * width + x] = s[x * 4 + 0];  // R
        }
        int32_t yy = y, sz = (int32_t)lineBytes;
        ok = std::fwrite(&yy, 4, 1, fp) == 1 && std::fwrite(&sz, 4, 1, fp) == 1 &&
             std::fwrite(line.data(), 1, lineBytes, fp) == lineBytes;
    }
    if (std::fclose(fp) != 0) ok = false;
    if (!ok) return "write failed";
    if (std::rename(tmp.c_str(), path.c_str()) != 0) return "rename failed";
    return "";
}
