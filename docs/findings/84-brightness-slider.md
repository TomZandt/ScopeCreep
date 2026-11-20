# #84: a slider row in the Panel menu, proven on waveform trace brightness

What ticket #84 showed on 2026-11-19, at the pure settings seam (`app/tests/settings.rs`) and the display seam (`display_seam.rs`, `colour_waveform_seam.rs`, `waveform_seam.rs`), headless.

- A setting is a list of choices or an amount (range, step, default, unit, digits, legacy map). A Panel with no settings still has no menu.
- Trace brightness is 20 to 300 % in steps of 5, default 100 %. The default draws exactly as the old Normal (the gain is 0.25 at 100 %, and the old list value Normal, saved in a file, draws the same pixels as the default).
- An old Dimmer or Brighter becomes 48 or 240 %, the percentages of the old gains 0.12 and 0.6; the spelling "dim" works too. A stored number out of the range is clamped; an unreadable one is the default; the number is kept in the same settings file, one value per Panel setting.
- A value between two old choices draws between them (170 % sits between 100 and 240 % on a grey ramp), and the label and counts do not change (brightness is drawing only).
- The menu seam: the slider row maps a press to a step of the range at scale 1 and 2, a drag past either end of the track (or out of the menu) gives the end, a press on a heading or the row's left padding is not a slider press, the Reset row gives the default, and everything drawn stays inside the menu's width.
- The menu stays open while dragging (the window keeps the slider being dragged until the button is let go) and the value is saved on every change; that part is window code and was not run headless.
- Not verified: how the slider feels in the real Window beside Resolve, and its drawing on a HiDPI display (#91).

**Checked at the app by the product owner, 2026-11-20 (#91):** the sliders work (the Panel follows the drag, the menu stays open, Reset to default works), and they are sharp and usable on HiDPI and small Panels, leave UHD 50 playback undisturbed, and come back after a restart. One fault: the slider's handle and number do not move while dragging (#220).
