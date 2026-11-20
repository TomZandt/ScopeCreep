# The Window is a tree of tab groups: Panels can be added, moved, tabbed and closed

Date: 2026-11-20 (spec #196). See ADR 0011, ADR 0012, ADR 0030.

The Panel grid fits one shape of footage and one set of seven Panels. A colourist cannot hide a Panel, put one beside another, or have a second clean picture, and on 2.39:1 or 9:16 footage the default cells hold small pictures in wide empty rectangles. ADR 0011 deferred hiding and docking "until users ask"; the product owner asked after trying a throwaway prototype of a tiling window manager's rules (branch `prototype/tiling-layout`).

## Decision

- The **layout tree** replaces the Panel grid. Its leaves are **tab groups** (one or more Panels, one shown) and its inner nodes are **splits** (side by side or stacked, with the share of its space, net of the gap, that the first child takes). The tree always fills the Panel area, so no cell is ever empty.
- **Add panel** splits the focused tab group along its longer side (wider than tall: the new one on the right; otherwise below) at 50/50 and focuses the new Panel. The direction comes from the focused cell's shape, never the footage's.
- **Edge add**: a **+** on each edge of a tab group opens the list of displays and adds the new tab group on that side at 50/50.
- **Dock**: dragging a title or tab onto another tab group, the outer quarter of an edge splits that edge and the middle makes a tab; the overlay shows the exact result; Escape cancels; Shift on drop swaps the two tab groups (the swap of #97 is kept).
- **Close** removes a Panel and its sibling takes the space; an emptied page shows one large Add panel button. A Panel's menu offers Replace with, Duplicate right, Duplicate below and Close.
- **Minimum size.** A tab group is at least the minimum Panel size; a split is at least its children plus the gap (side by side the sum, stacked the larger). Ratios are held so no child goes under its minimum, and an add or dock that would break a minimum is refused with a notice that names it.
- **Hidden Panels do not measure or draw.** Only the shown Panel of each tab group on the current page measures and draws; the rest keep their settings but cost nothing.
- Old saved grids convert to a tree when they are a sequence of cuts (the default layout is); one that is not, or cannot be read, gives the default.

## Considered and rejected

- **Keep the grid and add hide and swap.** Swap already existed and could not place a Panel beside another; a grid with spans can describe layouts that are not a sequence of cuts and needs repair rules (#179) that a tree does not.
- **Floating Panels that overlap.** Needs z-order and clipping every display inside one render pass, and a Panel can cover a scope. A tiled Window never hides one Panel behind another.
- **Panels as their own OS windows.** The window state (device, surface, text atlas) is one object in the app, and the desktop's own window manager can already tile the app beside Resolve. It stays a later change.
- **An off-the-shelf docking toolkit.** The app draws its own chrome through the style tokens and components (ADR 0026, 0027); a toolkit would bring its own look and break the style check.
- **A tiling layout that re-arranges itself live.** It moves Panels while the colourist works and fights hand arrangement; see ADR 0041.

## Consequences

- ADR 0011's "Panels cannot be hidden, moved freely or docked" and its consequence that every display measures every Feed frame no longer hold. ADR 0003's performance check must still pass with the most Panels the Window can show.
- ADR 0012's grid rules (column and row weights, spans, every cell used) are replaced by a ratio per split; its Panel minimums, the minimum Window rule and the repair of saved sizes (#179) carry over to the tree.
- Panels need ids of their own so a display can appear more than once (ADR 0039), and the saved layout text moves to a new version.
- The prototype's pure module (a plain tree and functions over it) is the reference for the model; the prototype itself is never merged.
