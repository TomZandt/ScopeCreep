# 224: the view block on every Panel

What building #224 (spec #196) showed. See ADR 0042.

- **Link was already on by default in the app's startup path** (`link_remembered` reads nothing saved as on, and the saved profile reads `state:link on`); what was off was `Link::default()`, so any display set built without the startup call was unlinked. It now opens on, locked by a Feed seam test (a fresh `DisplaySet` is linked, and a zoom on one picture Panel reaches every picture Panel). Whether the owner's session showed Link unlit for another reason (the strip's chip drawn quietly) was not reproduced; it needs a look at the app.
- **The block needs 175 logical pixels** (Fit, a 5-character readout slot, Link, the settings button and the padding), so the Panel's least width went from 120 to 176 and the Window's least size from 713 x 645 to 1015 x 645. The height is set by the tallest menu (the Hotkeys panel), not the Panels, once the Colour Setup row is gone: the Panels have 32 px more than they need at the least size.
- **Vectorscope defaults** (Both plots, All tonal range) were already the first choice of each setting; a test now says so, and a saved choice still wins.
- **Not verified here (needs a person at the app):** how the block, divider and the cut Colour Setup line look at UHD, and the page tab's right-click menu in a real Window. The pure seam tests cover layout, hit areas and menu entries at 1x and 2x.
