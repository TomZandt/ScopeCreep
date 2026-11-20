# Pages are the layouts, and Auto layout and Auto resize are one-shot buttons per page

Date: 2026-11-20 (spec #196). See ADR 0030, ADR 0036, ADR 0038.

The nine named layouts (ADR 0030) are a menu and a hotkey: nothing on screen says they exist, and recalling one replaces the arrangement being worked on. Separately, a layout drawn for 16:9 fits other footage badly, and the product owner wanted one press to fit the page to the footage, not a Window that moves by itself.

## Decision

- The Window has one or more **pages**, shown as tabs in a **Page bar** under the Colour Setup line. Each page has its own layout tree, focus and undo. A new page comes from a template (All 7 Panels, Picture views, Scopes, Picture only, Empty page) or as a copy of the current one, and a template page is arranged once for the Feed's current aspect. Pages are renamed in place and deleted, with at least one always left; the current page and every page are remembered. Only the current page's Panels measure and draw.
- The nine named layouts become pages, named as they were, in slot order after a first page, Main, that holds the arrangement the user had. Ctrl+1 to 9 become "Go to page 1 to 9"; the save-into-slot Commands (Ctrl+Shift+1 to 9) are removed. New Commands: Add panel, Auto layout, Auto resize, New page, Duplicate page, Delete page and Undo layout change (Ctrl+Z, per page, session only, the last 20 changes).
- **Auto layout** is a button on the Page bar that re-arranges every Panel on the current page once. It keeps the Panels in reading order, tries the ways of cutting that ordered list into splits (all of them up to 6 items, then only first cuts near the middle), a tab group counting as one item; sets each split's ratio from the items' weights; fits picture cells to the footage; drops any cut that breaks a minimum; and keeps the best score. The score is the sum over items of weight times ln(1 + useful area), with weights clean picture 3, luma waveform 1.6, Vectorscope 1.2 and every other display 1. Useful area is the fitted frame for a Picture View (less its legend strip), the aspect-true plot for the luma waveform (less its gutter) and the inscribed circle for the Vectorscope. If no cut fits, nothing changes and a notice names the minimum.
- **Auto resize** is a button beside it that changes ratios only, top down: where a split has exactly one picture-shaped child (a Picture View or the luma waveform) its ratio is set so that child's cell has the frame's aspect, clamped to 15 to 85 %. Places and nesting never change.
- Footage shape is the Feed frame's aspect (ADR 0012's #151 note), 16:9 with no Feed. Neither button runs by itself: a change of footage, window shape or Panel set never re-arranges a page, and Undo restores the previous layout.

## Considered and rejected

- **Auto layout as a toggle that re-arranges on every change.** Prototyped first. It moves Panels while the colourist works, needs a rule for when hand arrangement switches it off, and the product owner asked for a press, not a mode.
- **Only Auto resize.** Cheaper, but a picture beside a picture in the wrong place stays wrong; the two answer different needs.
- **A full search of every cut at any size.** 9 items is already 35,840 cuts (about 420 ms in the prototype under Node); above 9 a documented greedy fallback takes over.
- **Keeping the nine slots beside pages.** Two ways to keep a layout, one of them invisible.
- **Pages that carry their own display settings.** A Panel's settings travel with the Panel; per-page settings would double the places a setting can live.

## Consequences

- ADR 0030's slot file, the slot Commands and its Layout menu rows go; the layout text it saved is read by the migration. ADR 0036's default hotkeys change as above, and a saved hotkey line that names a removed Command is skipped by its existing rule.
- Pages are the cheap way to keep an expensive scope off the Feed: a Panel on another page costs nothing (ADR 0038).
- Auto layout runs on the UI thread and has a budget of about 100 ms at 7 Panels, which the pure seam measures; it must never stall the Feed thread (ADR 0003).
