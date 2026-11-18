# #97: drag a Panel onto another to swap

What ticket #97 showed on 2026-11-18, at the pure layout seam (`app/tests/panel_grid.rs`, no GPU).

- Swapping two Panels gives each the other's cell and span; swapping a Panel with itself changes nothing; the swap survives a save and reload of the grid text.
- A press decides between a border drag and a Panel drag by where it lands: on a border (including one in a gap) it is always a border drag, on a title bar a Panel drag, on a display nothing. The two are never both.
- The Panel under a drop point is found from its cell; a gap holds none, so a drop there cancels.

Not verified (a person at the app): the feel of the drag and the quiet outline on the target.
