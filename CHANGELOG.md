# Changelog

All notable changes to scopecreep are documented here.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and the project follows [Semantic Versioning 2.0.0](https://semver.org/). See [CONTRIBUTING.md](CONTRIBUTING.md) for how versions and entries are decided.

## [Unreleased]

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
