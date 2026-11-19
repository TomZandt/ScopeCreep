# What a display moves by hand is remembered beside the settings, not as a setting

## Decision

- A display can remember values the user changes by hand and the menu does not offer: the Loupe's position and zoom. They are *state*, not settings: free values that the display checks itself and replaces with its default when they are not sensible (a zoom outside 100 to 800 %, a position outside the Panel).
- State is kept in the same file as the Panel settings (`~/.config/scopecreep/panel-settings`), as a line whose key starts with `state:`, so there is one file to keep, and one place a stale line is ignored.
- Settings stay lists of choices (ADR 0013). State is not shown in a menu and cannot mislead: it moves a viewing aid, never what is measured.
- Changing state never counts as a change of settings: the Feed thread does not re-measure the frame for it. The display draws from its own copy, and takes the remembered value once, at start-up.
- The pointer reaches a display through `Display::pointer` (down, move, up, wheel, in device pixels over its Panel), and the display answers with what to remember. The window knows nothing about Loupes.
- The Loupe's zoom is in whole 100 % steps and means device pixels per source pixel (200 % is 2 × 2 device pixels), so a pixel peeper sees exact blocks, and the Loupe's size in device pixels does not change with the screen's scale.

## Considered and rejected

- **Loupe position and zoom as menu choices:** a continuous position cannot be a short list, and ADR 0013's reason for lists (nothing that misleads) does not apply to a viewing aid.
- **A second file for state:** two files to keep in step for no gain.
- **Zoom in logical pixels at HiDPI:** a 2 × 2 block of logical pixels is 4 × 4 device pixels at a scale of 2, so single pixels are blurrier to judge than on a standard screen.

## Consequences

- A later display with its own hand-moved aid (a movable marker, say) adds state keys and handles `Display::pointer`; the window and the file need no change.
- State written by a later version that a display no longer reads stays in the file, harmlessly.

## Note, 2026-11-19 (#162)

The Loupe's state (on, position, zoom, size) is now also the clean picture's, kept under its own display name (ADR 0034). The Channel view that first used it is out of the Window; its saved state stays in the file. The keys are constants in `loupe.rs` (`STATE_KEYS`), and a remembered zoom that is not a sensible one reads as the default.

## Note, 2026-11-19 (#165)

A picture Panel's zoom (and later pan and Link, spec #163) is display state of the other kind: held in memory by the display and never written to this file, so every session starts at Fit. Only Link is to be remembered (#168).
