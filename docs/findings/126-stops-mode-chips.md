# #126: Stops False Colour mode as chips

What ticket #126 showed on 2026-11-19, at the pure UI seam (`app/tests/toolbar.rs`; no Resolve needed).

- The mode is two chips on the title bar, Zones and Grey +1, one at a time. Full zones is the default and is lit when nothing is stored. B/W follows them only in Grey +1, where the picture is unpainted.
- A click on the chip already on keeps it on; the choice is remembered per Panel and round-trips through the saved settings.
- The popover keeps only the legend. The tag "grey and +1 stop" still follows the mode (it names a measurement change); the chips and the legend are drawing choices and are not tagged.
