# #8 Stage 0: Which luma weights does Resolve's own waveform use?

> Copied from the GitHub issue comments on TomZandt/scopecreep#8 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-13)

Stage 0 check 7 result (Resolve 21.2.0.6 Studio, Color page Waveform, Y mode, Low Pass Filter off, Data Level Scopes, 10-bit scale). Resolve's Y waveform uses fixed standard weights on the encoded output R'G'B', chosen by the Output Colour Space's primaries:

| Output Colour Space | Weights Resolve uses | ADR 0004 |
|---|---|---|
| Rec.709 Gamma 2.4 (YRGB and DWG) | Rec.709 0.2126 / 0.7152 / 0.0722 | agrees |
| **P3-D65 (gamma 2.6), P3-DCI** | **Rec.709** | **disagrees** (ADR: 0.2290 / 0.6917 / 0.0793) |
| Rec.2020 Gamma 2.4, Rec.2100 PQ, Rec.2100 HLG | Rec.2020 0.2627 / 0.6780 / 0.0593 | agrees |

Method: encoded ramps 0..1 in R, G, B and secondaries (media `lumasweep`, projects `scopecreep-test-luma-*`), input tagged equal to output so values pass through, line fit of each trace against the graticule (constant +2 code offset removed). Uncertainty about +-3 codes (10-bit). Green separates Rec.709 from P3-derived weights by 24 codes (red by 17), so P3 is unambiguous. Measured vs predicted: green 732 (709 weights 732, P3 708); on 2020/PQ/HLG green 694 (2020 weights 694).

Other findings: no scope setting changes the luma calculation (Y / CbCr / RGB are display modes). Y is computed on the *encoded* signal, so for PQ/HLG it is Rec.2020 weights on PQ/HLG-encoded values, not true luminance. The scope clips display to 0..1023 (not isolated whether before or after the Y calculation). In Data Level mode the 10-bit graticule runs 0-1023 with no 64..940 mapping. Extents and Video-vs-Data Level not tested here (feeds #18). The Parade/low-density single-level chart patches are drawn sparsely by the scope; use ramps for scope readings.

**ADR 0004** needs updating for P3 (done in #13). Proposed "match Resolve" mode wording: *"Match Resolve: use the luma weights Resolve's own waveform uses. Rec.709 weights for Rec.709, P3-D65 and P3-DCI; Rec.2020 weights for Rec.2020 and Rec.2100 (PQ and HLG)."* Default stays primaries-derived; the Scope label shows which set is in use.

Screenshots and notes (not in git): `~/scopecreep-testdata/screenshots/luma-*.png`, `~/scopecreep-testdata/notes/8-luma-weights.md`.


