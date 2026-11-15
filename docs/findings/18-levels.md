# #18 The waveform follows the project's Levels

What ticket #18 learned about Levels (Project Settings › Master Settings › Video Monitoring › Data levels: Video or Full) in DaVinci Resolve 21.2.0.6 Studio on Linux, with the real Sender (probe-alias build) on the timeline node. Every Resolve step ran under the shared `~/scopecreep-testdata/resolve.lock`.

## What Levels change in Resolve, and what they don't

- **The values Resolve hands a plugin do not change with Levels.** The same `ramp` frame read from the Feed in `scopecreep-test-yrgb-rec709g24` (video levels) and `scopecreep-test-yrgb-rec709g24-full` (data levels) gave bit-identical samples: 0.0, 0.25013027, 0.50026053, 0.7503908 and 1.0 at columns 0, 480, 960, 1440 and 1919 of row 100.
- **Renders do not change either:** the reference EXRs of the two projects are identical, and the 16-bit TIFFs differ only in 4 bytes (file metadata).
- This matches Resolve's manual (chapter "Data Levels, Color Management, and ACES"): inside Resolve all image data is full-range 32-bit float; the project's Data levels setting "only affects the data levels being output via the video interface", video levels being 10-bit 64–940 and full 0–1023 (the manual also says 4–1023 for full, the SDI-legal limit).
- **Resolve's own waveform ignores the project's Levels by default.** Its 10-bit scale shows data levels "regardless of the Video/Data Level setting" (manual, "Video Scope Measurement Using Scales"); a "Video Level Scopes" option in the scope's Waveform Scale Style submenu (the three dots menu) rescales the display to video levels. Measured: with the default Data Level scale (Extents off), the 0..1 ramp reads codes 2.4 / 258.0 / 513.7 / 769.3 / 1025.0 at 0, ¼, ½, ¾ and the full picture width in **both** projects (identical fits).

So the Levels decide only which codes the delivered values are. The app maps value v to code `v × 876 + 64` (video levels: black 64, white 940) or `v × 1023` (data levels: black 0, white 1023).

## What the app does

- The waveform's label shows the active range: `10-bit Legal 64–940` or `10-bit Full 0–1023`, with `set manually` when the Levels come from the Sender node's manual Colour Setup, and `assumed` (video levels, Resolve's default) while no Colour Setup is known.
- The brighter reference lines mark black and white for the Levels in use (64 and 940, or the plot's edges 0 and 1023); the scale numbers are 940 / 512 / 64 for video levels and 768 / 512 / 256 for data levels.
- Values past code 0 or 1023 still go to the edge rows: with video levels that is below −0.0731 or above 1.0947.

## Tests

- At the Feed seam (`app/tests/waveform_seam.rs`, synthetic frames): video levels put 0.0, 1.0, 0.5, 0.25, −0.0731 and 1.0947 on codes 64, 940, 502, 283, 0 and 1023, and −0.08 / 1.1 in the edge rows; data levels put 0.0, 1.0, 0.25 and 0.5005 on 0, 1023, 256 and 512. The reference lines move with the Levels.
- `app/tests/colour_setup_seam.rs`: the waveform's Levels follow the project's `videoDataLevels` as the scripting helper reads it, and the Sender's manual override, labelled "set manually".

## Against Resolve

A new harness project, **`scopecreep-test-yrgb-rec709g24-full`**, is the data-levels twin of `yrgb-rec709g24` (setup key `yrgb-rec709g24-full`, YRGB Rec.709 Gamma 2.4, `videoDataLevels=Full`). It was built with `harness.py projects/dump/render --only yrgb-rec709g24-full`, then given the Sender timeline the way `clone-sender` does (the `dwg-p3d65` .drp, colour setup checked against the dump, EXR renders identical to the reference); the probe-log check was skipped because the installed real Sender keeps no probe log.

The app's waveform on the live Feed (`cargo run -p scopecreep --example measure_feed`, Colour Setup read through the helper, playhead moved by script on the `ramp` clip, Resolve stopped):

| Picture column (ramp value) | Data-levels project, app | Resolve, Data Level scale (default) | Video-levels project, app | Resolve, Video Level Scopes on |
|---|---|---|---|---|
| 0 (0.0) | 0 | 2.4 | 64 | 66.2 |
| 480 (0.2501) | 256 | 258.0 | 283 | 285.1 |
| 960 (0.5003) | 512 | 513.7 | 502 | 504.1 |
| 1440 (0.7504) | 768 | 769.3 | 721 | 723.0 |
| 1919 (1.0) | 1023 | 1025.0 | 940 | 941.9 |

Resolve's columns are with Extents off (Resolve's default, as in #8).

Resolve's readings come from a least-squares line fit to its docked Y waveform (0.35 px per code, fit residual 1.1–1.3 codes rms), the same method as #8, which also saw a constant offset of about +2 codes. Resolve's readings were identical in both projects for each scale setting: its scale follows the scope option, not the project. **The app matches Resolve within ±3 codes in each Levels setting** (differences 1.3 to 2.4), when Resolve's scope is set to the same Levels as the project (Video Level Scopes, a person's click in the Waveform Scale Style submenu, for video levels; the default Data Level scale for data levels). With Video Level Scopes on, the graticule numbers stay 0–1023 and the plot is rescaled.

**Extents does not change the reading.** Extents (in the scope settings) had been left ticked for the first readings, so each scale was read both ways, in both projects. With Extents on: Data Level scale 2.3 / 258.0 / 513.8 / 769.5 / 1025.2, Video Level Scopes 65.8 / 284.9 / 504.0 / 723.2 / 942.3. These are within 0.4 codes of the Extents-off readings in the table, well inside the fit's ±3 codes. Extents only changes how the trace is drawn: an outline of the excursions, as the manual says.

**Out-of-range values differ in display, not in reading:** with Video Level Scopes on, Resolve drew the −0.25..1.25 part of the ramp flattened at about code 64 and about 940 (it does not show sub-black or super-white; the projects have "Retain sub-black and super-white data" off). The app shows values between −0.0731 and 0 at codes 0–64, values up to 1.0947 at codes 940–1023, and anything further out in its edge rows. Whether Resolve shows them with "Retain sub-black and super-white data" on was not tried.

Screenshots (not in git): `~/scopecreep-testdata/screenshots/18/` — Resolve: `video-project-datalevel-extentsoff.png`, `full-project-datalevel-extentsoff.png` (Data Level scale, Extents off: the defaults), `video-project-videolevelscopes-extentsoff.png`, `full-project-videolevelscopes-extentsoff.png` (Video Level Scopes on, Extents off); with Extents on: `video-project-scope-default.png`, `full-project-scope-default.png` (Data Level scale), `video-project-videolevelscopes.png`, `full-project-videolevelscopes.png` (Video Level Scopes on); the app: `app-full-project.png`, `app-video-project.png`, `app-full-project-merged.png` (after #19 landed).

## Not verified

- The Sender's manual Levels set by a person on the Sender node in Resolve (scripts cannot set OFX parameters); tested with the fake OFX host (#16) and at the Feed seam.
