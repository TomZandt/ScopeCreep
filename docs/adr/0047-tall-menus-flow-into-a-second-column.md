# Tall menus flow into a second column, so the Window's least size is 1420 x 764

Date: 2026-11-25 (spec #230). Supersedes the Window least size in ADR 0042, ADR 0044 and ADR 0045. See ADR 0026, ADR 0036, ADR 0044.

The Help menu added seven Commands to the Hotkeys panel, which lists every Command in one column. That column set the Window's least size, and the extra rows would have made it 1420 x 983, taller than the 1440 x 840 default Window.

## Decision

- A drop menu taller than `SPACE_MENU_MAX_HEIGHT` (720, a style token) flows into a second column. `DropMenu` does this for every menu, so the Hotkeys panel gets it with no special case.
- The Window's least size is now **1420 x 764** (it was 1420 x 833). `app/tests/min_window.rs` pins it.

## Consequences

- A menu never grows past the cap, however many Commands are added; a third column is a later decision.
- The "1420 x 833" in ADR 0042, ADR 0044 and ADR 0045 is superseded here.
