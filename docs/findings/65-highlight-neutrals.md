# 65: Highlight Neutrals

What the Highlight Neutrals ticket (#65) verified, headlessly (no Resolve involved: the Feed seam with ColorChecker frames from #52, `app/tests/highlight_neutrals_seam.rs`).

- A ColorChecker chart in DaVinci Intermediate (to Rec.709 Gamma 2.4) and one in ACEScct (ACES 2.0, Rec.709 BT.1886), taken through the Output Conversion, show all six grey patches (19 to 24) green over every pixel at the default 3 % tolerance, and no pixel of the 18 coloured patches green.
- A flat grey with a blue offset of 4 % of the range stays grey at 3 % and 1 % and turns green at 5 %; an offset of 2 % is green at the default.
- A pixel that is not neutral is drawn in its Luma grey (Rec.709 weights while the Colour Setup is unknown; the Measured Space's weights otherwise, as the waveform's default).
- The label says "Output picture" only for a converted picture (else "Picture as sent"), plus the Colour Setup, the Output Conversion accuracy and the tolerance in use.

What the check showed:

- **The real chart's greys are not perfectly neutral.** The published white patch (L\* a\* b\* 95.19, -1.03, 2.93) comes out of the DaVinci Intermediate to Rec.709 Gamma 2.4 pipeline as R 0.948, G 0.950, B 0.927: a 2.3 % spread. At the ticket's "about 2 %" the white patch was not green, so the default is 3 % (the ACEScct chart's greys are within 2 %). No coloured patch is green at 3 %.
- The tolerance is a list of choices (0.5, 1, 2, 3, 5, 10 %; 3 % first), not a free slider: ADR 0013 offers settings only as choices.
- The tolerance is a share of the full 0-1 range of the Output Colour Space's encoded values (largest channel minus smallest), whatever the Levels.

Not verified (a person at the app and Resolve): the look of the green over real footage, and a Resolve-graded chart: https://github.com/TomZandt/scopecreep/issues/73
