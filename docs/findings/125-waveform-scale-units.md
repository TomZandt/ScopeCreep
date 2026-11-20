# #125: waveform scale units, 10-bit codes, percent and nits

What ticket #125 showed on 2026-11-19, at the pure seam (`app/tests/waveform_scale.rs`) and the Feed seam (`app/tests/waveform_seam.rs`, needs a GPU). Decision: ADR 0033.

- Each unit names the same trace positions: video levels put 100 % at code 940, 50 % at 502, 0 % at 64; the edge rows read `> 109.5` and `< -7.3`. PQ puts 100 nits at code 509, 1000 at 723 and 10000 at 940 (video levels), 520, 769 and 1023 (full levels), as ST 2084 gives. HLG on a 1000 nit display puts 1000 nits at white.
- The counts and edge rows read back identical in every unit; only the scale numbers, the reference lines and the label change.
- Nits are offered in the popover only when the Output Colour Space is known and PQ or HLG. A remembered "nits" falls back to 10-bit codes (untagged) on any other output.
- The tag says `scale in %` or `scale in nits` (HLG: with the 1000 nit display it assumes); 10-bit codes, the default, add no tag.
- Not checked at Resolve (#153, closed by the product owner on 2026-11-20 on the strength of the headless tests): how the lines and numbers look in the Window on PQ and HLG projects, and whether Resolve's HLG nits readout agrees with the 1000 nit display assumption (ADR 0033 stays an assumption).
