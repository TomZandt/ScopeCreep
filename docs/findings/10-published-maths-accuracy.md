# #10 Stage 0: Published maths vs Resolve reference renders

> Copied from the GitHub issue comments on TomZandt/scopecreep#10 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-13)

Done on `integration/stage0-milestone1`: Rust crate `accuracy/` + harness command `dump-timeline-frames`.

Run: `cargo run --release --manifest-path accuracy/Cargo.toml` (~4 s). Compares the probe's Timeline-Colour-Space frame 1 of four clips (ramp, chart, out-of-range, natural; 24.9M channel values per setup) with Resolve's own float EXR render in the Output Colour Space (and the 16-bit TIFF). Resolve 21.2.0.6 Studio.

| Colour Setup | max err (float) | mean err (float) | max err (10-bit codes) | mean (10-bit) | verdict |
|---|---|---|---|---|---|
| yrgb-rec709g24 | 0 | 0 | 0 | 0 | exact |
| dwg-rec709 (DWG/DI -> Rec.709 G2.4) | 1.2e-5 | 3.7e-8 | 0.013 | 0.00004 | exact |
| dwg-p3d65 | 4.5e-7 | 2.9e-8 | 0.0005 | 0.00003 | exact |
| dwg-pq-tmoff (Rec.2100 PQ) | 9.4e-6 | 1.9e-6 | 0.0096 | 0.002 | exact |
| dwg-pq-tmon (DaVinci tone mapping "on") | - | - | - | - | **untested** (see below) |
| acescct-rec709 (ACES 2.0 BT.1886) | 3.8e-3 | 1.4e-5 | 3.9 | 0.014 | exact (float32 noise at black) |
| acescct-p3d65 (ACES 2.0) | 4.7e-3 | 1.7e-5 | 4.8 | 0.017 | exact (float32 noise at black) |
| acescct-pq (ACES 2.0 1000 nit) | 1.4e-3 | 3.6e-6 | 1.45 | 0.004 | exact (float32 noise at black) |

**Tolerance for "exact"** (encoded 0..1 values, what the waveform plots): |ours - Resolve| <= 1e-4 (0.1 of a 10-bit code), or <= 5e-6 in display-linear light. 1e-4 is ~8x the worst error of the simple pipelines and far below the effect of a wrong gamma/matrix/constant; Resolve is float32 on GPU, ours f64, so bit-exact isn't testable. The linear-light rule exists because one 10-bit code near black under gamma 2.4 is below float32 resolution of the ACES maths; passes by this rule alone are labelled "float32 noise at black" with worst code error printed.

**Approximated / not covered**
- ACES 2.0: ours is a port of the Academy reference CTL (aces-core 069b0bc3, aces-output 6d8f9071). Only difference is in the darkest band (<=0.035 of the encoded range; 0.7-2.4% of samples exceed 1e-4, up to 3.9/4.8/1.45 codes); in light the error is <= 2.3e-6 of white. A systematic difference smaller than that inside the dark band can't be ruled out.
- **DaVinci tone mapping: untested.** `colorSpaceOutputToneMapping` reads back "DaVinci" but Resolve's render is identical to tone mapping off, for every value/projection tried; `colorSpaceOutputToneLuminanceMax` rejects anything but 100. It needs UI investigation. Gamut mapping "on" is also untested (API accepts only "None").
- ACES 1.3 not covered (version unreadable/unsettable by script; these projects are confirmed ACES 2.0 by the match).

**Resolve behaviour found by comparison:** float output isn't clamped (out-of-range/negative values carried through); gamma outputs are sign-preserving power laws; **"P3-D65" output is gamma 2.6, not 2.4**; "Rec.709 Gamma 2.4" is a pure power law, not the BT.709 OETF; PQ negatives encode as 0; DWG scene-linear 1.0 = 100 nit; no extra tone/gamut handling when mapping is None; ACES ODT names in Resolve equal the CTL `ACESuserName`s.

**Sources:** Blackmagic "DaVinci Resolve 17 Wide Gamut Intermediate" v1.1; ITU-R BT.709-6, BT.1886, BT.2020-2, BT.2100-2; SMPTE ST 2084, ST 431-2, EG 432-1, ST 2065-1; Academy S-2016-001 (ACEScct), S-2014-004 (AP1); ACES 2.0 CTL above. Each is cited in the module that uses it.



## DaVinci tone mapping is the project's Output DRT (#21, checked 2026-11-20)

The old script key `colorSpaceOutputToneMapping` reads back "DaVinci" but changes nothing: Resolve 21.2's Color Management page has no tone-mapping dropdown any more; tone mapping is the **Output DRT** dropdown (None, Simple, Luminance Mapping, DaVinci, Saturation Preserving), whose script key is `outputDRT` (settable, read-back confirmed; the matching `inputDRT` too). That is why every earlier script-set "tone mapping on" render equalled "off".

Measured on `scopecreep-test-dwg-pq-tmon` (DWG/DI timeline, Rec.2100 PQ output), frame 1 of each test clip, float EXR renders, app conversion (no tone mapping) against Resolve, absolute difference on the PQ-encoded scale:

| Clip | vs Output DRT None: max / mean | vs Output DRT DaVinci: max / mean |
|---|---|---|
| ramp | 7.3e-6 / 3.8e-7 | 0.100 / 0.0151 |
| chart | 2.1e-6 / 3.1e-7 | 0.066 / 0.0096 |
| outofrange | 6.3e-6 / 1.6e-6 | 0.079 / 0.0114 |
| natural | 9.4e-6 / 3.5e-7 | 0.093 / 0.0248 |

So the app is exact when the Output DRT is None and about 0.07 to 0.10 of the PQ range away at worst (1.2 to 2.5 % mean) with DaVinci. The app reads only `colorSpaceOutputToneMapping`, not `outputDRT`, so a project with Output DRT on is labelled exact (a false claim, ADR 0007): see the issue filed from this check. The other three DRTs were set by script and accepted but not measured. Colour-managed `colorSpaceOutputGamutMapping` still accepts only "None" by script (14 values tried); the UI's "Limit output gamut to" is a different dropdown whose key is not yet identified (#22).
