# #210: Dock by dragging a title

What ticket #210 showed on 2026-11-20, at the pure layout seam (`app/tests/dock_layout.rs`) and the pure UI seam (`app/tests/dock_ui.rs`), headless. Nothing was checked at the app or in Resolve: how the drag, the half-outline and the notice feel on a real monitor is still to be seen by a person.

- A press on a title becomes a drag after 4 logical pixels (`SPACE_DRAG_START`, 8 device pixels at 2x). A press that never moves that far is a click and changes nothing; before, any press-and-release over another Panel swapped.
- Over another tab group, the outer quarter of each side (`SPACE_DOCK_EDGE`) is that edge's zone; in a corner the edge nearer by share of its side wins. The rest is the middle, which does nothing until #211 turns it into joining as a tab.
- A dock on an edge splits the target 50/50 (net of the gap) and the dragged Panel takes the half on that edge. The overlay is that exact half (outlined quietly, like the old swap outline), at 1x and 2x. The dragged Panel's old space goes to its sibling (the expected cells are worked out independently in the tests). The dragged Panel has the focus after.
- Dropping a single-Panel tab group on itself does nothing, on any edge. Shift on drop still swaps the two Panels' places (the #97 swap); with Shift held the overlay is the old outline of the whole target.
- Escape during a drag cancels it; the layout and the saved text are untouched.
- A side dock whose half would be under the minimum Panel size leaves the layout as it was and shows "No room to dock here: a Panel needs at least 120 x 105." Until #211 this is a refusal, not a tab.
- One drag is one layout change and one save. The app has no undo stack yet, so "one undo step" is met by there being one mutation (`LayoutTree::dock`); the page's undo (ADR 0041) will pick it up.
- Docking out of a tab group that holds several Panels removes only that tab and leaves the group's space alone; no way to build such a group exists until #211, so it is covered by the pruning rules the Close tests already check, not by a test of its own.
