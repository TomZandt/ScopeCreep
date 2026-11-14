// PROTOTYPE (throwaway): Stage 0 probe sender, issue #4.
#pragma once
#include <string>

// Writes a 32-bit float RGBA EXR, uncompressed, values bit-exact.
// `rgba` is OFX layout: bottom row first, 4 floats per pixel, `rowFloats`
// floats between row starts (may differ from width*4). The file is written
// top row first (EXR convention), so it looks upright in a viewer.
// Returns empty string on success, otherwise an error message.
std::string writeExrFloatRgba(const std::string& path, const float* rgba,
                              int width, int height, int rowFloats);
