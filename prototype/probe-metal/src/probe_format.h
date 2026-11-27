// PROTOTYPE (throwaway): pure helpers for the Metal probe (issue #253). No Metal, no OFX: testable on any OS.
#pragma once
#include <algorithm>
#include <cstddef>
#include <cstdio>
#include <string>
#include <vector>

namespace pm {

// MTLStorageMode raw values.
inline std::string storageModeName(unsigned long mode)
{
    switch (mode) {
    case 0: return "shared";
    case 1: return "managed";
    case 2: return "private";
    case 3: return "memoryless";
    }
    return "unknown(" + std::to_string(mode) + ")";
}

inline size_t roundUpToPage(size_t bytes, size_t page) { return (bytes + page - 1) / page * page; }

// Decimal GB per second.
inline double gbPerSec(size_t bytes, double ms) { return ms > 0 ? (double)bytes / (ms / 1000.0) / 1e9 : 0.0; }

// Timing samples in ms with a one-line summary.
class Samples {
public:
    void add(double ms) { v_.push_back(ms); }
    size_t count() const { return v_.size(); }
    std::string summary(const char* name) const
    {
        if (v_.empty()) return std::string(name) + " n=0";
        std::vector<double> s = v_;
        std::sort(s.begin(), s.end());
        double sum = 0;
        for (double x : s) sum += x;
        const size_t n = s.size();
        const double median = n % 2 ? s[n / 2] : (s[n / 2 - 1] + s[n / 2]) / 2;
        char b[200];
        std::snprintf(b, sizeof b, " n=%zu mean=%.3f min=%.3f max=%.3f median=%.3f ms", n, sum / n, s.front(), s.back(), median);
        return std::string(name) + b;
    }

private:
    std::vector<double> v_;
};

}  // namespace pm
