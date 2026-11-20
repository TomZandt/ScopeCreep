# #169: Z, Fit and back, as a reassignable hotkey

What ticket #169 showed on 2026-11-20, at the zoom model seam (`app/tests/zoom_link.rs`), the Feed seam (`app/tests/link_seam.rs`, GPU), and the command and Hotkeys panel seams.

- Zoomed in and panned, Z goes to Fit and a second Z comes back to the same zoom and the same picture point at the middle (not the frame's middle). At Fit with nothing to go back to, Z does nothing; going back uses the remembered zoom up, so after a Fit by the chip Z does nothing again. Before a frame has arrived it does nothing.
- With Link on, Z over any picture Panel takes all four picture Panels (different sizes) to Fit, and Z over another brings all of them back: each draws exactly the pixels it drew before. With Link off only that Panel moves.
- Over a Scope (the luma waveform) Z does nothing. In the window it acts on the Panel under the pointer, so off the Panels it does nothing too.
- The command "Zoom: Fit / last zoom" has Z by default, is listed in the Hotkeys panel, and can be reassigned, cleared, reset and saved like the others. No menu lists it yet: it acts on the Panel under the pointer, which a menu entry has no way to name.
- The remembered zoom is display state and is never saved (ADR 0035).
- Checked at the app beside Resolve by the product owner on 2026-11-20: zoom, pan, Link and Z all work, and keys typed in Resolve never reach the app (#170, #149).
