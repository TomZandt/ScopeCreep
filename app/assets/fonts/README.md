# Bundled fonts

Compiled into the app (`app/src/fonts.rs`, ADR 0045, ADR 0046). Nothing is modified except the Lucide subset. The licence texts are beside the files exactly as shipped by their projects; `THIRD-PARTY.md` at the repository root lists them with the full text.

| File | What | Licence | Source | SHA-256 |
| --- | --- | --- | --- | --- |
| `DMMono-Regular.ttf` | DM Mono Regular (all interface text) | SIL OFL 1.1, `OFL-DMMono.txt` | https://raw.githubusercontent.com/googlefonts/dm-mono/main/exports/DMMono-Regular.ttf (identical to https://raw.githubusercontent.com/google/fonts/main/ofl/dmmono/DMMono-Regular.ttf) | `55b4c98f123daebb3ed27947ba47b2af00554fc6284d639a540bcef5e6258ad2` |
| `DMMono-Medium.ttf` | DM Mono Medium (emphasis) | SIL OFL 1.1, `OFL-DMMono.txt` | https://raw.githubusercontent.com/googlefonts/dm-mono/main/exports/DMMono-Medium.ttf | `fd327daf461db87b44a87def475d251bf03b997f7c07d9680592d75dbbfaad0b` |
| `OFL-DMMono.txt` | the licence of DM Mono, with its copyright line "Copyright 2020 The DM Mono Project Authors (https://www.github.com/googlefonts/dm-mono)" | | https://raw.githubusercontent.com/google/fonts/main/ofl/dmmono/OFL.txt (the dm-mono repository carries no separate licence file) | `2bada5ea45c3c63b7f1ea1f88ce9672c9e4f0c42b2c3b7378949084fe55a3066` |
| `Anton-Regular.ttf` | Anton (the wordmark and display headings only) | SIL OFL 1.1, `OFL-Anton.txt` | https://raw.githubusercontent.com/google/fonts/main/ofl/anton/Anton-Regular.ttf | `a4ba3a92350ebb031da0cb47630ac49eb265082ca1bc0450442f4a83ab947cab` |
| `OFL-Anton.txt` | the licence of Anton, "Copyright 2020 The Anton Project Authors (https://github.com/googlefonts/AntonFont.git)" | | https://raw.githubusercontent.com/google/fonts/main/ofl/anton/OFL.txt | `ee67e6ee22790b7929f1a3769ca2801d565c64b5a9096942c1adf5596de9c9e4` |
| `Lucide-subset.ttf` | the icons the UI draws (7 glyphs of Lucide) | ISC, with Feather's MIT notice, `LICENSE-Lucide.txt` | subset of `font/lucide.ttf` from the npm package `lucide-static` 1.77.0 (`npm pack lucide-static@1.77.0`, tarball SHA-256 `04322da4737e0f505646ac2dbbc273c8512b95ec737a462248621a2a32c65da1`; the full font's SHA-256 is `0002d92fcc829cdff63f78717ec1773437a6e55ef512c62d168da41dfeb2bd4b`) | `14a73752366afee4b55b4b5b9903d4a25723aa618490e6097ef0933acc908ad8` |
| `LICENSE-Lucide.txt` | Lucide's licence file (ISC, and the Feather MIT text) | | `LICENSE` in the same package | |

The fonts were checked with `file` to be TrueType and the DM Mono Regular files from the two sources are byte-identical.

## The icon subset

Made with fontTools (installed in a throwaway virtual environment outside the repository). The GSUB, GPOS, GDEF and DSIG tables are dropped (the full font's ligature table does not survive subsetting, and no icon needs layout features), hinting is off, and only these codepoints are kept. The family name inside is `lucide`.

| `components::Icon` | Lucide icon | Codepoint |
| --- | --- | --- |
| `Plus` | plus | U+E13D |
| `ChevronUp` | chevron-up | U+E070 |
| `ChevronDown` | chevron-down | U+E06D |
| `ChevronRight` | chevron-right | U+E06F |
| `Reset` | rotate-ccw | U+E148 |
| `Ellipsis` | ellipsis | U+E0B6 |
| `Close` | x | U+E1B2 |

To add an icon, look its codepoint up in `font/codepoints.json` of the same package, subset again with it added (`Subsetter.populate(unicodes=[...])` with the layout tables removed first), record the new SHA-256 here, and add the variant to `components::Icon`.
