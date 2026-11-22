# View controls return to every Panel's title bar, as a block

Date: 2026-11-22 (spec #196, #224). Supersedes ADR 0040's view strip. See ADR 0035, ADR 0036, ADR 0013.

The product owner tried the Window with the view strip (ADR 0040) at the right end of the menu bar row and wanted Fit, zoom and Link back on each Panel: the controls belong to the picture they act on, and the strip made the eye travel to the top of the Window for something done at a Panel. The Colour Setup line also took a full-width row of its own.

## Decision

- **A view block on every Panel's title bar:** Fit, the Panel's own zoom readout ("62 %", "200 %"; "Fit" is only the button) and Link, at the top right. The title bar is, left to right: the name and tags, the Panel's own chips, a **divider**, the view block, and the settings button fixed at the right edge. The block is on every Panel, scopes too, so it is always in the same place; where nothing zooms (a Scope, or a picture before its first frame) it is drawn disabled and greyed, with no readout, and hits nothing. The divider is a hairline as tall as a chip with a gap on each side, and only drawn where the Panel has chips.
- **Link stays one Window-wide switch.** It is lit on every Panel with a picture while it is on, and a click on any Panel's Link switches it everywhere; turning it on takes the clicked Panel's view. **Fit** on a Panel takes that Panel to Fit, and every picture Panel with it while Link is on (they share one view). Link opens on in a fresh profile: `Link::default()` is on, so a Window that was never told otherwise is linked, and a saved "off" still wins.
- **The block never moves into the popover;** on a narrow Panel the chips give way first (whole settings from the right, ADR 0040's rule that stays) and then the name. The Panel's least width grew from 120 to 176 logical pixels so that the block, the settings button and the bar's padding fit a Panel of the least size; the Window's least size grows with it (1015 x 645).
- **The Colour Setup line moves into the menu bar row,** right-aligned at its right end, so the full-width row under the menu bar goes and the Panels get its height back (32 px). It is cut with an ellipsis to the room right of the menu titles, never overlapping them, and its full text is the hover. A menu open over it hides it, as before.
- **Right-click on a page tab opens the Layout menu's entries,** dropping from the tab, after making that tab's page the current one (Duplicate, Rename and Delete act on the current page). The Layout menu in the menu bar stays.
- **Vectorscope** defaults are the Both plots and the All tonal range: they already were the first choice of each setting, now locked by a test; saved choices are kept.

## Considered and rejected

- **Keeping the strip and adding per-Panel Fit.** Two places for one control.
- **A narrower block that hides the readout on small Panels.** The block would not be in the same place on every Panel, which was the point.
- **Collapsing the block into the popover on a narrow Panel.** It is the one thing a colourist reaches for on a zoomed picture; raising the Panel's least width costs less than hiding it.

## Consequences

- ADR 0040's view strip (and its menu bar row width, the "Mixed" readout, the read-only zoom tag) is superseded; its three-zone title bar and chip-order rules stand, now with the block as the middle zone.
- ADR 0035's zoom model, Link rules and saved state are unchanged; its "zoom chip" and "Link chip" are the Fit and Link buttons of the block.
- The Colour Setup line (ADR 0007's wording) is unchanged; only where it sits and how it is cut changed.
