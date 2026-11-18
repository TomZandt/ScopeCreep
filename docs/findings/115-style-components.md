# 115: Style components and the showcase

What the components ticket (#115) verified, headlessly (`app/tests/components.rs`, 27 tests at 1x and 2x) and by looking at the showcase image.

- Every component (chip single, multiple and toggle, channel chip, segmented control, slider with number, popover, tooltip, Panel frame and title bar, top bar, status bar, legend strip) has a pure size and hit area that scales exactly with the display, and a drawing in every state. Every state of a chip draws differently; a disabled control never lights.
- Title bar: chips and the settings button stay at the right, the name truncates before anything else moves, tags go to the hover when they do not fit beside the name.
- Chip rules: single choice keeps exactly one on; multiple choice never lets the last one go off; a toggle may be off.
- The right-click menu is drawn by the popover component and looks the same as before; the existing menu tests pass.
- The legend strip is horizontal, full width, left to right, one row. No vertical legend remains: Stops False Colour (since #93), ARRI and the heatmap all draw a strip under the picture.
- The showcase (`cargo run -p scopecreep --example style_showcase`) writes `style-showcase-1x.png` and `style-showcase-2x.png` to `~/scopecreep-testdata`; the 2x sheet is the 1x sheet doubled and sharp. Text widths in the pure layout are an estimate (`SPACE_CHAR_WIDTH`), tuned so "Luma waveform" fits its title bar.
- GPU tests share one GPU with other agents; `display_seam::the_displays_draw_at_the_screens_scale` failed with `Out of Memory` when the GPU was nearly full (7.4 of 8 GB used by others).

Not verified (a person at the app): how the redrawn menu look in the real window on a monitor, and the showcase's look (product owner review of the attached image).
