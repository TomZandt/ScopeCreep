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
