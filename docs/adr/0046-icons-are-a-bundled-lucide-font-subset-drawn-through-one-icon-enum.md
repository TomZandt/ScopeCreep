# Icons are a bundled Lucide font subset, drawn through one Icon enum

Date: 2026-11-22 (spec #196, #238). Notes on ADR 0026 and ADR 0027. See ADR 0026, ADR 0027, ADR 0044, ADR 0045.

The app's symbols were text glyphs in whatever font the machine fell back to (the ▲ ▼ arrows, the ↺ reset, a "+") or small rectangles (the settings button's three dots). DM Mono (ADR 0045) has none of the arrows, so they needed a home: one set, one place, the same on every platform.

## Decision

- The icon set is **Lucide** (ISC licence; its licence file also carries the Feather MIT notice for the icons derived from Feather, and both texts are shipped).
- It is bundled as an **icon font**, **subset** to only the glyphs the UI uses (about 2 KB, from the 1 MB `lucide-static` font), at `app/assets/fonts/Lucide-subset.ttf`. The README beside it records the source (the `lucide-static` npm package, version and SHA-256), the SHA-256 of the subset, the `pyftsubset`-style steps and the table of icon name to codepoint.
- `components::Icon` is the one place that maps an icon to its codepoint (`Plus`, `ChevronUp`, `ChevronDown`, `ChevronRight`, `Reset`, `Ellipsis`, `Close`). `Icon::draw(area, colour)` returns an ordinary text prim in `Face::Icon` (16 logical px, never smaller; a token, `TYPE_ICON`), so it is tinted by text colour and gets the usual hover colours from the control that draws it. Icons are in: the value box's arrows and the reset arrow, the edge +, the settings button.
- A test (`fonts_fit.rs`) shapes every `Icon` and fails if its codepoint is not in the subset or draws as a fallback or `.notdef`.

## Why a font and not SVG

The renderer already draws text through glyphon: an icon as a glyph needs no new pipeline, scales and aligns like text, takes its colour from the text colour, and costs two kilobytes. SVG would need a rasteriser, a cache per size and scale, and its own tinting.

## How to add an icon

Pick the name on lucide.dev; look its codepoint up in `font/codepoints.json` of the same `lucide-static` version; add it to the subset (the README lists the exact command) and to the README table; add a variant to `Icon` and to `Icon::ALL` and `Icon::glyph()`. The test then checks it.

## Consequences

- Only the glyphs listed are in the font; a codepoint outside it draws nothing useful, which the test catches.
- Updating Lucide is a re-subset and a new SHA-256 in the README.
