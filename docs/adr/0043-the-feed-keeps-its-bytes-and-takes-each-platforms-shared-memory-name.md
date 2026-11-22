# The Feed keeps its bytes and takes each platform's shared-memory name

The Feed lives at `/dev/shm/scopecreep-feed`, a path that exists only on Linux. ScopeCreep is coming to Windows (NVIDIA, CUDA) and macOS (Apple Silicon, Metal), so the Sender and the app need another way to find the same memory on those platforms without adding any cost to Resolve's render (ADR 0003).

## Decision

- The Feed layout in `feed/PROTOCOL.md` is the same on every platform. Only where it lives differs:
  - Linux: the file `/dev/shm/scopecreep-feed`, as now.
  - macOS: the POSIX shared-memory object `/scopecreep-feed` (`shm_open`).
  - Windows: the named file mapping `Local\scopecreep-feed`, backed by the paging file.
- `SCOPECREEP_FEED` still overrides the location on every platform.
- A platform's Installer goes into a Release only once the UHD 50 performance check passes on that platform (see "Release" in the glossary).

## Considered and rejected

- **An ordinary file in the temp folder on every platform:** one code path, but on macOS and Windows the OS may write the pages to disk, which puts the Sender's never-wait budget at risk.
- **A different protocol per platform:** three things to keep in step for no gain, since the bytes are already language and OS neutral.

## Consequences

- `feed/PROTOCOL.md`, the C header and the Rust crate name the location per platform. The harness's Feed reader follows them.
- Decided by the product owner on 2026-11-22 while shaping the release-pipeline, Windows and macOS specs.
