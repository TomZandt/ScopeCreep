// Offline test (runs on any OS) for the pure helpers in src/probe_format.h.
#include "../src/probe_format.h"
#include <cstdio>
#include <cstdlib>
#include <string>

static int fails = 0;
#define EXPECT_EQ(a, b) do { auto _a = (a); auto _b = (b); if (!(_a == _b)) { \
    std::fprintf(stderr, "FAIL line %d: %s != %s\n", __LINE__, #a, #b); ++fails; } } while (0)

int main()
{
    // MTLStorageMode raw values: Shared 0, Managed 1, Private 2, Memoryless 3.
    EXPECT_EQ(pm::storageModeName(0), std::string("shared"));
    EXPECT_EQ(pm::storageModeName(1), std::string("managed"));
    EXPECT_EQ(pm::storageModeName(2), std::string("private"));
    EXPECT_EQ(pm::storageModeName(3), std::string("memoryless"));
    EXPECT_EQ(pm::storageModeName(9), std::string("unknown(9)"));

    // Metal wants the length of a no-copy buffer to be a whole number of pages (16 KiB on Apple Silicon).
    EXPECT_EQ(pm::roundUpToPage(1, 16384), (size_t)16384);
    EXPECT_EQ(pm::roundUpToPage(16384, 16384), (size_t)16384);
    EXPECT_EQ(pm::roundUpToPage(16385, 16384), (size_t)32768);
    EXPECT_EQ(pm::roundUpToPage(3840ull * 2160 * 16, 16384), (size_t)132710400);  // 8100 pages exactly

    // 132710400 bytes in 10 ms = 13.27104 GB/s (decimal GB).
    EXPECT_EQ(pm::gbPerSec(132710400, 10.0), 13.27104);
    EXPECT_EQ(pm::gbPerSec(100, 0.0), 0.0);

    pm::Samples s;
    EXPECT_EQ(s.summary("blit"), std::string("blit n=0"));
    s.add(3.0); s.add(1.0); s.add(2.0);
    EXPECT_EQ(s.summary("blit"), std::string("blit n=3 mean=2.000 min=1.000 max=3.000 median=2.000 ms"));
    s.add(10.0);  // even count: median is the mean of the middle two
    EXPECT_EQ(s.summary("blit"), std::string("blit n=4 mean=4.000 min=1.000 max=10.000 median=2.500 ms"));

    if (fails) return 1;
    std::puts("format_test: all passed");
    return 0;
}
