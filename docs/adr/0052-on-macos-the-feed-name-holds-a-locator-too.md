# On macOS the Feed's name holds a locator too

Date: 2026-11-27 (#254, spec #236). Refines ADR 0043, follows ADR 0049.

ADR 0043 puts the Feed on macOS in the POSIX shared-memory object `/scopecreep-feed`. Linux replaces a Feed by renaming a new file over the old name. macOS has no way to rename a shared-memory object, and no way to give one a second name: like a street where a house number can only go to a new build by demolishing the old house first, a reader opening the name in between would find nothing, or a half-built Feed.

## Decision

- On macOS `/scopecreep-feed` (or the `SCOPECREEP_FEED` override) is a **locator**, the same page as on Windows (ADR 0049): the number N of the Feed now published and the last N handed out. Feed N is the object `/scopecreep-feed.N`, with the protocol's layout unchanged. Publishing is storing N in the locator; a reader sees a replaced Feed when N changes.
- Objects last until unlinked, which a Windows mapping does not, so publishing also unlinks the Feed it replaces. A reader that has it mapped keeps it. The locator stays, as a Linux Feed file stays after a crash; the reader's Sender-alive check tells it is stale.
- The code has the same shape as Windows', in `feed/src/platform/macos.rs`. The protocol version stays 1.2. `feed/PROTOCOL.md`, "Locator", is the specification for both.
- macOS shared-memory names hold 31 bytes, so a `SCOPECREEP_FEED` name must be short enough for `.N`.

## Considered and rejected

- **Unlinking the name and making the new Feed at it:** a reader opening between the two sees no Feed or one still being filled, and the old Feed could not be marked retired.
- **Copying the new Feed's bytes into a fresh object at the final name:** a full ring is hundreds of megabytes copied while the Sender runs, and the writer's mapping of the new Feed would not be the published object.

## Consequences

- The Sender's C header needs the same locator code on macOS (the Sender's Metal path, #255), and `feed/tests/c_interop.rs` runs on Linux only until it has it.
- Amended in the review of spec #236: the Sender's C header has the macOS locator code since #255, so `c_interop.rs` runs on Linux and macOS. This ADR first said Linux only.
- The harness's Feed reader follows the locator on macOS as on Windows.
