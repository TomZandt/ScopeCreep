# The brand accent is lime and the chrome is Hairline: grey, with the accent only as thin lines and text

Date: 2026-11-22 (spec #196, #226). Supersedes the colour parts of ADR 0026's look (the old grey "accent" and the ring round a focused Panel) and ADR 0027's chip look; notes on ADR 0027 and ADR 0042. See ADR 0026, ADR 0027, ADR 0042.

ScopeCreep is a professional colour grading tool: the chrome around the picture must stay strictly neutral, and a brand colour must be small and away from the picture. The product owner tried three variants in an interactive demo and chose B, Hairline.

## Decision

- **Accent colour.** The default is #d2ff00. It is a user option in Settings > Appearance > App accent colour: six swatches, a hex box (it behaves as the value box of ADR 0027: a click types, Enter accepts, Escape cancels, a bad hex is refused and the accent stays), Reset, and a notice that says when the contrast of the accent as text on a chip is under 4.5:1. It is remembered in its own file, `~/.config/scopecreep/accent`, and read as unchecked input: a missing, oversized (over 1 KiB), unreadable or other-version file, or a value that is not `#rrggbb`, is the default. The pressed accent is derived (a darker share of it); there is no hover accent.
- **Hairline.** Chrome greys stay neutral (equal R, G, B). The accent is only a thin line or text, never a fill, never over the picture and never on measurement data: an on chip is accent text and a 2 px accent line at its foot on a slightly lighter grey (R, G, B, Y chips keep their colours); the page tab and a tab group's tab are accent text and underline; the focused Panel has a 2 px accent line along the top of its title bar (no frame round the picture); the slider is a 2 px accent track with a grey thumb; a chosen menu row has a small accent square and a hovered one a 2 px accent bar at its left; the dock overlay is a dashed accent outline over a faint grey wash, with the words "Split left", "Join as a tab", "No room to split: join as a tab" or "Swap places"; the open menu title has the accent line at its foot.
- **Hover is neutral, never the accent**: a lighter grey with white text, so hover never reads as on. Pressed is a darker grey. The edge + is a neutral 24 px grey. The title bar's rule is grey (#5c5c5c), 2 wide, with 20 of air each side.
- **Spacing and readability** come from the demo's tokens (logical px): menu bar 32, page bar 36, status row 26, title bar 34, chip height 24 (the least hit target), chip padding 9, chip gap 2, group gap 8, name to chips 12, view block gap 4, settings button 28 wide with a gap of 8, bar padding 12, gap between Panels 8, menu row 30, popover amount row 34 and width 400. Text is 13 with a floor of 12. Tag and scale text is TEXT_MUTED 150 (it was 112, about 3.9:1) which reads at about 5:1 on the Window. Where the demo has tabular numbers, the app's font does not offer them to the text renderer yet, so readouts and value boxes are right-aligned or centred in a fixed slot instead.
- **Tokens by role** in `style.rs`: `accent()`, `accent_text()`, `accent_pressed()`, the on and pressed chip greys, the rule, the dock wash. The current accent is held per thread (the Window draws on one thread; a test that sets one cannot disturb another) in `accent.rs`. The style check stays green and `accent.rs` and `appearance_panel.rs` hold no chrome literal.
- **Notes on ADR 0042 and ADR 0027.** The view block is on picture Panels only, never on a Scope (ADR 0042 had it on every Panel, greyed on scopes). The divider is a full rule, 2 wide, in the title bar's height and only where the Panel has chips. The chips give way first: whole settings leave from the right and a "+N" chip says how many chips are now in the Panel's settings; a click on it opens that settings popover, where those chips' choices are. Then the tag goes, then extra tabs fold into "+N", then the name is cut. The view block and the settings button never go.
- **Least Panel width** is the bar's padding (24), the name cut to 44 plus its gap of 12, the view block (about 139) and the settings button with its gap (36): 255 logical pixels (it was 176). The Window's least size is **1420 × 833** (it was 1015 × 645), and it opens at 1440 × 840. The "+N" chip shows where it also fits beside the rule and the least name, which is a title bar of about 339; a Panel between 255 and that has no room for it, so its chips are only in its settings popover.

## Considered and rejected

- **A (Signal, accent fills) and C (tinted greys).** A puts a lime block beside the picture; C tints the surround a colourist judges the picture against.
- **A least width that holds "+N" and the rule (331).** It makes the default layout need a Window of about 1776 wide, which does not fit small displays.
- **The accent for hover.** It made a hovered chip read as on.

## Consequences

- Checks for a person (queue #219): the lime against the false colour legends (Stops +½ to +1 stop, Arri's green band) at the app, and the look at UHD.
- The Window's least size and the Panel's least width are quoted in the changelog and the README; tests derive them from the tokens.

## Note (#238)

The tabular numbers this ADR could not give the text renderer are now there: the interface font is DM Mono, whose digits are all one width (ADR 0045). The text sizes and the least sizes above are unchanged.
