# Every display has its own Panel, always visible, in a fixed grid

Most scope apps give a few slots and a picker for what each slot shows. scopecreep shows every Scope and Picture View at once instead, each in its own Panel, so a colourist can take in all the information at a glance without switching anything.

## Decision

- The Window holds one Panel per display, laid out in an ordered grid sized to the number of displays.
- The user can drag the borders between Panels. Panels cannot be hidden, moved freely or docked.
- Displays that differ only in a setting the user would compare side by side get separate Panels, not a toggle. For example, the vectorscope at 100 % and at 200 %, and the luma waveform and the colour-tinted waveform.

## Considered and rejected

- **A few slots with a picker per slot:** this hides information behind clicks, which goes against the goal of seeing everything at once.
- **Free docking or several windows:** more to set up and test. Both stay open as later changes if users need them.
- **Hiding Panels:** deferred until users ask for it.

## Consequences

- Every display measures every Feed frame, so the performance check (ADR 0003) must pass with all Panels shown, and each Panel's cost must be measured.
- Each Panel gets less room as displays are added, so labels and scales must stay legible at small Panel sizes.

## Update 2026-11-18 (#92): the vectorscope is one Panel

The vectorscope at 100 % and at 200 % is one Panel, not two. They are one scope at two zooms, and as two Panels each had its own label, menu and measurement, so the colourist set up and read two things that must always agree. The Panel shows both plots side by side (100 % left, 200 % right), is two grid cells wide so each plot keeps its size, and has one label, one right-click menu and one measurement per frame. The example in the third decision no longer holds; the luma waveform and the colour-tinted waveform still get separate Panels. Settings saved under "Vectorscope 100 %" carry over to the "Vectorscope" Panel; those saved under "Vectorscope 200 %" are ignored.

## Note, 2026-11-19 (#162)

The Channel view is out of the Window (ADR 0034): the clean picture's Y R G B chips and Loupe do its job, so the Window shows 7 Panels. Its module, tests and saved settings are kept.

## Note, 2026-11-20 (#174)

The two plots are now joined as one scope: edge to edge with no gap, one background with a faint divider at the join (graticule tone), sized from the smaller of half the Panel's width and its height and centred as a pair, so the pair scales as a unit. The Panel is still two grid cells wide with one label, menu and measurement; only the layout of the plots inside it changed. A single plot still fills the Panel.

## Note, 2026-11-20 (spec #196, ADR 0038)

The second and third decisions' limits no longer hold. Panels can be hidden (closed or tabbed away), moved and docked in a tree of tab groups (ADR 0038), the same display can be on the Window more than once (ADR 0039), and only the shown Panels on the current page measure and draw, so "every display measures every Feed frame" in the first consequence now means every shown Panel. The default layout still shows all seven displays at once, which is what "see everything at a glance" asked for. The rejected "picker per slot" returns in a different shape: Replace with in a Panel's menu, which changes one Panel and hides nothing else.
