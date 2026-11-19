# #168: Link, picture Panels zoom and pan together

What ticket #168 showed on 2026-11-19, at the zoom model seam (`app/tests/zoom_link.rs`), the Feed seam, the Toolbar seam and the Settings seam (`app/tests/link_seam.rs`); a GPU, no Resolve.

- Two Panels of different sizes (800 x 450 and 320 x 180 for a 1600 x 900 frame) share one zoom and one centre point: scroll, middle-drag and the slider on either move both, they look at the same picture point, and at Fit each says its own Fit ("Fit 50 %", "Fit 20 %").
- Turning Link on takes the clicked Panel's zoom and spot; turning it off keeps every Panel's view, and they then move apart.
- At the Feed seam (ARRI False Colour, Stops False Colour, Skin Tone, saturation heatmap at four different Panel sizes): after a wheel on one and a pan on another every Panel reads "200 %" and the frame's split sits the same distance right of each Panel's middle (within a pixel of each other, within 3 px of the pan: the Panel a zoom starts in rounds its Fit placement to whole device pixels).
- The link chip is second on every picture Panel's title bar, unlit by default, and lights on all of them at once.
- Link round-trips through the settings text (`state:link`); rubbish ("rubbish", "ON", "1", "true") reads as off. Zoom is still never written.
- The wheel stepping from a smaller Panel can land below a larger Panel's Fit (for instance 49 % against a Fit of 50 %): that Panel shows Fit until the shared zoom passes it.

Not verified (a person at the app beside Resolve, #170): clicking the link chip and the wheel and middle-drag across Panels with a real mouse.
