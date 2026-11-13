// PROTOTYPE (throwaway): Stage 0 probe sender, issue #4.
#include "exr_dump.h"

#include <OpenEXR/ImfChannelList.h>
#include <OpenEXR/ImfFrameBuffer.h>
#include <OpenEXR/ImfHeader.h>
#include <OpenEXR/ImfOutputFile.h>

#include <cstdio>
#include <vector>

std::string writeExrFloatRgba(const std::string& path, const float* rgba,
                              int width, int height, int rowFloats)
{
    try {
        std::vector<float> flipped((size_t)width * height * 4);
        for (int y = 0; y < height; ++y) {
            const float* src = rgba + (size_t)(height - 1 - y) * rowFloats;
            float* dst = flipped.data() + (size_t)y * width * 4;
            for (int i = 0; i < width * 4; ++i) dst[i] = src[i];
        }

        Imf::Header header(width, height);
        header.compression() = Imf::NO_COMPRESSION;
        const char* names[4] = {"R", "G", "B", "A"};
        for (const char* n : names) header.channels().insert(n, Imf::Channel(Imf::FLOAT));

        Imf::FrameBuffer fb;
        for (int c = 0; c < 4; ++c) {
            fb.insert(names[c], Imf::Slice(Imf::FLOAT, (char*)(flipped.data() + c),
                                           sizeof(float) * 4, sizeof(float) * 4 * width));
        }
        const std::string tmp = path + ".tmp";
        {
            Imf::OutputFile out(tmp.c_str(), header);
            out.setFrameBuffer(fb);
            out.writePixels(height);
        }
        if (std::rename(tmp.c_str(), path.c_str()) != 0) return "rename failed";
        return "";
    } catch (const std::exception& e) {
        return e.what();
    }
}
