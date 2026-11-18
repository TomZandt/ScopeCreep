# Changelog

All notable changes to scopecreep are documented here.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and the project follows [Semantic Versioning 2.0.0](https://semver.org/). See [CONTRIBUTING.md](CONTRIBUTING.md) for how versions and entries are decided.

## [Unreleased]

### Changed

- The Window says less, and says it once. A bar across the top carries the Colour Setup (Timeline → Output, Levels, where it came from, and whether Output Conversion is exact or approximated). Each Panel's title bar shows its name and short tags only for what changes how to read it (a zoom, a mode, "approximated", "unconverted", "Rec.709 assumed", a non-default setting such as "luma weights: match Resolve"), and hovering it shows the full label. The status line at the bottom is the playback rate, dropped frames and why, and the timeline feeding the Window; the Sender, process and version details are in its hover. At 320 pixels wide the rate and the drops stay and the rest drops first (#99, ADR 0007).
- Black bars around the picture (whole rows at the top and bottom or whole columns at the sides, every pixel exactly black) stay black on the Panels that colour the picture: ARRI False Colour, Stops False Colour, Skin Tone and the saturation heatmap. A black pixel inside the picture is coloured as before (#102, ADR 0029).
- The vectorscope is one Panel showing the 100 % and 200 % plots side by side, two cells wide so each plot keeps its size, with one label, one right-click menu and one measurement per frame: colour targets, skin-tone line, trace brightness and soft spread apply to both plots. The Window shows 10 Panels. Settings you saved for the 100 % vectorscope carry over; a layout saved with the two vectorscope Panels is ignored and the default used (#92, ADR 0011, ADR 0012).
- Drag a Panel by its title bar onto another Panel to swap them: each takes the other's cell and span. While you drag, the Panel you would swap with is outlined quietly; letting go anywhere else cancels. The arrangement is remembered. A press on a border between Panels still resizes, and a press on a title bar never does (#97).
- The Window is a layout, with the clean picture as the hero: it spans three columns and two rows at the top left, with the luma waveform and the two vectorscopes beside it and the other Panels below, every cell used. Panels can span several columns and rows. Drag the borders between columns and between rows to resize them (never below the smallest Panel size, never through a Panel that spans them); the layout is remembered, and a grid saved by v0.2.0 is ignored. Each Panel has a thin border and a slim title bar at its top with its label, and the label strip under Panels is gone (#96, ADR 0012).
- The Window shows 11 Panels: the gamut/legal warning, Highlight Neutrals and the CIE plot are out of it for now. Their code and tests are kept, and their saved settings stay in `~/.config/scopecreep/panel-settings`, so they come back if the Panels return (#95).
- Stops False Colour has a horizontal scale and never goes black. The legend is a bar of 17 swatches under the picture, with the stop numbers (-6 to +6, -1/2, 18 %, +1/2) under them and captions over them (Underexposed, Dark skin, Light skin, Overexposed); text is dropped rather than overlapped when the Panel is narrow. The zones are centred on the stop marks (black below -6 1/2 stops, white above +6 1/2) in a new cool-to-warm colour scale, and the grey and +1 stop mode has a horizontal two-swatch legend. When the Timeline Colour Space is not known or cannot be decoded, it paints the Output picture through its display curve (Rec.709 Gamma 2.4 assumed) with a label saying "approximated: display-referred", and only when even that cannot be read does it say so in a message over a neutral background (#93, ADR 0019, ADR 0010).
- The right-click menu is drawn by the new popover component, from the style system's shared parts (chips, segmented control, slider with a number, tooltip, title bar, top and status bars, legend strip), each with hover, pressed, on, off and disabled states. It looks the same as before (#115, ADR 0027).
- The saturation heatmap keeps the picture's shape: the picture is drawn as its Luma in grey and saturation is laid over it in a colour that gets stronger with saturation, from blue to red, so neutral areas stay grey and saturated areas stand out. The default scale is now 75 % so a normal graded picture uses most of it (#100).
- The Window's chrome (menu, labels, scale numbers, status line, borders, gaps) takes its colours and sizes from one set of style tokens named by role, and a test fails if chrome code uses a raw colour or size. Nothing looks different (#114, ADR 0026).
- The Skin Tone Panel reads at a glance: the picture keeps its own colours, and pixels near the skin-tone line are painted solid flat colours (yellow within the tolerance, green or magenta on either side out to six times it); neutral pixels and hues beyond keep their own colour (#101).
- The vectorscopes' outer ring sits at the furthest a full-strength Rec.709 bar reaches (the green and magenta bars, 0.596 in Cb/Cr), so every legal bar is inside it, and the plot is a little bigger to fit it whole.
- The vectorscopes show clipping: where colours go beyond the furthest a full-strength Rec.709 bar reaches, the outer ring turns thicker and brighter at their hue, in their tint, the way the waveform marks out-of-range pixels. It does not depend on the zoom: the 200 % Panel only magnifies, so a colour that merely falls off its edge is not called clipping.
- The vectorscopes read like a phosphor scope, as the waveform does: brightness follows how many pixels land in each spot (relative to the picture's size and the Panel's, so the look holds at any size), stray pixels stay faint instead of a grey haze, and a soft spread of about 1 pixel (right-click "Soft spread", on by default) turns speckle into smooth tone.
- The vectorscopes' outer ring is a hue wheel: each point of it is the colour that lands there, muted so it does not compete with the trace. The inner ring, crosshair and skin-tone line stay neutral grey.
- The skin-tone line on the vectorscopes and the Skin Tone Panel is at 123°, the angle Resolve 21.2's own vectorscope draws its line at (measured, #60), not the 134.5° first assumed. Skin now sits on it: the ColorChecker's dark skin is 2° and light skin 5° from the line.
- The vectorscopes' trace is tinted by the colour that put each spot there (blue sky draws blue, the middle is white), and each of the six colour-target boxes is drawn in its own colour (red, yellow, green, cyan, magenta, blue). The plot, grid and skin-tone line stay neutral grey.

## [0.2.0] - 2026-11-17

Milestone 2: every display in its own Panel. The Window is a grid of 14 Panels, all always visible (ADR 0011).

### Added

- The Window is a grid of Panels: the clean picture and the luma waveform each have their own Panel with its label under it, in an ordered grid sized to the number of displays that fills the Window. Drag the borders between Panels to resize them; the positions are remembered between sessions (`~/.config/scopecreep/panel-grid`). Every Panel shows the same Viewer Frame, uploaded to the GPU once. Labels and scale numbers keep their size at small Panel sizes and draw at the screen's scale on HiDPI screens (#50).

- Right-click a Panel for its settings, remembered per Panel between sessions (`~/.config/scopecreep/panel-settings`). A Panel with no settings shows no menu. The luma waveform offers its luma weights, from the Measured Space's primaries (default) or matching Resolve's own waveform, and its label names the weights in use (#51, ADR 0013).

- A Channel view Panel shows one channel of the Output picture as grey, for inspecting noise: blue by default, red and green from its right-click menu, with the channel named in its label. On it sits the Loupe, a movable square that magnifies the picture under it: 200 % to start (each picture pixel is 2 × 2 screen pixels), the scroll wheel over it sets 100 % to 800 %, drag it anywhere in the Panel, and pick small, medium or large from the menu. Its position, zoom and size are remembered (#55, ADR 0015).
- The luma waveform reads like a phosphor scope: brightness follows how many pixels land in each spot, relative to the Panel and frame size, so dense areas glow, stray pixels stay faint but visible (about 8 %), and the same frame looks the same in a large and a small Panel. New right-click settings: trace brightness (Normal, Dimmer, Brighter) and a soft spread of about 1 pixel (on by default). The out-of-range edge rows are as clear as before (#53, ADR 0017).
- Black and white Panel: the Output picture in grey, each pixel's grey equal to its Luma with the same weights as the waveform, so a grey you see lines up with its trace. It follows the luma waveform's luma weights setting and its label names the weights (#54).

- A gamut/legal warning Panel: the Output picture's Luma in black and white, fading to red from 90 % up to legal white (solid above it) and to blue from 10 % down to legal black (solid below it), with every channel checked so out-of-gamut colours show too. Right-click for the two thresholds and EBU R103 tolerance (-5 % to 105 %); the label names the Levels and any tolerance in use (#56).
- Colour Waveform Panel beside the luma waveform: the same plot, scale, Levels lines and phosphor look, with each spot tinted by the average colour of the pixels counted there, so a green scene gives a green trace. Right-click for trace brightness and soft spread; it follows the luma waveform's luma weights (#59, ADR 0022).
- Highlight Neutrals Panel: the Output picture in grey, with pixels whose red, green and blue are nearly equal shown in green, for checking that whites and greys are balanced. Right-click for the tolerance (3 % by default, from 0.5 % to 10 %); the label says which (#65).

- ARRI False Colour Panel: the Output picture painted by its Luma in the Rec.709 percentage bands third parties publish as ARRI's (purple, blue, green, pink, yellow, red; everything else in grey). Its label says the bands are not ARRI's specification and that scopecreep is not affiliated with ARRI; the right-click menu turns the legend on or off (#57, ADR 0016).
- Stops False Colour Panel: each pixel painted by how many stops it is above or below middle grey, measured in scene light decoded from the Timeline Colour Space with that space's own curve (ACEScct, ACEScc, ACEScg, DaVinci Intermediate; Rec.709 Gamma 2.4 works too, with its label saying the stops are approximated). Right-click it for its mode, full zones (whole stops out to six either side of grey, half-stop zones beside grey) or grey and +1 stop (a black and white picture with 18 % grey green and one stop over pink, for the ColorChecker routine), and for the legend on or off (#58, ADR 0009, ADR 0010).

- Two vectorscope Panels of the Output picture, at 100 % and 200 % (the same data at twice the radius). Cb/Cr use the Measured Space's luma weights, so they agree with the waveform; the trace has the waveform's phosphor-style response. Right-click for trace brightness, 75 % or 100 % colour-bar targets and the skin-tone line on or off. The skin-tone line is drawn at an assumed 134.5°, which is not yet checked against Resolve's own vectorscope; the label says so (#61, ADR 0018).
- CIE chromaticity plot Panel: every pixel placed on the CIE 1931 xy diagram with the spectral locus, the Output Colour Space's gamut triangle and the Timeline Colour Space's (Rec.709, P3-D65, Rec.2020, DaVinci Wide Gamut, ACES AP1, from published primaries). Right-click for trace brightness and to hide the Timeline triangle; the label names both spaces (#62, ADR 0020).
- Saturation heatmap Panel: each pixel of the Output picture coloured from cool (neutral) to warm (very saturated) in a muted, even-brightness palette. Saturation is the distance from neutral on the vectorscope's scale, so 100 % is the vectorscope's ring. Right-click for the scale (the percentage at the warm end) and a legend on or off (#64, ADR 0023).
- Skin Tone Panel: the Output picture in grey, with pixels near the skin-tone line lit yellow, drifting to green or magenta either side and fading back to grey far from skin (and for near-neutral pixels). Only hue counts, not brightness. Right-click for the on-line tolerance (±5° by default, with the fade out to 4 times and grey beyond 6 times that). It uses the same assumed 134.5° line as the vectorscopes; the label says it is assumed (#63, ADR 0021).

### Changed

- The waveform no longer takes the picture's shape and width: it fills its own Panel, so a picture column no longer sits exactly level with its trace column (#50, ADR 0012).

### Fixed

- A long label (a wrapped Skin tone, ARRI or Stops label) no longer spills out of its Panel: the display keeps at least 80 logical pixels and the label is cut off below that. The smallest Panel is now 120 by 110 logical pixels, and saved border positions that would leave a Panel smaller than that are ignored.
- Letting go of the left button while a right-click menu is open now ends a Loupe or border drag, instead of leaving it following the pointer.
- Every display that shows Luma follows the luma waveform's luma weights setting: the gamut/legal warning, ARRI False Colour, Highlight neutrals, both vectorscopes, Skin tone and the saturation heatmap (the vectorscope label no longer always says "from primaries").
- Settings and border positions are saved atomically, and only when something changed (a scroll tick at a Loupe's zoom limit no longer rewrites the file).

### Removed

- The `--luma` command-line option: the luma weights are now the luma waveform's setting in its right-click menu (#51).

### Known limitations

- Resolve's own skin-tone line angle has not been measured: the vectorscopes and the Skin Tone Panel use an assumed 134.5° and say so (#60, ADR 0018).
- Nothing in milestone 2 has been compared by eye with Resolve's own scopes or checked on a real monitor, on HiDPI, or with a real mouse; those checks are open as `ready-for-human` issues (#66, #68-#76, #78-#81).
- Each Panel's own GPU time per UHD frame has not been measured (#82). All 14 Panels together keep Resolve at 50 fps at UHD 50 (#67).
- The CIE plot's spectral locus points are typed from the CIE 1931 table and not yet checked against it (#77). sRGB output is decoded as gamma 2.4 and HLG without its system gamma in the CIE plot; its label says so (ADR 0020).
- Stops False Colour on ACEScg, ACEScc and Rec.709 Gamma 2.4 timelines is not yet checked against Resolve's frames (#79).
- A Panel smaller than the frame is sampled nearest-neighbour in the gamut/legal warning, so a single offending pixel can be missed (ADR 0014).

## [0.1.0] - 2026-11-16

Milestone 1: the Feed from Resolve to the app, the Output Conversion, and the luma waveform.

### Added

- Output Conversion: when the Feed is in the Timeline Colour Space, the app converts it to the Output Colour Space on the GPU with published maths, and the Picture View and the waveform show the converted picture. DaVinci Wide Gamut / Intermediate to Rec.709 Gamma 2.4, P3-D65 (gamma 2.6) and Rec.2100 ST2084, and ACEScct (ACES 2.0) to Rec.709 BT.1886, P3-D65 and Rec.2100 ST2084 (1000 nit), are labelled "Output Conversion exact" ("exact for ACES 2.0" while the ACES version is assumed). A Colour Setup read after playback stops converts the picture already on screen (#17).
- Approximated conversions say why: DWG/DI to Rec.2020 Gamma 2.4 (not yet measured against Resolve), DaVinci tone mapping or gamut mapping on (not applied, unpublished), YRGB with colour spaces other than Rec.709 Gamma 2.4 (#17).
- Accuracy suite on the real Feed: `harness.py feed-frames` captures each test project's frames after the app's own Output Conversion, and `cargo run -p scopecreep-accuracy -- feed` compares them with Resolve's renders and checks the app's label never claims more than was measured (#17).
- Feed protocol v1.0 (`feed/PROTOCOL.md`): the versioned shared-memory hand-over between the Sender and the app, with drop counting, a reader-present flag and version-mismatch reporting. C header for the Sender, Rust crate for the app (#14).
- scopecreep Sender (`sender/`): OFX plugin on Resolve's CUDA path that copies each Viewer Frame into the Feed without ever making Resolve wait, and does no work when no app is attached (#14).
- scopecreep app (`app/`): one window with a Picture View of the clean picture, a quiet label and one-line notices; Viewer Frames chosen by pacing so background cache renders are ignored (#14).
- `feed-synth`, a synthetic Feed writer for testing without Resolve (#14).
- Luma waveform beside the Picture View (two panels): computed on the GPU from every pixel of every Viewer Frame at full resolution in 32-bit float, one waveform column per picture column, on a 10-bit scale (0–1023) with legal reference lines at 64 and 940. Pixels below 0 or above 1023 are counted and shown in their own edge rows, never cut off. A quiet label states the scale, the Measured Space and the luma weights (#15).
- Luma weights derived from the Measured Space's primaries and white point (default), or a Match Resolve mode with the weights Resolve's own waveform uses (`--luma match-resolve`) (#15).
- The Colour Setup travels with the Feed: the app shows it in the Picture View label, e.g. "ACEScct → Rec.709 BT.1886 (from project, assumes ACES 2.0)". It is read from the Resolve project through a scripting helper the app runs under Resolve's own Python (Studio, external scripting set to Local), only while playback is stopped; a changed project setting shows within a few seconds, with nothing restarted. Resolve's "P3-D65" is labelled gamma 2.6. ACES setups say "assumes ACES 2.0" (the version is not readable by script) (#16).
- Manual Colour Setup on the Sender node ("Set Colour Setup manually": Timeline Colour Space, Output Colour Space, Levels, ACES version). It wins over the project's and is labelled "set manually"; it is the way to use scopecreep with Resolve's free version (#16).
- The waveform measures in the Colour Setup's Output Colour Space when it is known (#16).
- When the Colour Setup cannot be read, the label says it is unknown and a notice says how to fix it (external scripting, or the Sender node) (#16).
- The app knows the Resolve project and timeline names (from scripting) for the Sender's identity (#16).
- `--no-scripting`: never read the project by Resolve scripting (#16).
- The waveform follows the project's Levels: with video levels black (0.0) is at code 64 and white (1.0) at 940, with data (full) levels at 0 and 1023. Its label shows the active range ("Legal 64–940" or "Full 0–1023"), followed by "set manually" when the Levels come from the Sender node's manual Colour Setup, or "assumed" (video levels, Resolve's default) while the Colour Setup is not known. The reference lines and scale numbers follow the Levels (#18).
- To compare with Resolve's own waveform on a video-levels project, turn on Video Level Scopes in Resolve's scope options (Resolve's scopes show data levels by default, whatever the project's Levels) (#18).
- `measure_feed` development example: measures live Feed frames with the app's own waveform and prints the codes of chosen columns (#18).
- A quiet status line under the panels: Resolve's live playback rate, measured from the Sender's own render times (within ±0.01 fps of the timeline rate at 24, 25 and 50 fps in Resolve; "paused" when stopped), dropped frames split into "app too slow" and "Sender busy", and the Sender driving the Feed with its project and timeline names when known (#19).
- More than one Sender: exactly one drives the Feed. Senders rendering the same frames are settled by a fixed rule (the instance Resolve created first in its process) and the others are named in a notice; a timeline switch simply follows the Sender now rendering. A second Resolve process joins the Feed as a guest, sends no frames and is flagged; it takes the Feed over when the first Resolve quits (#19).
- `SCOPECREEP_FRAME_LOG=<file>`: a per-frame timing log of what the app read and showed, for diagnostics (#19).
- The Sender node greys out its manual Colour Setup controls (Timeline Colour Space, Output Colour Space, Levels, ACES version) while "Set Colour Setup manually" is off, so it never offers settings that are not in effect; scopecreep then uses the project's Colour Setup, as before. Values set by hand are kept when the switch is turned off and come back when it is turned on again (#46).

### Changed

- The waveform takes the picture's shape: the Picture View and the waveform are the same size, in the Feed frame's aspect ratio (16:9, 2.39:1, 4:3, 9:16...), each letterboxed in its half of the window, so each picture column sits level with its trace. They follow a change of timeline resolution without a restart, and each panel's label sits directly under it (#42).
- The waveform scale's numbers keep their full gutter in a small window; on a short waveform, numbers that would overlap are left out (black and white stay longest) (#42).
- The Picture View label states the Output Conversion and how far it can be trusted, instead of "as sent, no Output Conversion" (#17).
- Feed protocol 1.2: a Sender registry in the header (instances, timeline rate, host, last render) and guest Senders from a second Resolve process. Older readers and writers stay compatible (#19).
- Background cache fills while paused are now recognised at 50 and 60 fps too (they render about 1.6-2x real time, in pairs, from two threads), so the Scopes stay on the viewer frame; a ragged playback start is held back for a moment and then shown (#19).
- Feed protocol 1.1: the slot's Colour Setup block now has a format (provenance plus `key=value` lines). It carries the Sender's manual override; 1.0 readers and writers stay compatible (#16).
- Feed frames go to the GPU once, through reused staging buffers, for the Picture View and the waveform to share: the app now keeps up with UHD float at 60 fps (#15).

### Fixed

- A Colour Setup set by hand on the Sender node no longer claims "Output Conversion exact": it cannot say whether tone or gamut mapping is on, so the label reads "Output Conversion approximated: assumes tone and gamut mapping off" (#17).
- When the Feed is shown as sent (no Output Conversion), the waveform label says "Rec.709 assumed, measured as sent, unconverted" instead of reading like a Rec.709 measurement (#17).
- The Sender no longer allocates graphics memory, pins the Feed's memory or scans Feed files on Resolve's render thread: the ring (when the first frame arrives or the frame size grows) and a second Resolve's look at the first one's Feed are handled by the Sender's own worker thread. The frame that triggers a new ring passes through and is counted as dropped ("Sender busy"); retired rings are freed by the worker even when no more renders arrive (#2).

### Known limitations

- Rec.2100 HLG, P3-DCI and sRGB outputs, ACES output transforms other than the three above, ACES 1.3, ACEScc and timelines other than DWG/DI are not converted yet: the picture is shown and measured as sent, and the label says "as sent, no Output Conversion" and why (#17).
- Performance with the Sender on has not been benchmarked on this release (#20); Stage 0 measured no change in Resolve's playback with the Sender's copy at UHD 23.976-60 fps. Checked after release (2026-11-16, #20): no change at UHD 50 fps with the Sender, or with the Sender and the app.

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

[Unreleased]: https://github.com/TomZandt/scopecreep/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/TomZandt/scopecreep/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/TomZandt/scopecreep/compare/v0.0.0...v0.1.0
[0.0.0]: https://github.com/TomZandt/scopecreep/releases/tag/v0.0.0
