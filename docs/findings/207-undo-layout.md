# #207: Undo layout change

What ticket #207 showed on 2026-11-20, at the pure layout seam (`app/tests/undo_layout.rs`), headless. Nothing was checked at the app: Ctrl+Z, the Layout menu entry and a real divider drag are still to be tried by a person.

- Undo steps back through close, add, swap and a divider drag, one step each, and ends with nothing to undo. A drag is one step however many moves it has (the history is taken at the press and kept at the release); a drag that moves nothing, an add of a Panel already on the page and an add refused for no room are not steps.
- Depth is 20: after 25 changes exactly 20 can be undone and the oldest 5 are gone. The history is a value (`LayoutHistory`) kept beside a tree, not a global, so a Page can own one later (#212 onwards). It lives in memory only, so a restart starts with nothing to undo.
- It stores the arrangement only, so undo never changes a Panel's settings or state. The focus alone is not a change, and after undo the focus is a Panel that is on the page.
- Showing the default layout or a saved layout is also one step (not asked for by the ticket; cheap and expected). Save into a slot is not a layout change.
- "Layout: undo change" is a Command with Ctrl+Z by default, reassignable. The Layout menu has "Undo layout change" (hotkey shown), greyed when empty. Ctrl+Z was free: the one test that assigned it to the Fit toggle now uses Ctrl+Y.
- For later tickets: record a change with `history.change(&mut tree, |t| ...)` (replace, duplicate, dock, Auto resize, Auto layout), or `begin_drag`/`end_drag` for a gesture. `layout_menu` still shows the entry enabled for size measuring; the Window uses `layout_menu_with_undo`. The Layout menu got one entry taller, so `every_layout_menu_fits_a_small_window` uses a 440 high window.
