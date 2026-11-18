# scopecreep

Real-time, professional-grade video scopes for DaVinci Resolve, on their own screen.

scopecreep takes the picture Resolve is playing (the **Feed**) and shows it next to **Scopes** (luma and colour waveform, vectorscopes, CIE chromaticity) and **Picture Views** (clean picture, channel view with a Loupe, black and white, gamut warning, false colour, skin tone, saturation, neutrals) in a separate app. The goals:

- **Accurate.** Every pixel is measured at full resolution in 32-bit float, in the project's actual colour space. Anything that isn't exact is labelled as approximated.
- **Invisible.** Turning scopecreep on must not change Resolve's playback frame rate.
- **Cross-platform.** Windows, Linux and macOS, on Resolve 21.x.
- **Quiet.** A tool, not a sales pitch: no clutter, and sensible defaults read from the Resolve project.

## Status

Pre-alpha. Nothing to install yet.

| Stage | What | State |
|---|---|---|
| 0 | Colour and performance checks against Resolve (throwaway prototypes) | Done ([report](docs/stage0-report.md)) |
| 1 | Feed plus clean picture plus luma waveform (Linux, NVIDIA) | Released in v0.1.0 |
| 2 | Every display in its own Panel: 14 Panels (waveforms, vectorscopes, CIE plot, false colour, skin tone and more) | Released in v0.2.0 |
| 3 | A colourist's Window: the picture as the hero, arranged and saved layouts, title bars and one Colour Setup line, a style system with Panel toolbars and sliders, stronger Skin Tone and heatmap, following the scrub | Planned: specs [#94](https://github.com/TomZandt/scopecreep/issues/94), [#113](https://github.com/TomZandt/scopecreep/issues/113), [#83](https://github.com/TomZandt/scopecreep/issues/83), [#106](https://github.com/TomZandt/scopecreep/issues/106); backlog [#119](https://github.com/TomZandt/scopecreep/issues/119); checks at Resolve [#120](https://github.com/TomZandt/scopecreep/issues/120) |
| Later | Parade, histogram, HDR, Setup Checklist, Capture, Windows/macOS, installers, licensing | Decided, parked |

## Development (Linux, NVIDIA)

| Part | Where | What |
|---|---|---|
| Feed protocol | `feed/PROTOCOL.md`, `feed/include/scopecreep_feed.h`, `feed/` crate | The shared-memory ring between Sender and app: spec, C header (Sender), Rust reader and synthetic writer (app, tests) |
| Sender | `sender/` | C++ OFX plugin, Resolve's CUDA render path, async pinned copy into the Feed |
| App | `app/` | Rust + wgpu: one window, a grid of 14 Panels (Scopes and Picture Views) all showing the latest Feed frame (GPU, every pixel, full resolution) |

```sh
cargo test                       # Feed protocol (Rust and C against each other) and the app, driven at the Feed seam (waveform tests need a GPU)
cargo run -p scopecreep          # the app (reads /dev/shm/scopecreep-feed, or $SCOPECREEP_FEED); right-click a Panel for its settings (the luma waveform's luma weights, for one); --no-scripting to never read the project by Resolve scripting
cargo run -p scopecreep-feed --bin feed-synth   # a moving test card into the Feed, no Resolve needed

make -C sender                   # build the Sender (needs /opt/cuda and Resolve's OpenFX headers)
make -C sender test              # drive the built Sender with a fake CUDA host (needs the GPU, not Resolve)
make -C sender install           # developer install into /usr/OFX/Plugins, then restart Resolve
```

In Resolve, put **scopecreep Sender** (Effects > OpenFX > scopecreep) on its own last timeline node, with no grade on that node. GPU processing must be CUDA.

### Running the app from the app launcher

The launcher entry (`~/.local/share/applications/scopecreep.desktop`) runs a copy of the app installed outside the repo, so cleaning build outputs (`cargo clean`) never breaks it:

```sh
cargo build --release -p scopecreep && install -Dm755 target/release/scopecreep ~/.local/bin/scopecreep
```

The installed copy doesn't update itself: run the line above again after changing the app. The app shows a picture only while a Sender is playing in Resolve.

The `scopecreep-test-*` projects (in Resolve's `scopecreep` project folder) carry the Stage 0 probe on their timeline node. To use them, install the Sender with `make -C sender PROBE_ALIAS=1 install`, which answers to the probe's identifier, and restart Resolve. The plain `make -C sender install` build doesn't load in those projects.

## How it works

```
DaVinci Resolve                                 scopecreep app (Rust + wgpu)
┌──────────────────────────────┐               ┌──────────────────────────────┐
│ timeline node tree           │   the Feed    │ Output Conversion (if needed)│
│  … grades → [Sender (OFX)] ──┼──────────────►│   ├─► Picture View           │
│            32-bit float      │ shared memory │   └─► Scopes (GPU compute)   │
└──────────────────────────────┘               └──────────────────────────────┘
```

- **Sender:** a small C++ OFX plugin placed on the last timeline node. It hands each rendered frame to scopecreep without ever making Resolve wait.
- **App:** reads the Feed, converts it from the Timeline Colour Space to the Output Colour Space when required, and draws the Scopes and Picture Views on the GPU.
- **Colour Setup:** read from the project by Resolve's scripting API (Studio, external scripting set to Local) while playback is stopped, with a manual override on the Sender node (ADR 0005, ADR 0006).

## Documentation

- [GLOSSARY.md](GLOSSARY.md): the project's vocabulary. Use these terms in code, issues and docs.
- [docs/adr/](docs/adr/): architecture decisions and why they were made.
  - [0001](docs/adr/0001-ofx-sender-with-output-conversion.md): the Sender is the primary Feed Source; scopecreep does the Output Conversion
  - [0002](docs/adr/0002-cpp-sender-rust-app.md): C++ Sender, Rust + wgpu app
  - [0003](docs/adr/0003-resolve-never-waits.md): Resolve never waits for scopecreep
  - [0004](docs/adr/0004-luma-follows-measured-space.md): luma weights follow the Measured Space
  - [0005](docs/adr/0005-colour-setup-from-scripting-api.md): the Colour Setup comes from Resolve's scripting API and a manual override
  - [0006](docs/adr/0006-colour-setup-helper.md): the app reads the Colour Setup through a helper it owns; only the manual override travels in the Feed
  - [0007](docs/adr/0007-labels-claim-only-what-is-proven.md): labels claim "exact" only for what has been proven
  - [0008](docs/adr/0008-manual-colour-setup-does-not-mirror-the-project.md): the Sender's manual Colour Setup controls don't mirror the project
  - [0009](docs/adr/0009-exposure-in-scene-light-clipping-on-output.md): exposure in stops is measured in scene light; clipping and legal checks on the Output picture
  - [0010](docs/adr/0010-false-colour-schemes-are-not-licensed-or-named.md): third-party false colour schemes are modelled, not licensed, and never claimed
  - [0011](docs/adr/0011-every-display-has-its-own-panel.md): every display has its own Panel, always visible, in a fixed grid
  - [0012](docs/adr/0012-panel-grid-shape-follows-display-count.md): the Panel grid's shape follows the number of displays, not the window, and Panels fill their cells
  - [0013](docs/adr/0013-panel-settings-are-kept-by-display-name-and-offered-as-choices.md): Panel settings are lists of choices, kept by display name in a text file, in a menu the app draws itself
  - [0014](docs/adr/0014-gamut-warning-judges-every-channel-in-fractions-of-the-levels-range.md): the gamut/legal warning judges every channel, in fractions of the Levels' range
  - [0015](docs/adr/0015-display-state-is-remembered-beside-the-settings.md): what a display moves by hand (the Loupe) is remembered beside the settings, not as a setting
  - [0016](docs/adr/0016-arri-style-band-edges-and-out-of-gamut-luma.md): ARRI-style bands take Luma as the picture has it, include lower edges, and have open ends
  - [0017](docs/adr/0017-waveform-brightness-is-relative-density-with-a-phosphor-response.md): the waveform's brightness is density relative to the Panel and frame, with a phosphor response and a separate soft spread pass
  - [0018](docs/adr/0018-vectorscope-skin-tone-angle-is-assumed-and-shared.md): the vectorscope skin-tone line's angle sits in one shared constant (first assumed at 134.5°, now measured from Resolve at 123°)
  - [0019](docs/adr/0019-stops-false-colour-decodes-the-timeline-frame-as-it-draws.md): Stops False Colour takes the Timeline Colour Space frame and decodes it as it draws
  - [0020](docs/adr/0020-cie-plot-reads-the-output-encoding-as-light.md): the CIE plot reads the Output encoding as light, decoded by the Output Colour Space's name
  - [0021](docs/adr/0021-skin-tone-tolerance-is-choices-scaled-from-one-width.md): the Skin Tone Panel's tolerance is a list of choices that scale one width, and its tints fade through each other
  - [0022](docs/adr/0022-colour-waveform-tint-is-the-average-colour-at-the-luma-waveforms-positions.md): the Colour Waveform is the luma waveform's counts tinted by the average colour of the pixels counted at each spot
  - [0023](docs/adr/0023-heatmap-saturation-is-a-share-of-the-vectorscope-ring.md): the saturation heatmap's 100 % is the vectorscope's ring (Cb/Cr 0.6 since the ring moved), not each colour's own bar
  - [0024](docs/adr/0024-the-last-sender-in-the-frame-drives-the-feed.md): the last Sender in the frame drives the Feed, and more than one active Sender is a warning
- [docs/findings/](docs/findings/): what testing showed, one file per ticket.
- [docs/research/](docs/research/): what public sources say, with dates and links.

## Licence

Proprietary. All rights reserved.
