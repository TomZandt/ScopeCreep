# #44: ACES 2.0 dark saturated colours inside the exact tolerance in 32-bit float

What ticket #44 measured on 2026-11-20, at the Feed seam (`app/tests/output_conversion_seam.rs`) against the accuracy crate's f64 reference, on the RTX 3070 Ti. No Resolve: the reference is the same f64 port #10 proved against Resolve's renders.

## Before

#17 found a few dark saturated colours in ACES 2.0 → Rec.2100 ST2084 (1000 nit) just over the "exact" tolerance (1e-4 encoded, or 5e-6 of 100 nit in light) on a dense grid. Reproduced with every mix of 31 ACEScct levels per channel (29,791 colours) from 0 to 1.0, 1.1 and 1.2:

| Grid top | over tolerance | worst error (encoded) |
|---|---|---|
| 1.0 | 4 | 5.5e-4 (0.56 of a 10-bit code) |
| 1.1 | 6 | 3.8e-4 |
| 1.2 | 1 | 3.0e-4 |

All in the red channel of saturated cyans and greens whose red output sits near black (PQ about 0.05 to 0.12). P3-D65 and Rec.709 passed.

## Where the error came from

Measured by rounding one step of the f64 reference to 32-bit float at a time, then by reading the shader's intermediate values back:

- **The cone responses.** Rounding only the input cone responses (or their post-adaptation values) to 32 bits moved the worst colour by 2.0e-4 to 3.8e-4; rounding the output cone responses by 1.6e-4. The opponent a and b are small differences of cone responses about 15,000 times larger, and the gamut compression then makes the output red very sensitive to them. Holding just those four steps at higher precision brought the grids' rounding error to 8.3e-5.
- **The hue round trip.** The reference turns a and b into a hue in degrees, then back into cos and sin for the chroma norm and the output a and b. A hue near 200 degrees keeps only 1.5e-5 of a degree in 32 bits, and the worst colours moved by about 5e-5 per such step.
- **The GPU's compiler.** It simplifies algebra on error-free transformations: on the RTX 3070 Ti every two-sum (`s = a + b; (a - (s - (s - a))) + ...`) returned 0, because it reads `(a + b) - a` as `b`, and it hands `fma` an unrounded `x * (1 / y)` instead of the rounded quotient. So the compensated dot product of #17 had been recovering only the products' rounding (fma) and the matrices' lost digits, never the sums'.

## The fix (`app/src/output_conversion.wgsl`)

- **Opaque steps:** every rounded value an error-free step depends on is multiplied by a 1 the compiler cannot know (`encode.w` in the parameters, `opaque` in the shader). Two-sum, the products and the quotient then stay as written. Checked on its own: double-float x^0.42 and x^(1/0.42) on the GPU to 5e-12, division to 1e-14 (from 6e-8 without it).
- **Double-float cone responses:** the input cone responses, their post-adaptation values, the output cone responses after the Aab matrix and after the inverse post-adaptation are carried as hi + lo (about 46 bits). The power 0.42 = 21/50 is spow's 32-bit result plus one Newton step on y^50 = x^21 (and z^21 = w^50 for the inverse), the powers taken of the mantissas in double-float so nothing overflows. The matrices on either side take the low parts.
- **Hue direction from a and b:** cos and sin of the hue are a / M and b / M, the same numbers without the round trip through degrees. The hue in degrees is still used to look up the tables.

Both halves of the double-float are needed: either one alone left 1 to 5 colours a grid over tolerance.

## After

| Grid top | over tolerance | worst error (encoded) | worst fraction of the tolerance |
|---|---|---|---|
| 1.0 | 0 | 8.3e-5 | 0.59 |
| 1.1 | 0 | 1.3e-4 (dark, inside by light) | 0.82 |
| 1.2 | 0 | 5.7e-5 | 0.39 |

The fraction is the smaller of (error / 1e-4) and (light error / 5e-6), the larger of which a sample must keep under 1. P3-D65 and Rec.709 stay inside on the same grids. An earlier step of the fix (before the quotient was made opaque) also passed grids topped at 0.9, 1.05, 1.15 and 1.3. The three PQ grids, and P3-D65 and Rec.709 on them, are now tests (`..._matches_on_a_dense_grid`).

DaVinci Wide Gamut results move by less than one 32-bit unit (the sums of its matrix are now compensated too); its tests pass unchanged.

## Cost

GPU time of the conversion of one UHD frame (median of 40 runs of 10 dispatches, `OutputConversion::convert`, RTX 3070 Ti, no Resolve):

| | before | after |
|---|---|---|
| ACEScct → ACES 2.0 Rec.2100 PQ | 1.92 ms | 3.29 ms |
| DWG/DI → Rec.2100 PQ | 0.57 ms | 0.57 ms |

About 1.05 ms of the ACES increase is the double-float cone responses, the rest the opaque steps. It is about 7 % of a 50 fps frame.

## Labels

Unchanged: "exact for ACES 2.0" for Rec.709 BT.1886, P3-D65 and Rec.2100 ST2084 (1000 nit). The claim now holds on the dense grids as well as the standard grid and the real test frames.

## Not verified

- The real Feed captures against Resolve's renders (`scopecreep-accuracy feed`) were not re-run with this shader; the change only brings the shader closer to the f64 maths that #17 measured against Resolve.
- The opaque steps were measured on one GPU and driver (RTX 3070 Ti, NVIDIA on Linux). Another driver could simplify differently; the dense-grid tests would show it.
- Not verified against Resolve's renders since the change: #190.
