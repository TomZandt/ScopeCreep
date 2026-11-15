# Changelog

All notable changes to scopecreep are documented here.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and the project follows [Semantic Versioning 2.0.0](https://semver.org/). See [CONTRIBUTING.md](CONTRIBUTING.md) for how versions and entries are decided.

## [Unreleased]

### Added

- Feed protocol v1.0 (`feed/PROTOCOL.md`): the versioned shared-memory hand-over between the Sender and the app, with drop counting, a reader-present flag and version-mismatch reporting. C header for the Sender, Rust crate for the app (#14).
- scopecreep Sender (`sender/`): OFX plugin on Resolve's CUDA path that copies each Viewer Frame into the Feed without ever making Resolve wait, and does no work when no app is attached (#14).
- scopecreep app (`app/`): one window with a Picture View of the clean picture, a quiet label and one-line notices; Viewer Frames chosen by pacing so background cache renders are ignored (#14).
- `feed-synth`, a synthetic Feed writer for testing without Resolve (#14).
- Luma waveform beside the Picture View (two panels): computed on the GPU from every pixel of every Viewer Frame at full resolution in 32-bit float, one waveform column per picture column, on a 10-bit scale (0–1023) with legal reference lines at 64 and 940. Pixels below 0 or above 1023 are counted and shown in their own edge rows, never cut off. A quiet label states the scale, the Measured Space and the luma weights (#15).
- Luma weights derived from the Measured Space's primaries and white point (default), or a Match Resolve mode with the weights Resolve's own waveform uses (`--luma match-resolve`) (#15).
- Known limitation: until the Colour Setup reaches the app (#16, #17) the waveform measures the values as sent and assumes Rec.709; its label says "Rec.709 assumed" (#15).

### Changed

- Feed frames go to the GPU once, through reused staging buffers, for the Picture View and the waveform to share: the app now keeps up with UHD float at 60 fps (#15).

## [0.0.0] - 2026-11-15

Stage 0: colour and performance checks. Nothing here is the product yet; it is the evidence the product is built on.

### Added

- Throwaway probe Sender (`prototype/probe-sender/`): an OFX plugin that loads in Resolve 21.2 Studio, logs what Resolve tells it, dumps frames as float EXR, and copies UHD float frames out over CUDA into a shared-memory ring.
- Resolve test harness (`harness/`): builds a test project per Colour Setup, renders reference frames, reads the Colour Setup by script, drives playback and screenshots, clones the hand-placed Sender node to every project.
- Accuracy tool (`accuracy/`): compares the published colour maths with Resolve's own renders and reports each Colour Setup as exact or approximated.
- Stage 0 report (`docs/stage0-report.md`) and findings for every check (`docs/findings/`).
- ADR 0005: the Colour Setup comes from Resolve's scripting API and a manual override, not OFX.
- Glossary terms Viewer Frame and the Match Resolve luma mode.
- This changelog, Conventional Commits, Semantic Versioning and the findings convention (`CONTRIBUTING.md`).

### Changed

- ADR 0001: published maths is the Output Conversion method; the hidden branch is parked.
- ADR 0002: the Sender must never link OpenEXR or Imath.
- ADR 0003: "Resolve never waits" is proven at UHD float, 23.976 to 60 fps, on the CUDA path.
- ADR 0004: Resolve's waveform uses Rec.709 weights for P3; offered as a Match Resolve mode.

[Unreleased]: https://github.com/TomZandt/scopecreep/compare/v0.0.0...HEAD
[0.0.0]: https://github.com/TomZandt/scopecreep/releases/tag/v0.0.0
