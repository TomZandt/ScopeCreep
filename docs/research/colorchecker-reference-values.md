# ColorChecker Classic reference values

Desk research for the milestone 2 tests, done on 2026-11-16. The values below were computed from published data, not measured with scopecreep or Resolve. Claims marked *inferred* were not stated by a source.

## Which data is authoritative

- **Use X-Rite's "ColorChecker24 – After November 2014" values.** They are L\*a\*b\* under D50 (2°), measured 2015-04-28 on an i1Pro 2 (M0).
  - X-Rite's CGATS file is mirrored on Zenodo: <https://zenodo.org/records/3245895>.
  - The same values ship in the `colour` library as `ColorChecker24 - After November 2014`; the two agree to within 7e-14.
  - *Inferred:* today's Calibrite ColorChecker Classic uses these values. The production date is printed on the back of each chart.
- **colour-nuke's rendered charts** (<https://github.com/colour-science/colour-nuke/tree/master/colour_nuke/resources/images>, BSD-3-Clause):
  - Half-float EXRs in about 54 colour spaces, made by `colour_rendition_chart.nk` with CAT02 adaptation.
  - The linear ColorChecker2014 files match the values below to half-float precision.
  - Their **ACEScct, ACEScc and ACESproxy files are linear ACEScg, not log-encoded**: the script applies matrices only.
  - **No DaVinci Wide Gamut or DaVinci Intermediate files exist.**
  - The ColorChecker2005 set is for the older chart edition.
- **AMPAS's aces-dev and related repos** hold no ColorChecker reference frames.

## How the values were computed

- **Adaptation:** Bradford, the ICC convention for D50 L\*a\*b\* data. D50 to D65 for Rec.709 and DaVinci Wide Gamut; D50 to the ACES white for AP0 and AP1. Using CAT02 instead moves Rec.709 luma by at most 0.33 % and changes no band.
- **ACEScct** (S-2016-001, <https://docs.acescentral.com/encodings/acescct/>): x ≤ 0.0078125 gives 10.5402377416545 x + 0.0729055341958355; otherwise (log2 x + 9.72) / 17.52. 18 % grey encodes to 0.4136.
- **DaVinci Intermediate** (Blackmagic, *DaVinci Resolve 17 Wide Gamut Intermediate*): x ≤ 0.00262409 gives 10.44426855 x; otherwise 0.07329248 (log2(x + 0.0075) + 7.0). 18 % grey encodes to 0.3360.
- **Rec.709 luma:** 0.2126 R′ + 0.7152 G′ + 0.0722 B′, full range, on values encoded with the BT.709 OETF. 18 % grey gives 40.90 %. A pure 2.4 inverse EOTF gives 48.94 % instead.
- **Hue:** atan2(Cr, Cb) in BT.709 Cb/Cr, with 0° at +Cb, measured counter-clockwise.
  - Rec.709 primaries and secondaries: R 102.9°, Yl 174.8°, G 229.7°, Cy 282.9°, B 354.8°, Mg 49.7°.
  - The traditional skin-tone I-line is 123° in the composite U/V plane, which rescales to 134.5° in BT.709 Cb/Cr.
- **Out of gamut:** only cyan (18) goes negative in Rec.709 (linear R −0.044). No patch is negative in DaVinci Wide Gamut, Rec.2020, ACEScg or ACES2065-1.

## The 24 patches at true reflectance

| # | Patch | Y | Stops from 18 % | ACEScct (R G B) | DaVinci Intermediate (R G B) | Rec.709 luma, BT.709 OETF | ARRI-style band | Hue in Rec.709 Cb/Cr |
|---|---|---|---|---|---|---|---|---|
| 1 | dark skin | 0.0971 | -0.89 | 0.3904 0.3519 0.3206 | 0.3027 0.2575 0.2365 | 27.89 % | none | 120.7° |
| 2 | light skin | 0.3327 | +0.89 | 0.4886 0.4546 0.4321 | 0.4253 0.3841 0.3680 | 56.17 % | none | 117.8° |
| 3 | blue sky | 0.1810 | +0.01 | 0.3950 0.4158 0.4581 | 0.3223 0.3500 0.3840 | 40.49 % | green | 322.3° |
| 4 | foliage | 0.1340 | -0.43 | 0.3791 0.3965 0.3270 | 0.2902 0.3032 0.2474 | 34.22 % | none | 197.6° |
| 5 | blue flower | 0.2310 | +0.36 | 0.4344 0.4287 0.4791 | 0.3676 0.3698 0.4107 | 46.65 % | none | 359.6° |
| 6 | bluish green | 0.4171 | +1.21 | 0.4446 0.4941 0.4825 | 0.3797 0.4327 0.4208 | 62.18 % | none | 272.5° |
| 7 | orange | 0.3053 | +0.76 | 0.5021 0.4366 0.3278 | 0.4389 0.3540 0.2859 | 50.49 % | none | 131.7° |
| 8 | purplish blue | 0.1128 | -0.67 | 0.3554 0.3669 0.4682 | 0.2865 0.3158 0.3922 | 29.84 % | none | 341.6° |
| 9 | moderate red | 0.1847 | +0.04 | 0.4743 0.3762 0.3802 | 0.4055 0.2980 0.3083 | 36.33 % | none | 96.0° |
| 10 | purple | 0.0632 | -1.51 | 0.3544 0.3047 0.3849 | 0.2677 0.2338 0.2934 | 20.85 % | none | 38.9° |
| 11 | yellow green | 0.4295 | +1.25 | 0.4740 0.4943 0.3639 | 0.4052 0.4197 0.3167 | 63.66 % | none | 189.4° |
| 12 | orange yellow | 0.4214 | +1.23 | 0.5121 0.4756 0.3391 | 0.4517 0.3987 0.3061 | 61.76 % | none | 149.7° |
| 13 | blue | 0.0590 | -1.61 | 0.2946 0.3062 0.4415 | 0.2272 0.2628 0.3573 | 18.66 % | none | 342.1° |
| 14 | green | 0.2231 | +0.31 | 0.3870 0.4475 0.3532 | 0.3012 0.3624 0.2818 | 43.38 % | none | 234.1° |
| 15 | red | 0.1238 | -0.54 | 0.4522 0.3299 0.3055 | 0.3765 0.2412 0.2370 | 25.91 % | none | 101.6° |
| 16 | yellow | 0.5901 | +1.71 | 0.5267 0.5107 0.3511 | 0.4703 0.4406 0.3291 | 73.83 % | none | 163.6° |
| 17 | magenta | 0.1876 | +0.06 | 0.4708 0.3744 0.4473 | 0.4049 0.3168 0.3751 | 36.99 % | none | 68.9° |
| 18 | cyan | 0.1850 | +0.04 | 0.3362 0.4286 0.4690 | 0.2709 0.3641 0.3958 | 34.56 % | none | 293.5° |
| 19 | white 9.5 (.05 D) | 0.8802 | +2.29 | 0.5442 0.5446 0.5405 | 0.5000 0.4999 0.4964 | 93.86 % | none | 180.6° |
| 20 | neutral 8 (.23 D) | 0.5900 | +1.71 | 0.5109 0.5116 0.5107 | 0.4581 0.4586 0.4579 | 76.77 % | none | 228.9° |
| 21 | neutral 6.5 (.44 D) | 0.3649 | +1.02 | 0.4710 0.4721 0.4719 | 0.4077 0.4089 0.4087 | 59.92 % | none | 272.1° |
| 22 | neutral 5 (.70 D) | 0.1906 | +0.08 | 0.4182 0.4184 0.4180 | 0.3417 0.3419 0.3415 | 42.23 % | none | 215.4° |
| 23 | neutral 3.5 (1.05 D) | 0.0882 | -1.03 | 0.3538 0.3552 0.3561 | 0.2639 0.2655 0.2662 | 26.95 % | none | 300.5° |
| 24 | black 2 (1.5 D) | 0.0315 | -2.51 | 0.2699 0.2700 0.2719 | 0.1700 0.1703 0.1716 | 13.29 % | none | 346.2° |

## What this means for the chart routine

- **The bands are narrower than the patches are apart.** At true exposure through the plain BT.709 OETF, neutral 5 lands at 42.23 %, just above the ARRI-style green band (38–42 %). Neutral 6.5 lands at 59.92 %, above pink (52–56 %).
  - The two bands are about 0.8 stop apart; the two patches are 0.94 stop apart.
  - Both land only when the picture is pulled down by 0.19–0.27 stop (BT.709 OETF), or 0.61–0.81 stop (pure 2.4), or through a tone-mapped output that rolls off the highlights.
- **Neutral 5 is 19.06 %, not 18 %.** It sits at +0.08 stops and neutral 6.5 at +1.02 stops, whatever the Timeline Colour Space or output curve. That is why scopecreep's chart check uses stops.
- **The skin patches** are 1 (dark skin) and 2 (light skin), at 120.7° and 117.8°. At true exposure they sit at 27.9 % and 56.2 % luma.
- **Blackmagic's cameras** describe pink as good exposure for lighter skin and green for darker skin, but publish no levels, and the levels change with camera model and ISO (see [false-colour-and-skin-tools](false-colour-and-skin-tools.md)).
