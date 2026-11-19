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
