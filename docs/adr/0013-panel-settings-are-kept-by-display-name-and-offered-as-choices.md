# Panel settings are lists of choices, kept by display name in a plain-text file, in a menu the app draws itself

## Decision

- A display offers its settings as a short list of named choices (the first is the default), never as free numbers or switches that could mislead. The right-click menu is built only from that list; a Panel with no settings shows no menu.
- Choices are remembered in `~/.config/scopecreep/panel-settings`, one `display name<TAB>key<TAB>value` line each, next to the Panel grid's file. A remembered value that is not a choice any more (or a display that is gone) is ignored and the default is used, so a file from another version never reaches a display.
- The settings are keyed by the display's name, not its position, so rearranging or adding displays does not move anyone's settings.
- The menu is drawn by the app (flat rectangles and text over the Panels), not by a toolkit.
- The `--luma` command-line option is removed: the luma weights are the Luma waveform's setting (ADR 0004 still holds; the default is still "from primaries").
- A changed setting reconfigures the displays and measures the frame on screen again, so it takes effect with Resolve stopped.

## Considered and rejected

- **Keying by Panel position:** a new display would shift every setting after it.
- **A settings file in a structured format (TOML, JSON):** a new dependency for a handful of text values.
- **A native menu from the windowing toolkit:** winit has none; another toolkit is a large dependency for one menu.
- **Keeping `--luma` as an override:** two ways to set one thing, and the one the user can see (the menu) would disagree with the command line.

## Consequences

- Renaming a display forgets its settings; display names are therefore treated as stable.
- A new setting is one entry in a display's list and one line reading it in `configure`; the menu, saving and the label rule need no change.

## Note, 2026-11-18: settings can also be chips on the title bar (#116)

A display may mark a setting to be shown as a row of chips on its Panel's title bar (spec #113). It is still one of the display's ordinary settings: a short list of named choices, stored and remembered under the display's name exactly as above, and shown in the tags when it changes what is measured. A chip is one choice; the popover (opened from a small settings button on the title bar, or by right-click) holds the settings that are not chips. Only single choice is built so far (the Channel view's Y R G B, with Luma as a fourth channel choice); multiple choice and toggles follow with the displays that use them. The decision above is otherwise unchanged: no free numbers, nothing that could mislead.

## Note, 2026-11-18

Multiple choice and Toggle chips now exist alongside single choice (the luma waveform's Y R G B chips, the B/W and Colorise toggles). They are stored and remembered by display name exactly as above.

## Note, 2026-11-19: amounts are sliders (#83, #84)

A setting is either a list of choices or an **amount**: a number with a range, a step, a default, a unit, how many digits to show, and a map from the list values an earlier version saved. Choices stay lists; amounts are sliders in the popover with the number and its unit beside them, and a "Reset to default" row under each. The menu stays open while a slider is dragged, and the Panel is drawn again after each change. The range is limited to sensible values (the same rule as the lists: nothing that could mislead), a dragged value is a multiple of the step, and every default reproduces the look of the old default exactly. The file still keeps one value per Panel setting, now a plain number for an amount; a stored number is clamped into the range, one that cannot be read falls back to the default, and an old list value maps to the number it produced (trace brightness Dimmer, Normal, Brighter are 48, 100 and 240 %, both spellings "dim" and "dimmer"). The decision above is otherwise unchanged. Trace brightness (luma and Colour Waveform) was the first amount.
