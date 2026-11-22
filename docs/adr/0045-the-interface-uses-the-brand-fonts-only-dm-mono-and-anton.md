# The interface uses the brand fonts only: DM Mono for all text and numbers, Anton for the wordmark

Date: 2026-11-22 (spec #196, #238). Supersedes the implicit decision that chrome text is the system sans (every `Family::SansSerif` in the text renderer). Notes on ADR 0026 and ADR 0044. See ADR 0026, ADR 0027, ADR 0044, ADR 0046.

The Window's text was whatever sans the machine had, so it looked different on Linux, Windows and macOS, and its digits were proportional, so a changing number (a zoom readout, the rate and dropped frames) made its neighbours move. The product owner tried the brand fonts in an interactive demo (first only for numbers, then for everything) and chose: everything is our fonts.

## Decision

- **DM Mono** (SIL Open Font Licence 1.1, Regular and Medium) is the interface font: every label, value, menu, tag, tooltip, Panel name and chip. Regular by default; **Medium** for emphasis (the focused Panel's name, headings in menus and popovers). Every digit is as wide as every letter, so numbers line up and do not move.
- **Anton** (OFL 1.1) is only the wordmark and display headings (the About popover's title, the Stylesheet). Never small chrome.
- Both are compiled into the app (`include_bytes!` in `app/src/fonts.rs`) and loaded into the text renderer's font database, whose default family is set to DM Mono, so no chrome text draws in a system sans and the Window looks the same on every platform. The system's fonts are only a fallback for a glyph DM Mono lacks (for example the "→" in the Colour Setup line); the app's own symbols are icons (ADR 0046), not text glyphs.
- **Tokens by role** in `style.rs`: `FONT_INTERFACE`, `FONT_WORDMARK`, `FONT_EMPHASIS_WEIGHT`, the `FontRole` enum (Interface, Emphasis, Wordmark, Icon), `TYPE_WORDMARK` and `WORDMARK_TRACKING`. A face of the type scale (`components::Face`) names its font role; an overlay text draws in its face's font.
- **Files, licences and provenance** are under `app/assets/fonts/` (source URL and SHA-256 of each file in its README, the OFL text of each font exactly as shipped, with its copyright line). The repository root's `THIRD-PARTY.md` lists both fonts with their licence text. The OFL allows bundling and use with software of any licence; the fonts are not modified and not sold on their own.

## Cost: wider text

DM Mono is 0.6 em per character, so a 13 px label is 7.8 px a character and a 12 px scale number 7.2. The pure layout (ADR 0027) already laid every character out at `SPACE_CHAR_WIDTH` = 7.9 px, which was meant as a safe upper bound; the least sizes (title bar, menus, popover, Page bar, status line, Panel, Window) were derived from it. `app/tests/fonts_fit.rs` shapes text in the real font and asserts the estimate is never below the real width at the label and scale sizes (Regular and Medium), so **nothing needed to grow: a Panel is still at least 255 wide and the Window at least 1420 × 833** (ADR 0044). The 15 px notice text wraps and is not laid out from the estimate. The text is more even but looks wider than a sans; the smallest size that stays readable in DM Mono is 12 px (the floor in ADR 0044), so a smaller Window is a matter of dropping to 12, not of a different font.

## Considered and rejected

- **DM Mono for numbers only, system sans for words.** The first trial; the owner chose one family.
- **A bundled UI sans (such as Inter).** Not wanted: one brand family reads as one product.
- **Loading the fonts from the system or the network.** Breaks the identical look and the app never goes online by itself.

## Consequences

- The "→" in the Colour Setup line (and any other glyph DM Mono lacks) falls back to a system font until it is replaced; DM Mono's coverage of the symbols the app draws itself is not needed because those are icons.
- A new chrome text face picks its font through `Face::font()`; a raw family name outside `style.rs` and `fonts.rs` is a mistake.
