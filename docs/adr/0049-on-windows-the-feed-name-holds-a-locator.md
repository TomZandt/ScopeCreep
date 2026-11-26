# On Windows the Feed's name holds a locator, and each Feed is its own mapping

Date: 2026-11-26 (#248, spec #235). Refines ADR 0043.

ADR 0043 puts the Feed on Windows in the named file mapping `Local\scopecreep-feed`. On Linux the Sender replaces a Feed (a new ring, or Resolve restarting) by renaming a new file over the old name while the app still maps the old one. A Windows named mapping cannot do that: it cannot be renamed, and its name stays taken, pointing at the old Feed, as long as any process (the app) holds it. Like a house number that can't be given to a new house while anyone still lives in the old one, so the street gets a signpost instead.

## Decision

- On Windows `Local\scopecreep-feed` (or the `SCOPECREEP_FEED` override) is a **locator**: one page holding the number N of the Feed now published and the last N handed out.
- Feed N is the mapping `Local\scopecreep-feed.N`, with the protocol's layout unchanged. Publishing a Feed is storing its N in the locator; a reader sees a replaced Feed when the locator's N changes, as Linux sees a new inode.
- Every process using a Feed keeps the locator open, so it lives as long as any Feed is in use. When the last process lets go, both are gone: an app started after Resolve quit finds no Feed, where on Linux it finds the stale file.
- The protocol version stays 1.2: no field moves or changes meaning, and Linux is untouched. `feed/PROTOCOL.md`, "Windows", is the specification.

## Considered and rejected

- **Waiting for the reader to let go before reusing the name:** the Sender would wait on the app, breaking the rule that nobody waits.
- **A fresh name per Feed with no locator, the app searching for the newest:** a search on every open, and no single place that says which Feed is current.

## Consequences

- The Sender's C header (`scfeed_os_*`) and the Rust crate (`feed/src/platform/windows.rs`) implement the locator the same way; the harness's Feed reader must too when it runs on Windows.
