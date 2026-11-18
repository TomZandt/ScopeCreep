# #95: three Panels out of the Window

What ticket #95 showed on 2026-11-18.

- The gamut/legal warning, Highlight Neutrals and the CIE plot are off by one switch each (`IN_WINDOW` in `app/src/display.rs`). Their modules and seam tests are unchanged and build them directly, so they pass without the Window.
- The Window registers 11 displays; `PanelGrid` sizes itself from the display count, so the grid laid out for 11 with no change.
- Saved settings for the three are kept: the settings file holds every Panel's lines whether or not the Panel is shown, and a test pins that a reload writes them back unchanged.
- Test runs on this machine fail with `RequestDeviceError(OutOfMemory)` in a changing set of GPU tests while Resolve (4.4 GB), the app and the desktop hold about 7.5 of the RTX 3070 Ti's 8 GB. Each binary passed on a retry except the large HiDPI draw in `display_seam` (`the_displays_draw_at_the_screens_scale`), which passed in the first full run. Run the GPU tests with Resolve closed for a clean result.
