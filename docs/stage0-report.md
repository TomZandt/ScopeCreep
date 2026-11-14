# Stage 0 report: colour and performance checks

Run against DaVinci Resolve 21.2.0.6 Studio on Linux (RTX 3070 Ti, driver 615.78, CUDA 13.4). Spec: #2. Evidence lives in the closing comments of #4, #6, #8, #10, #11 and #12 and the findings comment on #9. Experiment code is under `prototype/`, `harness/` and `accuracy/`. Everything in `prototype/` is throwaway; only these findings carry forward.

## Verdict

Milestone 1 is buildable as designed, with four changes to the plan:

1. **The Colour Setup cannot come from OFX.** Resolve gives a plugin no per-frame colour space. It comes from the scripting API and a manual override.
2. **The hidden branch does not work as built**, so the Output Conversion uses published maths.
3. **Resolve's own waveform uses Rec.709 luma weights for P3 outputs.** ADR 0004 disagreed. A "Match Resolve" mode is added.
4. **The Sender must use Resolve's CUDA render path.** On it, copying UHD float frames out of Resolve costs Resolve no playback rate (ADR 0003 holds).

## 1. Which Colour Setups are measured exactly (check 4, ticket #10)

Method for all of them: **published maths** (`accuracy/` crate, cited sources in each module). "Exact" means within 1e-4 on the encoded 0..1 scale (0.1 of a 10-bit code), or 5e-6 in display-linear light (to allow float32 noise at black under ACES). Compared on four clips (ramp, chart, out-of-range, natural) against Resolve's own float EXR renders.

| Colour Setup | Max error (10-bit codes) | Verdict |
|---|---|---|
| YRGB, Rec.709 Gamma 2.4 → Rec.709 Gamma 2.4 | 0 | exact |
| DWG/DI → Rec.709 Gamma 2.4 | 0.013 | exact |
| DWG/DI → P3-D65 (gamma 2.6) | 0.0005 | exact |
| DWG/DI → Rec.2100 PQ, tone mapping None | 0.0096 | exact |
| ACEScct, ACES 2.0 → Rec.709 BT.1886 | 3.9 (dark band only) | exact (float32 noise at black) |
| ACEScct, ACES 2.0 → P3-D65 | 4.8 (dark band only) | exact (float32 noise at black) |
| ACEScct, ACES 2.0 → Rec.2100 PQ 1000 nit | 1.45 (dark band only) | exact (float32 noise at black) |

**Labelled "approximated" (reason shown in the app) until verified:**

- **DaVinci tone mapping on.** Not testable by script: the setting reads back "DaVinci" but Resolve's render equals tone mapping off for every value tried. Needs UI investigation.
- **Gamut mapping on.** The scripting API accepts only "None".
- **ACES 1.3.** The version cannot be read or set by script. Our ACES 2.0 port matches Resolve to float noise, so the test projects are ACES 2.0. 1.3 would need a UI-built project and an RRT/ODT port.
- **Never tested:** Rec.2100 HLG, Rec.2020 Gamma 2.4, P3-DCI, other ACES output transforms. Treated as approximated until the accuracy suite covers them.

The label never claims more than the table above proves.

**Behaviour found by comparison** (recorded in `accuracy/src/pipeline.rs`):
- "P3-D65" output is **gamma 2.6**, not 2.4. App labels must say so.
- "Rec.709 Gamma 2.4" is a pure power law, not the BT.709 OETF.
- Float output is not clamped; out-of-range and negative values pass through. Gamma outputs are sign-preserving power laws. PQ negatives encode as 0.
- DWG scene-linear 1.0 = 100 nit. With mapping None there is no extra tone or gamut handling.
- ACES ODT names in Resolve equal the reference CTL `ACESuserName`s.

## 2. Hidden branch (check 3, ticket #9) — does not work as built

The user-built branch (Color Space Transform, then a second probe node, mixed in with key output gain 0):

- The hidden probe is rendered for every played and scrubbed frame, even at 0% mix (192 scrub steps, 4 plays, no frame missing).
- Its frames are **all zeros**, so it cannot do the Output Conversion.
- The branch **changes the delivered output** (max abs diff up to 8434 on the out-of-range clip). Disabling only the CST restores the output exactly; disabling the mixer or the hidden node does not. Key output gain 0 does not isolate the CST.
- No playback cost was measurable at 1080p24.

**Decision:** use published maths. The hidden branch stays open as a parked, optional improvement: its only remaining value is covering DaVinci tone mapping, gamut mapping and ACES 1.3. Retrying needs a human to try other node-tree variants (see #9).

## 3. How the Colour Setup is obtained (check 1 and 2, tickets #4 and #6)

- **OFX colour management gives nothing.** The host advertises style `Full` (config `ofx-native-v1.5_aces-v1.3_ocio-v2.3`), but the source clip reads `Raw`, the output clip `OfxColourspace_Source`, with no OCIO config or display values. Tested with the plugin declaring OCIO and `Full`, in an ACEScct (ACES 2.0) and a DWG project, with "Enable OpenFX Color Management" ticked. Answer: **no**.
- **The scripting API gives everything except the ACES version.** `IsStudio()`, product name and every Colour Setup key are readable (keys and values in #6's closing comment). The host name does not distinguish Studio from free.
- **The Resolve page is not reported** to the plugin on any page. OFX also gave no project or timeline name in the logs.

**Priority order, changed:** (1) scripting API (Studio, external scripting = Local); (2) the Sender's manual override. OFX colour management is dropped. The ACES version is unknown to the API, so ACES setups say "assumes ACES 2.0" unless set manually.

**Hazard:** Resolve scripting calls made during playback block for minutes and starve Resolve's main thread. The Colour Setup must be read when playback is stopped, never from the Sender's render call, and ideally from outside Resolve.

## 4. Transport and copy strategy (check 6, ticket #12)

The strategy that held at UHD 32-bit float RGBA, 23.976 to 60 fps, copy on vs off: **identical playback rate, zero dropped** in every run.

- The Sender opts into Resolve's **CUDA render path**. On the CPU path Resolve copies each frame to host memory synchronously (~13 ms) and UHD playback slows to ~30 fps on 50 and 60 fps timelines.
- Per viewer render: copy the frame into a private device staging buffer on Resolve's stream, then `cudaMemcpyAsync` into a **pinned slot in a shared-memory ring** (`/dev/shm`) on the Sender's own stream. A host callback publishes the slot. The render call only enqueues work and returns (~0.07 ms).
- A full ring overwrites the oldest unread slot and counts a drop. The Sender never blocks.
- Cost at 60 fps UHD: ~9 GB/s of ~23 GB/s PCIe, +~28 points GPU utilisation, copy GPU time ~5.7 ms, submit-to-done latency ~10 ms mean (max 45 ms).
- Frames are bit-exact: 2,444 frames passed a checksum; 4 matched Resolve's own dumps; CPU and CUDA paths give identical bytes.
- Saturated (uncapped render): ~14-18% throughput cost. Zero in real-time playback.
- **VRAM is the tight resource:** Resolve held 7.6-7.8 of 8 GB. Keep staging small.
- **The Sender must skip the copy when no app is attached** (the probe did not).
- **Never link OpenEXR or Imath into the Sender.** Resolve bundles its own copies; the clash crashed Resolve. Resolve also caches plugin descriptions keyed on the binary's mtime.

Fallbacks (half-float, GPU-side reduction, GPU-to-GPU sharing) were not needed and their accuracy cost was not measured.

## 5. Ignoring non-viewer renders, and placement (check 5, ticket #11)

- **Thumbnails:** all 157 were flagged `thumbnail=1` (143x81, 208x117, 288x162, 92x46); no full-size render was. Ignore `thumbnail=1`.
- **Background cache renders** look identical to viewer renders (same thread, no thumbnail flag, flags and page unset). A cache fill produced 96 renders in ~1.2 s while the playhead stayed on frame 10. They can only be told apart by frame vs playhead and pacing: a run of new frames at several times real time with a stationary playhead is a cache fill; keep the last viewer frame. Also de-duplicate: a first visit to a frame renders it 1-3 times.
- **Shift-D (bypass all grades): the Sender gets nothing.** A disabled node or the Fusion page also give no frames. Zero renders is "no frames" (show a frozen state), not an error.
- **A plugin sees its node's input, before that node's own grade or LUT.**

**Placement guidance for users:** put the Sender in its own last timeline node, with no grade or LUT on that node; leave that node's cache mode Auto or Off; do not use an adjustment clip if timeline-level grades are to be monitored; do not use Shift-D.

**Not verified (needs a human):** a clip or timeline Color Output cache hiding the Sender; the timeline node's cache mode (scripts cannot read or set it); a real adjustment clip; Shift-D on the Edit and Cut pages.

## 6. Luma weights (check 7, ticket #8)

Resolve's own waveform (Y mode) computes luma on the **encoded** signal, with fixed standard weights chosen by the Output Colour Space's primaries:

| Output Colour Space | Weights Resolve uses | ADR 0004 (primaries-derived) |
|---|---|---|
| Rec.709 | Rec.709 | agrees |
| **P3-D65, P3-DCI** | **Rec.709** | **disagrees** (0.2290 / 0.6917 / 0.0793) |
| Rec.2020, Rec.2100 PQ, Rec.2100 HLG | Rec.2020 | agrees |

Uncertainty ±3 codes; the P3 difference is 24 codes on green. No scope setting changes the luma calculation. For PQ and HLG this is Rec.2020 weights on encoded values, not true luminance.

**Decision:** keep primaries-derived weights as the default (ADR 0004) and add a **Match Resolve** mode (ADR 0004 updated). In Resolve's Data Level mode the 10-bit scale runs 0 to 1023; the Video Level and Extents settings were not tested (ticket #18).

## Open questions for the product owner

- Should the default luma be primaries-derived (mathematically consistent, differs from Resolve on P3) or Match Resolve (what a colourist cross-checking against Resolve expects)? ADR 0004 keeps the existing decision.
- Reading the Colour Setup: the app is Rust and Resolve scripting is Python/Lua. Likely a small helper process; to be decided in #16.
