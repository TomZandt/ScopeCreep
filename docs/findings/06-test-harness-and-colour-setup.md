# #6 Stage 0: Resolve test harness and Colour Setup by script

> Copied from the GitHub issue comments on TomZandt/scopecreep#6 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-13)

Done on branch `integration/stage0-milestone1` (`harness/`; run `python3 harness/harness.py <cmd>`: `all`, `projects`, `render`, `dump`, `play`, `stop`, `goto TC`, `shot`, `clean`). Generated data lives in `~/scopecreep-testdata/` (not in git).

**Check 2 (Colour Setup by script): yes.** With external scripting = Local, `IsStudio()`, product name and every Colour Setup key are readable via `Project.GetSetting()` / `Timeline.GetSetting()` (timeline values equal project values). Keys: `colorScienceMode`, `colorSpaceTimeline(Gamma)`, `colorSpaceOutput(Gamma)`, `colorSpaceOutputToneMapping`, `colorSpaceOutputGamutMapping`, `outputDRT`, `inputDRT`, `colorAcesODT`, `colorAcesIDT`, `colorAcesGamutCompressType`, `videoDataLevels`, `timelinePlaybackFrameRate`.

| Project | colorScienceMode | Timeline | Output | Tone map | ACES ODT |
|---|---|---|---|---|---|
| yrgb-rec709g24 | davinciYRGB | Rec.709 Gamma 2.4 | Rec.709 Gamma 2.4 | None | - |
| dwg-rec709 | davinciYRGBColorManagedv2 | DaVinci WG/Intermediate | Rec.709 Gamma 2.4 | None | - |
| dwg-p3d65 | davinciYRGBColorManagedv2 | DaVinci WG/Intermediate | P3-D65 | None | - |
| dwg-pq-tmoff / tmon | davinciYRGBColorManagedv2 | DaVinci WG/Intermediate | Rec.2100 ST2084 | None / DaVinci | - |
| acescct-rec709 / p3d65 / pq | acescct | n/a | n/a | n/a | Rec.709 BT.1886 / P3-D65 / Rec.2100 ST2084 (1000 nit) |

All: `videoDataLevels`='Video'. Full dumps: `~/scopecreep-testdata/dumps/`.

**Verified:** idempotent one-command rebuild of all projects (all `scopecreep-test-*`: 8 setups + `perf-uhd`); one-command reference render (float EXR + 16-bit TIFF, frame 1 of each clip, in Output Colour Space; YRGB EXRs bit-identical to source incl. out-of-range); playback start/stop (keystrokes via Hyprland `hl.dsp.send_shortcut`), playhead move, window screenshots.

**Not automated / findings**
- ACES version can't be set or read by script (`acesVersion` not exposed). Resolve's default is **ACES 2.0** (seen in Project Settings for acescct-pq). No ACES 1.3 project exists yet; it needs the UI.
- Timeline-level node can't be created by script (timeline graph has 0 nodes) -> #7 stays manual.
- UI clicks via xdotool don't register in Resolve; keystrokes do. A pending modal dialog blocks the scripting API.
- Resolve only renders under its Media Storage root (`~/Videos`); the harness renders there and moves frames out.
- TIFF reference frames exist but are not decoded yet (#10). Playback rate/drops not measured yet (#12/#20).


