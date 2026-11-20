# A Panel is an instance of a display, so a display can be on the Window more than once

Date: 2026-11-20 (spec #196). See ADR 0013, ADR 0015, ADR 0038.

A colourist wants four clean pictures, each on its own channel or crop, or two Vectorscopes. Today a Panel is its display: one of each, and the settings, the Loupe's state and the saved layout all name the display.

## Decision

- A **Panel** is one instance of a display with a stable id. Instances are independent: each measures (Scopes) and draws on its own and has its own settings.
- The first instance of a display keeps the display's name as its settings and state key, so every saved setting and every saved Loupe or link state still applies unchanged (ADR 0013, ADR 0015). Further instances are named "Name 2", "Name 3" and so on. The number is given when the instance is created, never reused while the page holds it, and never follows the Panel's position.
- Duplicate and Copy of this page copy each Panel's settings and saved state. Closing a Panel forgets its settings.
- Picture zoom stays in memory and is not saved (ADR 0035). Link stays one Window-wide switch and reaches every picture Panel.
- The Add list offers the seven displays the Window shows today; the displays kept out of the Window stay out through the existing in-Window flag.

## Considered and rejected

- **Numbering copies by reading order.** The prototype did this for display only, but a Panel's settings would then change owner when Panels move.
- **A per-Panel Link.** Unlinked crops are reached by turning Link off for all; a per-Panel switch is a later change.
- **Sharing one measurement between copies with identical settings.** A real saving for copies of a Scope, but a second change to the display code; unmeasured, so left until the cost check (#197) says it is needed.

## Consequences

- Display names are no longer the only key: the settings file gains keys for further instances, and an instance's Loupe state is stored under its key.
- Four clean pictures and two Vectorscopes are the cases to measure at UHD 50 (#197, with #186 and #187 saying the Vectorscope and Colorise are already over their share).
- A saved layout names instances, not displays.
