# Issue #8: which luma weights does Resolve's waveform use?

Resolve 21.2.0.6 Studio, Color page, Scopes panel = Waveform, mode **Y**. During measurement: **Low Pass Filter OFF**, **Data Level Scopes ticked** (not changed by me), Extents unticked, Colorize ticked, scale 10-bit.

## Answer
Resolve's Y waveform applies fixed standard weights chosen by the Output Colour Space's primaries, to the **encoded (non-linear) output R'G'B'**:

| Output Colour Space | Weights Resolve uses | Primaries-derived (ADR 0004) |
|---|---|---|
| Rec.709 Gamma 2.4 (YRGB, and DWG colour-managed) | Rec.709: 0.2126 / 0.7152 / 0.0722 | same |
| P3-D65 (Resolve: gamma 2.6) | **Rec.709**: 0.2126 / 0.7152 / 0.0722 | 0.2290 / 0.6917 / 0.0793 (DIFFERS) |
| P3-DCI | **Rec.709** | differs likewise |
| Rec.2020 Gamma 2.4 | Rec.2020: 0.2627 / 0.6780 / 0.0593 | same |
| Rec.2100 ST2084 (PQ) | **Rec.2020** | same |
| Rec.2100 HLG | **Rec.2020** | same |

ADR 0004 agrees with Resolve for Rec.709 and the Rec.2020/Rec.2100 spaces, and **disagrees for P3** (Resolve uses Rec.709 weights, not P3 primaries-derived ones). Resolve is not "always Rec.709" either: Rec.2020/Rec.2100 outputs use Rec.2020 weights.

## Method (needed because the original chart does not read well)
In the original chart, each colour patch is a single-level line, and Resolve's scope draws such lines sparsely (the scope is 1023 levels drawn in about 358 px, so some single-level lines vanish or hide in graticule lines). Greys appeared but the colour patches did not. So I made `~/scopecreep-testdata/media/lumasweep` (1920x1080 float EXR, 24 frames): seven bands, each an encoded ramp 0 to 1 across x in pure R, G, B, cyan, magenta, yellow and white. In a Y waveform each band is a straight line whose right end (x = 1) is (the weighted sum) x 1023. Each test project tags the clip with an Input Colour Space equal to the Output Colour Space (DWG timeline), so the output encoded values equal the source values (checked: all lines straight, white ends at 1023). Projects: `scopecreep-test-luma-yrgb709`, `-luma-dwg709`, `-luma-dwgp3d65`, `-luma-dwgpq`, `-luma-dwg2020g24`, `-luma-dwgp3dci`, `-luma-dwghlg`. The hand-built dwg-p3d65 project was not touched.

Reading: least-squares line fit to the coloured trace pixels per band, converted to 10-bit codes with the 0 and 1023 graticule rows. The fit has a constant offset of about +2 codes (lines start at about 2 at x = 0); subtract it. **Uncertainty about +-3 codes** (0.35 px per code, fit residual 0.3 px for most lines). Red, yellow and white have larger residuals (lines overlap or touch graticule), so rely on green, blue, cyan and magenta.

## Measured end-of-ramp codes (raw; in brackets minus the +2 offset)

| band | Rec.709 out | P3-D65 out | P3-DCI out | Rec.2020 g2.4 | Rec.2100 PQ | Rec.2100 HLG | pred. Rec.709 w | pred. P3 w | pred. Rec.2020 w |
|---|---|---|---|---|---|---|---|---|---|
| red | 221.6 (220) | 221.5 (220) | 221.5 | 272.3 | 272.2 (270) | 272.0 | 217 | 234 | 269 |
| green | 734.4 (732) | 734.5 (732) | 734.4 | 696.3 | 696.4 (694) | 696.3 | 732 | 708 | 694 |
| blue | 76.5 (75) | 76.6 (75) | 76.8 | 63.2 | 64.5 (63) | 62.9 | 74 | 81 | 61 |
| cyan | 808.3 (806) | 808.4 (806) | 808.5 | 757.0 | 757.3 (755) | 756.9 | 806 | 789 | 754 |
| magenta | 294.1 (292) | 294.2 (292) | 293.8 | 332.0 | 333.3 (331) | 331.8 | 291 | 315 | 329 |
| yellow | 948.5 | 945.2 | 945.4 | 958.2 | 957.5 | 961.0 | 949 | 942 | 962 |

Green separates Rec.709 and P3 weights by 24 codes (8x the uncertainty) and red by 17, so P3 outputs match Rec.709 weights unambiguously; green, red and blue each exclude the P3-derived weights.

## Other findings
- Scope settings that change the luma calculation: none found. Y / CbCr / RGB are display modes; no weights option was reported in the settings panel or scope menu (as reported by the user; I could not open them myself).
- Out-of-range values: the Waveform draws the ramp clip's -0.25..1.25 band flattened at both ends, i.e. clipped to 0..1023 for display. Whether Y is computed before or after clipping was not isolated (my sweep stays in range). With the original chart in DWG projects (output RGB' like 1.25, -0.43, -0.21) I could not read the Y trace, because the single-level lines were not drawn.
- **Extents** (unticked): not tested, needs a click. **Video vs Data Level Scopes** (Data ticked): not tested, needs a click. All results are Data Level, 10-bit scale: 0 at 0 and white at 1023 on the graticule, no 64..940 mapping visible (relevant to #18).
- Measurements are on the encoded output signal, not linear light. PQ/HLG results mean Y' = 0.2627 R' + 0.6780 G' + 0.0593 B' on PQ/HLG-encoded values (non-constant luminance).
- Mode check: `resolve-20261113-233139.png` (natural clip, single colourized luma trace) confirms Y mode.

## Screenshots (in `~/scopecreep-testdata/screenshots/`)
`luma-luma-yrgb709.png`, `luma-luma-dwg709.png`, `luma-luma-dwgp3d65.png`, `luma-luma-dwgp3dci.png`, `luma-luma-dwg2020g24.png`, `luma-luma-dwgpq.png`, `luma-luma-dwghlg.png` (full window, waveform bottom right). Also `resolve-20261113-233401.png` (Rec.709 YRGB), `-233434.png` (P3-D65), `-233453.png` (PQ), `-233128.png` and `-233217.png` (original chart, sparse trace).
Scripts live in the scratchpad (not in the repo): `mk.py` (media), `build.py` (projects), `cap.py` (capture), `fit.py` (measurement).
