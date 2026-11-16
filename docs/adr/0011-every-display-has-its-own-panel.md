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
