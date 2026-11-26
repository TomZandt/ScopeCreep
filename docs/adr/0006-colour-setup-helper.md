# The app reads the project's Colour Setup through a helper it owns; only the manual override travels in the Feed

ADR 0005 decided the Colour Setup comes from Resolve's scripting API, then the Sender's manual override, and left open how the Rust app reaches a Python/Lua API without ever calling it during playback.

## Decision

- **A helper process owned by the app.** The app starts a small Python script (`app/resolve/colour_setup_helper.py`, built into the app) under Resolve's own Python (`/opt/resolve/bin/ResolvePython`, overridable with `SCOPECREEP_RESOLVE_PYTHON`). It talks over stdin/stdout: the app writes `read`, the helper answers with `key=value` lines (Resolve's own setting keys, the project and timeline names, Studio or not). It calls Resolve only when asked. It is started on the first request, killed after a read that takes over 5 s, dies with the app (`PR_SET_PDEATHSIG`) and inherits no descriptors but its pipes. Windows: ADR 0052. The Sender plays no part: it stays free of Python, sockets and anything that could block its render call.
- **The app decides when to ask, from the Feed.** A read is requested only while a Sender's Feed is attached and the Sender has rendered nothing for 1 s (Feed header `last_render_ns` and frame times; the moment of attaching counts as a render, since an identity Sender is not rendered at all). While stopped, the project is read every 2 s, so a changed setting shows within about 3 s without restarting anything. With no Sender there are no reads: nothing to label, and no way to tell whether Resolve is playing. If the Sender renders while a read is in flight (playback started under it), the next read waits for 10 s of quiet, so a render stall in playback is not taken for a stop.
- **Only the manual override travels in the Feed.** Feed protocol 1.1 gives the slot's reserved Colour Setup block a meaning: a provenance and `key=value` text. The Sender writes it only when "Set Colour Setup manually" is on (provenance manual); otherwise it writes none, and the app uses the project's. The app resolves the two per picture: manual wins and is labelled "set manually", else the project's, labelled "from project".
- **ACES version.** Not readable by script (Resolve 21.2), so an ACES setup from the project always says "assumes ACES 2.0". The manual override has an ACES version choice.
- **Sender identity.** The project and timeline names come from the helper and are kept in the app's status, next to the Feed's own identity (process, host, instance).

## Why not put the project's Colour Setup in the Feed too

Think of the Sender as a courier that labels each parcel. It can only write on the label what it knows, and it knows only what the user set on its node. The project's Colour Setup would have to be handed to the courier by the app, written on the parcel, and read back by the app: a round trip that adds a third writer to the Feed and a file the Sender must read from its render path. It would also be wrong at exactly the moment that matters: a colourist changes a setting while stopped, Resolve re-renders before the helper has read the change, and the frame on screen carries the old setup until the next render. And a future Capture Feed Source has no Sender at all. So the project's Colour Setup goes straight from the helper to the app. Provenance 1 ("project") stays defined in the protocol for a Sender that could learn it.

## Consequences

- Automatic colour needs Resolve Studio with external scripting set to Local; otherwise the app says the Colour Setup is unknown, why, and offers the Sender node's manual override.
- A read collides with playback only if playback starts during its ~10 ms. In Resolve testing such a read blocked until playback stopped (7.2 s) without slowing playback; the 5 s timeout kills the helper and the collision rule stops a second read.
- Reads are not gated when playback renders nothing for the Sender (Fusion page, Shift-D, a disabled Sender node): the app cannot tell that apart from a stop.
- Manual override choices are saved by Resolve as indexes: options may only be appended.
