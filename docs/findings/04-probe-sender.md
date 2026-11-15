# #4 Stage 0: Probe Sender loads in Resolve and reports what it sees

> Copied from the GitHub issue comments on TomZandt/scopecreep#4 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-13)

Done on branch `integration/stage0-milestone1` (prototype/probe-sender/).

**Toolchains:** CUDA 13.4.92 (ticket said 12.8; driver 615.78 supports 13.4, `make cuda-check` passes), Rust 1.99.1, g++ 16.2.1, GNU Make 4.4.1, cmake 4.4.4. Resolve 21.2.0.6 Studio.

**Build/install:** `make` / `make install` (to /usr/OFX/Plugins). `make test` runs offline tests (EXR round-trip bit-exact incl. NaN/inf/denormal/-0; plugin vs fake host).

**Verified in Resolve:** probe listed as `Open FX/scopecreep/scopecreep Probe Sender` (category `ofx`) via scripting; applied on the timeline node it passes the picture through and logs every field in the ticket; frame dump to float EXR works (1920x1080, uncompressed 32-bit float, valid per exrheader).

**Answers**
- Per-frame colour space via OFX colour management: **No.** Host advertises style `Full` (config `ofx-native-v1.5_aces-v1.3_ocio-v2.3`) but source clip colourspace reads `Raw` and output `OfxColourspace_Source`, with no OCIO config/display values. Same with the probe declaring OCIO style and Full style, in an ACEScct (ACES 2.0) project and a DaVinci WG project, with "Enable OpenFX Color Management" ticked.
- Host name distinguishes Studio from free: **No.** name=`DaVinciResolve`, label `DaVinci Resolve`, version 21,2,0,6. Use scripting `IsStudio()`.
- Resolve page: **not reported** (page property unset in all renders).
- Thumbnails: flagged correctly (`thumbnail=1`, small frames e.g. 92x46).

**Finding worth keeping:** linking the system OpenEXR into an OFX plugin **crashes Resolve** (clashes with Resolve's bundled OpenEXR). The probe now uses a dependency-free EXR writer. The real Sender must not link OpenEXR/Imath either.

**Not done:** frame dump bit-exactness vs a Resolve reference render is checked later (#9/#10).


