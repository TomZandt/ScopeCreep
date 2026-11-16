# #17 Output Conversion on the GPU: accuracy against Resolve

What ticket #17 measured while building the Output Conversion. DaVinci Resolve 21.2.0.6 Studio on Linux, RTX 3070 Ti, with the real Sender (probe-alias dev build) on every `scopecreep-test-*` timeline node. Every Resolve step ran under the shared `~/scopecreep-testdata/resolve.lock`.

## Method

- The app converts the Feed (Timeline Colour Space) to the Output Colour Space in a WGSL compute pass (`app/src/output_conversion.wgsl`), in 32-bit float. Every matrix, constant and ACES 2.0 table comes from the accuracy crate's f64 reference (`accuracy/src/`), the code Stage 0 proved against Resolve (#10). The shader repeats only the per-pixel steps.
- **Accuracy suite:** `harness.py feed-frames` moves Resolve's playhead to frame 1 of each test clip; the capture tool (`app/examples/capture_converted.rs`) attaches to the real Feed, reads the Colour Setup by script and converts on the GPU exactly as the app does, and saves the converted frame with the app's label. `cargo run --release -p scopecreep-accuracy -- feed` compares them with Resolve's float EXR renders (24.9M channel values per setup), using the #10 tolerance (1e-4 encoded, or 5e-6 of white in display light), and checks the label never claims more than was measured.

## Accuracy suite: the real Feed after Output Conversion vs Resolve

| Colour Setup | max err (float) | max err (10-bit codes) | over 1e-4 | measured | app label | label holds |
|---|---|---|---|---|---|---|
| yrgb-rec709g24 | 0 | 0 | 0 % | exact (bit-identical) | exact | yes |
| dwg-rec709 | 1.5e-5 * | 0.015 * | 0 % * | exact * | exact | yes * |
| dwg-p3d65 | 3.6e-7 | 0.0004 | 0 % | exact | exact | yes |
| dwg-pq-tmoff | 9.4e-6 | 0.0096 | 0 % | exact | exact | yes |
| dwg-pq-tmon | 9.4e-6 | 0.0096 | 0 % | exact | approximated: DaVinci tone mapping not applied (unpublished) | yes (claims less) |
| acescct-rec709 | 3.8e-3 | 3.91 | 2.37 % | exact (float32 noise at black) | exact for ACES 2.0 | yes |
| acescct-p3d65 | 4.7e-3 | 4.78 | 2.35 % | exact (float32 noise at black) | exact for ACES 2.0 | yes |
| acescct-pq | 1.4e-3 | 1.45 | 0.70 % | exact (float32 noise at black) | exact for ACES 2.0 | yes |

The ACES samples over 1e-4 all sit in the darkest band (reference at most 0.037 / 0.041 / 0.029 encoded) and are within 2.5e-6 of white in light. The numbers are the same as the f64 maths against Resolve in #10: the app's 32-bit GPU conversion matches Resolve as closely as the f64 reference does.

\* **dwg-rec709** was first measured against renders out of step with its project (max 4.78, every sample, and the suite said so: "the label claims more than was measured"). Its four clips had picked up a clip grade (Primary Balance and Saturation/Hue/Lum Mix) after the Stage 0 renders; rendered fresh from the graded project the same captures measured exact (max 3.1e-5). #43 removed the grade, re-rendered the references and re-captured the Feed; the row above is that measurement (2026-11-16), and the published maths against the new renders gives the Stage 0 numbers again (max 1.2e-5, mean 3.7e-8, #10).

## Test data must match its project (#43)

- **A clip grade can be removed by script:** `Graph.ResetAllGrades()` on a timeline item's `GetNodeGraph()` returns True and leaves the clip as Resolve creates it (one node, no tools, as in the Stage 0 dumps). The timeline graph (the Sender) is untouched. `harness.py reset-grades --only <key>` does this and verifies by reading every clip graph back.
- **The state a project's test data came from is recorded next to it:** `render` writes `project-state.txt` into the renders directory and `feed-frames` into feed-converted: the Colour Setup keys, the timeline graph (any Sender build reads as "Sender") and each clip's grade, one line each.
- **Drift fails loudly:** `harness.py check-state` compares each live project with the state recorded with its renders and exits non-zero naming every changed line and every graded clip; `feed-frames` refuses to capture from a project that no longer matches its references; `accuracy/tests/resolve_renders.rs` fails when references were rendered from a graded clip or the Feed captures came from a different state than the references.
- Only dwg-rec709 has a recorded state so far; references rendered before #43 are named on stderr by the test and are not checked (#48).

## Synthetic frames at the Feed seam (`app/tests/output_conversion_seam.rs`)

Grey ramps from below 0 to above 1 plus every mix of 11 levels per channel, converted on the GPU and compared with the f64 reference within the same tolerance: DWG/DI to Rec.709 Gamma 2.4, P3-D65 (gamma 2.6), Rec.2100 PQ and Rec.2020 Gamma 2.4; ACEScct through ACES 2.0 Rec.709 BT.1886, P3-D65 and Rec.2100 ST2084 (1000 nit); YRGB unchanged. All pass. The waveform's bins are checked on the converted picture, at video levels (#18): DWG/DI 18 % grey at code 493 and 90 % at 902 (as sent they would be 358 and 497); ACES 2.0 18 % grey at code 400 (10.013 nit).

On a denser grid (31 levels per channel, 29,791 colours) the ACES 2.0 conversion stays within tolerance except **2 channel values out of about 90,000 in Rec.2100 PQ (1000 nit), worst 4.2e-4 (0.43 of a 10-bit code)**, both saturated colours near black (encoded about 0.05). P3-D65 passes there only by the light rule (worst 6.7e-4 encoded, dark); Rec.709 worst 4.4e-5.

## What 32-bit float needed (ACES 2.0)

A plain port lost up to 1.7e-4 on bright saturated colours. The CAM16 steps in ACES 2.0 raise values in the hundreds to the power 0.42 and then subtract nearly equal results (the opponent a/b channels), so small rounding becomes large. Three changes, measured stage by stage against the f64 reference, brought it inside tolerance on the standard grid:

- **pow:** the GPU's `pow` is `exp2(e * log2(x))`, and rounding `e * log2(x)` loses bits when `log2(x)` is large. The shader splits `x = m * 2^k`, recovers the rounding of `e * k` with `fma`, and passes only the small remainder to `exp2`. About twice as accurate.
- **Matrices:** each matrix is sent as 32-bit floats plus what they lose, and multiplied as a compensated dot product (`fma` and two-sum).
- **ACEScct decode:** `17.52` and `9.72` are not exact in 32-bit float; their lost digits are added back.

The remaining error comes from the 32-bit cone responses themselves (one unit in the last place times the Aab matrix's 15,000 scale): it cannot be removed without carrying them at higher precision. Range-reducing the hue before `sin`/`cos` and a cancellation-free form of the toe made no measurable difference on these tests; both are kept, being the same maths.

## Labels

| Colour Setup | Conversion | Label |
|---|---|---|
| YRGB, Rec.709 Gamma 2.4 → Rec.709 Gamma 2.4 | none needed | exact |
| YRGB, other colour spaces | none (YRGB converts nothing) | approximated: not yet measured |
| DWG/DI → Rec.709 Gamma 2.4, P3-D65, Rec.2100 ST2084 | published maths | exact |
| DWG/DI → Rec.2020 Gamma 2.4 | published maths | approximated: not yet measured against Resolve |
| DWG/DI with tone or gamut mapping on | published maths without the mapping | approximated: … not applied (unpublished) |
| ACEScct, ACES 2.0 → Rec.709 BT.1886, P3-D65, Rec.2100 ST2084 (1000 nit) | ACES 2.0 reference CTL port | exact (from the project: "exact for ACES 2.0", the version being assumed) |
| DWG/DI → Rec.2100 HLG, P3-DCI, sRGB; other ACES output transforms; ACES 1.3; ACEScc; other timelines | none | "as sent, no Output Conversion: … not yet supported" |

When there is no conversion the waveform says "Rec.709 assumed" rather than naming the Output Colour Space. A Colour Setup read after playback stops converts the frame already on screen.

## Not verified

- Rec.2020 Gamma 2.4, HLG, P3-DCI, sRGB and the other ACES output transforms against Resolve (no test projects); ACES 1.3 (#23), tone mapping (#21), gamut mapping (#22).
- GPU cost of the conversion at UHD 60 fps with Resolve playing.
- The suite needs the reference renders to match the projects' current state; checked for dwg-rec709 only until the other projects' states are recorded (#48).
