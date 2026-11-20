# 0037: The ACES 2.0 conversion pays about 1.4 ms of GPU time to stay inside the exact tolerance

Date: 2026-11-20 (#44). See ADR 0007, ADR 0003.

## Decision

The ACES 2.0 path of Output Conversion (`output_conversion.wgsl`) is computed with more care than plain 32-bit floats allow, so that every colour of the dense test grids, dark saturated ones included, stays inside the exact tolerance in Rec.2100 PQ, P3-D65 and Rec.709, and the "exact" label for ACES 2.0 stays true (ADR 0007).

- The CAM16 cone responses, in and out, are carried as a pair of 32-bit floats (about 46 bits), with one Newton step for the 0.42 power and its inverse.
- The hue's cosine and sine come straight from a/M and b/M, not through the hue in degrees (which keeps only about 1.5e-5 of a degree near 200).
- The error-correcting steps (two-sum, the compensated dot product) are wrapped in `opaque()`: each value they rely on is multiplied by a 1 read from the parameters (`encode.w`, which was unused). The GPU compiler otherwise rewrites `(a + b) - a` to `b`, so every two-sum returned 0, and feeds `fma` an unrounded product.

## Why

The alternative was to loosen the label or the tolerance for dark saturated colours. A colourist grading near black reads those pixels; a label that claims exactness has to hold there. The extra precision costs GPU time instead of accuracy.

## Consequences

- Converting one UHD frame through ACES takes 3.29 ms instead of 1.92 ms on the RTX 3070 Ti (about 7 % of a 50 fps frame); DWG conversion is unchanged at 0.57 ms. The cost sits inside the budget measured in #82 and is shared by every Panel.
- The `opaque()` trick depends on the compiler not seeing that the parameter is 1. A driver that behaves differently is caught by the dense-grid tests in `output_conversion_seam.rs`, not by reading the shader; the behaviour was measured only on NVIDIA Linux driver 615.
- Anyone simplifying the shader must keep the 1 in `encode.w` and the grid tests.
