# The Colour Setup comes from Resolve's scripting API and a manual override, not from OFX

The spec planned to read the Colour Setup first from OFX colour management, then from the scripting API, then from a manual override. Stage 0 showed that Resolve 21.2 tells an OFX plugin nothing useful about colour: the host advertises the `Full` style, but the source clip reads `Raw` and the output clip `OfxColourspace_Source`, with no config or display values, in both ACEScct and DaVinci Wide Gamut projects and whichever style the plugin declares. OFX also reports no Resolve page, and no project or timeline name. The scripting API, by contrast, returns every Colour Setup key and Studio versus free, except the ACES version.

## Decision

The Colour Setup comes from, in priority order: the scripting API (Studio, external scripting set to Local), then the Sender's manual override. Every Colour Setup carries its provenance (project or manual). When the ACES version is not readable, the app says it assumes ACES 2.0 unless the user overrides it.

## Consequences

- The app depends on Resolve Studio with external scripting enabled for automatic colour; the free version works only with the manual override.
- Resolve scripting calls made during playback block for minutes and starve Resolve's main thread. The Colour Setup is read while playback is stopped and never from the Sender's render call, ideally by a process outside Resolve.
- The Resolve scripting API is Python and Lua, so the Rust app needs a small helper to read it. Decided in ADR 0006 (ticket #16).
- The Feed's Sender identity cannot be the Resolve page. What it can be is decided in ticket #14.
