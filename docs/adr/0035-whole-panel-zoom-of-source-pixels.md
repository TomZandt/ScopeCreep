# Whole-Panel zoom of source pixels, from Fit to 800 %

A colourist checking noise, grain, focus or a key edge needs to see single picture pixels. The Loupe laid a small square over the picture, which covered what it magnified and had to be dragged to each spot; Resolve's own Viewer zooms the whole picture with the scroll wheel and shows the percentage. Spec #163 makes every Picture View zoom the way the Viewer does; this ADR records the first slice (#165, the clean picture's wheel, chip and slider).

## Decision

- **Zoom is of source pixels.** 100 % is one picture pixel to one device pixel, whatever the Panel's size or the screen's scale. The range is **Fit** (the whole frame fitted in the Panel, as always) up to **400 %**.
- **Free below 100 %, whole steps above.** Each wheel notch below 100 % scales the zoom by a fixed ratio (1.25) and a notch that would cross 100 % lands on it; above 100 % each notch is one whole step (200, 300, 400 %), so every picture pixel is an exact square and scaling never makes anything that looks like noise. At or below 100 % the picture is sampled as before; above it the stage reads the exact texel.
- **A frame already past 100 % at Fit** (a small frame in a big Panel) starts at Fit, shows "Fit 250 %", and zooms on in whole steps to 400 %. When Fit is 400 % or more there is nothing beyond it: the top of the range is the larger of 400 % and Fit, and zooming out stops at Fit.
- **Toward the pointer.** The picture pixel under the pointer stays under it. The picture's edge stops at the Panel's edge (a pan can never leave the Panel empty) and an axis on which the picture is smaller than the Panel stays centred, so the spot is held except where those two rules win.
- **The stage clips.** The zoom model (`zoom.rs`) gives the part of the picture inside the Panel's picture area and where it goes; the stage draws only that, so the legend strip, the mattes (ADR 0029) and the title bar are untouched.
- **The zoom chip.** The first chip on a picture Panel's title bar reads "Fit 23 %" at Fit and "200 %" zoomed, and is lit while zoomed in so a zoomed picture is never mistaken for the whole frame. Clicking it opens a popover with one slider from Fit to 400 %, whole percent below 100 and snapping to 100, 200, 300 and 400 above, and a way back to Fit. When the title bar is too narrow for all its chips the chip shortens to its number ("23 %") and stays first, so it stays on the bar.
- **Zoom is display state, not a setting.** It is held by the Picture View (and later the Window, for Link) and never written to the settings file; every session starts at Fit. The chip is not a `ToolbarDef`: the display answers `Display::zoom` and `Display::set_zoom`, and the window opens the popover for it.
- **The wheel belongs to the zoom.** On the clean picture the wheel zooms the picture, except over a Loupe that is on, where it is still the Loupe's zoom. The Loupe is not added to the clean picture by this spec (#161 is closed in favour of it); its code stays for the Channel view.

## Considered and rejected

- **Whole steps only, from Fit.** Simple, but a colourist framing an area between Fit and real pixels would jump from 23 % to 100 %.
- **Free zoom above 100 %.** Non-integer scaling blurs or shimmers single pixels, which is exactly what the zoom is for.
- **A zoom setting in the settings file.** It would reopen a session zoomed in on a frame that may be a different one, and every session would have to be told to forget it.
- **A fixed zoom in percent of the Panel.** Hides how much the frame is scaled; "Fit 23 %" says it.

## Consequences

- ADR 0028's stage asks the zoom model for the view (`PictureStage::draw_zoomed`); `draw` is Fit. Pan (#166, done: a middle-drag moves the centre point by the pointer's movement over the zoom, held by the same clamp the wheel uses) , the other picture Panels (#167, done: ARRI False Colour, Stops False Colour, Skin Tone and the saturation heatmap hold a `PanelZoom` each and draw through `draw_zoomed`, the legend's strip kept free), Link (#168) and Z (#169) grow the same model.
- The Stage uniform grew by two vec4s (the window of the picture drawn, and the exact-texel flag).
- `docs/research/panel-controls.md` said "Drop for now" for a picture zoom control (the Loupe did that job). This ADR reverses that for the picture's zoom (chip, slider and wheel; the research table is updated), while the Loupe's own zoom stays on the Loupe.
- ADR 0015's display state now includes the zoom, kept beside the settings but not in them (and never saved).
- `Display::zoom` and `set_zoom` take the device scale, because a legend's strip is in device pixels and Fit depends on it.
- The clean picture's chips are seven (with the link chip) at the left of the settings button; the narrowest Panels still run over (#171).

## Note, 2026-11-19 (#168): Link

- **One Window-wide Link.** A link chip, second on every picture Panel's title bar (after the zoom chip), switches one Link for the whole Window and is lit on every picture Panel while it is on. The display set owns the one `zoom::Link` and hands it to every display on registering; each Panel's `PanelZoom` joins it.
- **One shared view, in picture coordinates.** While Link is on every `PanelZoom` zooms, pans and reads the shared `ZoomModel` instead of its own: one zoom and one centre point in source pixels, so Panels of different sizes look at the same pixels. A shared zoom at or below a Panel's own Fit is Fit for that Panel, so at Fit each fits its own Panel ("Fit 50 %" here, "Fit 31 %" there). Scroll, middle-drag and the slider on any Panel move all of them. Z (#169) works on the same model, so it moves all of them too.
- **On takes the clicked Panel's view; off keeps each.** Turning Link on copies the clicked Panel's view into the shared one. Turning it off leaves every Panel at the shared view it had; each Panel takes it on its next use (a generation counter on the Link), and from then on they move apart.
- **Link is remembered, zoom is not.** Link is Window-wide state under the name "Window" beside the Panel settings (ADR 0015, `state:link`); anything but "on" reads as off. Every session starts at Fit.
- **Rejected:** a Link flag per Panel (they could disagree, and "lit on all of them" would need syncing); the Window moving every Panel's own model (it would have to know each Panel's frame and strip).

## Note, 2026-11-20 (#176): Link on by default, 800 %, no slider

Three changes asked for by the product owner after using the shipped zoom (spec #163 follows them):

- **Link opens on.** Nothing saved, or rubbish saved, now reads as on; only a saved "off" is off, and a saved choice still wins. This replaces the rule in the #168 note ("anything but on reads as off").
- **The range goes up to 800 %** (it was 400 %): free below 100 %, landing on 100 %, then whole steps 200, 300 ... 800 %, nearest texel above 100 %. The top of the range is the larger of 800 % and Fit.
- **The zoom chip has no popover and no slider.** A click goes to Fit, on every picture Panel while Link is on; the wheel is the way to zoom. This drops the slider of #165 (`Menu::zoom`, the Reset row and their tests); `Display::set_zoom` stays as the model's Fit and test entry point, and `DisplaySet::zoom_to_fit` is what the window calls. The chip's tooltip says "Zoom: scroll, or click for Fit".

## Note, 2026-11-20 (spec #196, ADR 0040)

The zoom chip and the link chip on every picture Panel's title bar become a view strip at the right end of the menu bar row: Fit, the shared zoom readout and Link. A picture Panel keeps a read-only zoom tag. The zoom model, Link's rules and saved state, the wheel, middle-drag and Z are unchanged.
