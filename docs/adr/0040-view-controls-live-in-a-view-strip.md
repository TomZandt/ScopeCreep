# View controls live in a view strip on the menu bar, and title bars are three fixed zones

Date: 2026-11-20 (spec #196). See ADR 0013, ADR 0035.

The zoom chip and the Link chip share a right-anchored row with each Panel's own chips, so Link sits at a different x on every Panel, is missing on the scopes, and is the first chip a narrow Panel pushes into its popover (#171). Link is one Window-wide switch drawn on every picture Panel. The product owner asked for the view controls to be separate from the Panel's controls and always in the same place.

## Decision

- A **view strip** at the right end of the menu bar row holds **Fit**, the shared zoom readout and **Link**. Fit takes every picture Panel to Fit. The readout shows the shared zoom while Link is on ("Fit" or "200 %") and "Mixed" when Link is off and Panels differ. The strip is disabled when the page has no picture Panel, never moves into a popover, and counts in the minimum Window width.
- A picture Panel's title bar loses its zoom and Link chips and keeps a **read-only zoom tag** (such as "Fit 17 %") with its other tags. The wheel, middle-drag pan and Z behave as ADR 0035 and ADR 0036 say, and Link keeps its saved state and its on-by-default rule.
- Every title bar is **three zones**: label and tags on the left, the Panel's own chips, then the settings button fixed at the right edge. On a narrow Panel only the chips give way to the popover, the leftmost first (#171's rule). The gap between chip groups is at least twice the gap within a group, so the Vectorscope's eight chips become three groups (plots, mono, tonal range).

## Considered and rejected

- **A fixed slot for the zoom and Link chips on every title bar.** Consistent, but still drawn once per Panel, and on a narrow Panel the slot competes with the Panel's own chips.
- **A per-Panel zoom chip beside the strip.** The strip's Fit and the wheel cover it; the tag keeps the information.
- **Moving B/W and channel chips out as well.** They change what a Panel shows and belong on that Panel (ADR 0013's toolbar).

## Consequences

- ADR 0035's "zoom chip" and "link chip on every picture Panel" become the view strip; its zoom model, Link rules and Fit behaviour are unchanged.
- The menu bar row is no longer only menus; its height and the minimum Window width must hold the strip.
- Spec #192 adds a View menu (legends on and off) to the same row and owns the waveform's edge labels (#183). The two do not clash, but both count in the minimum Window width and in the menu bar tests; whichever is built second adjusts to the first.
