# #82 Each Panel's own GPU time per UHD frame

What ticket #82 measured on 2026-11-20, headless, with the development example `app/examples/panel_gpu_time.rs` (`cargo run --release -p scopecreep --example panel_gpu_time -- --frames 1000 --warmup 50`). The app at `ddf0bd6` (`main`) plus this example. NVIDIA GeForce RTX 3070 Ti, driver 615.78.08, Vulkan; Resolve 21 open but not playing, and the desktop on the same GPU.

## Method

- Synthetic UHD 3840 × 2160 float frames: **graded** (smooth gradients with fine noise, a skin-like patch and some highlights over 1: spread like footage), the Feed's **test card** (bars over a ramp), and **flat** grey (every pixel the same: the most contention any counting display can meet).
- Colour Setup DaVinci Wide Gamut / Intermediate → Rec.709 Gamma 2.4 (colour managed), so Output Conversion runs every frame and Stops False Colour decodes scene light. Every display on its default settings, plus the luma waveform with its Colorise chip on.
- Each frame goes through the app's real code: the copy from a host staging buffer to the frame texture (as `FrameUploader`), `OutputConversion::convert`, then each display's `on_frame` and `on_timeline_frame` ("measure"), then each display's `draw` into a Window-sized target at three sizes: the default 1280 × 760 Window at 1x (the default layout's cells), a 3840 × 2160 Window at 2x (the same layout on a 4K monitor) and one Panel filling that 4K Window (Ctrl+F). The hidden displays draw into a default-layout cell of their kind (picture Panels ARRI False Colour's, Scopes the Luma waveform's).
- Timing: the adapter has `TIMESTAMP_QUERY`, `TIMESTAMP_QUERY_INSIDE_ENCODERS` and `TIMESTAMP_QUERY_INSIDE_PASSES`, so every part is timed with wgpu timestamp queries (encoder timestamps round a measure, pass timestamps round a draw). Each part runs in its own submission and the CPU waits for it before the next, so no two parts overlap on the GPU and each time is that part's alone. (Without timestamps inside encoders the example falls back to CPU time from submit to done and says so; not needed here.) The Window as the app runs it (all seven measuring in one submission, all seven drawing in one pass) is timed as well.
- 1000 frames per picture after 50 warm-up frames: median and 99th percentile.

**The budget.** The UHD 50 frame is 20 ms (ADR 0003). The app is allowed half of it on the GPU (10 ms), the rest left to Resolve's own render. The shared work (upload and Output Conversion, about 3.3 to 3.5 ms) comes off first, and the rest is split evenly over the Window's 7 Panels: **a share of about 0.92 to 0.96 ms per Panel**, against the median of its measure plus its draw at the default size. The 99th percentile is reported but not judged: the GPU is shared with the desktop and Resolve, and their work can land between a part's two timestamps (in an earlier 300-frame run, the p99 of a different set of 0.25 ms Panels jumped to 1 to 1.5 ms while their medians stayed put).

## Results

The whole Window per UHD frame (upload + conversion + the 7 measuring + the 7 drawing at 1x), median / 99th percentile:

| Picture | Upload | Output Conversion | 7 measuring | 7 drawing at 1x | **Whole Window** | Of the 20 ms frame |
|---|---|---|---|---|---|---|
| graded | 3.032 / 5.054 | 0.508 / 0.544 | 2.576 / 4.940 | 0.289 / 0.408 | **6.357 / 9.706** (max 11.67) | 31.8 % |
| card | 2.778 / 4.974 | 0.507 / 0.535 | 2.408 / 4.607 | 0.287 / 0.403 | **6.133 / 8.924** (max 9.60) | 30.7 % |
| flat | 2.918 / 4.317 | 0.506 / 0.540 | 1.505 / 2.719 | 0.288 / 0.398 | **5.372 / 7.144** (max 8.10) | 26.9 % |

Each Panel on the **graded** picture (share 0.92 ms), ms, median / 99th percentile:

| Panel | In Window | Measure | Draw 1280×760 1x | Draw 3840×2160 2x | Draw filling 4K | Measure + draw 1x | Share |
|---|---|---|---|---|---|---|---|
| Clean picture | yes | 0.248 / 0.292 | 0.040 / 0.045 | 0.240 / 0.272 | 0.263 / 0.302 | 0.289 / 0.337 | within |
| Luma waveform | yes | 0.274 / 0.294 | 0.009 / 0.010 | 0.014 / 0.015 | 0.123 / 0.162 | 0.284 / 0.303 | within |
| ARRI False Colour | yes | 0.251 / 0.292 | 0.006 / 0.007 | 0.018 / 0.023 | 0.224 / 0.253 | 0.257 / 0.298 | within |
| Vectorscope | yes | 0.999 / 1.388 | 0.210 / 0.288 | 0.172 / 0.175 | 1.025 / 1.036 | **1.212** / 1.588 | **over** ([#186](https://github.com/TomZandt/scopecreep/issues/186)) |
| Saturation heatmap | yes | 0.249 / 0.406 | 0.007 / 0.008 | 0.019 / 0.025 | 0.226 / 0.255 | 0.255 / 0.412 | within |
| Skin tone | yes | 0.247 / 0.287 | 0.008 / 0.010 | 0.022 / 0.028 | 0.230 / 0.261 | 0.255 / 0.295 | within |
| Stops False Colour | yes | 0.001 / 0.002 | 0.008 / 0.009 | 0.019 / 0.022 | 0.246 / 0.273 | 0.009 / 0.010 | within |
| Luma waveform, Colorise on | yes (chip) | 1.632 / 3.028 | 0.015 / 0.016 | 0.018 / 0.019 | 0.138 / 0.176 | **1.648** / 3.043 | **over** ([#187](https://github.com/TomZandt/scopecreep/issues/187)) |
| Gamut/legal warning | no | 0.249 / 0.295 | 0.008 / 0.010 | 0.022 / 0.029 | 0.226 / 0.253 | 0.257 / 0.304 | within |
| Highlight neutrals | no | 0.246 / 0.403 | 0.008 / 0.009 | 0.022 / 0.028 | 0.227 / 0.256 | 0.254 / 0.411 | within |
| CIE chromaticity | no | 0.380 / 0.422 | 0.029 / 0.032 | 0.069 / 0.077 | 0.638 / 0.647 | 0.409 / 0.452 | within |
| Black and white | no | 0.249 / 0.290 | 0.012 / 0.014 | 0.046 / 0.051 | 0.254 / 0.292 | 0.260 / 0.304 | within |
| Colour waveform | no | 1.360 / 2.680 | 0.015 / 0.016 | 0.018 / 0.019 | 0.138 / 0.176 | **1.376** / 2.695 | over (hidden) |
| Channel view | no | 0.249 / 0.300 | 0.012 / 0.014 | 0.047 / 0.053 | 0.253 / 0.289 | 0.261 / 0.313 | within |

Where the other pictures differ (measure + draw at 1x, median): the Vectorscope is 1.194 on the card (over) and 0.471 on flat grey (within); the luma waveform with Colorise on is 0.999 and 0.994 (over on both); the CIE plot is 1.154 on the card (over; hidden) and 0.281 on flat; the Colour waveform is 0.726 and 0.722. Every other Panel is within 0.01 ms of its graded figure on every picture. The full output of all three pictures is in `~/scopecreep-testdata/panel-gpu-time/20261120-1000frames.md` (not in git).

## What it says

- **The whole Window fits.** About 6.4 ms a UHD frame at the median on footage-like content, 9.7 ms at the 99th percentile: under a third of the 20 ms frame, which agrees with the v0.3.1 UHD 50 bench passing ([#67](67-milestone-2-performance.md)).
- **The upload is the largest single cost** (about 2.8 to 3.0 ms): 133 MB a frame from host memory over PCIe. It is shared, not a Panel's, and the Window can't do without it.
- **Over its share in the Window:** the Vectorscope (about 1.2 ms on spread content; its measure is cheap on a flat frame, so it is the scattered atomics into its million bins, not contention) — [#186](https://github.com/TomZandt/scopecreep/issues/186); and the luma waveform once its Colorise chip is on (1.0 to 1.6 ms, the ADR 0022 colour pass) — [#187](https://github.com/TomZandt/scopecreep/issues/187), beside the at-the-app check [#150](https://github.com/TomZandt/scopecreep/issues/150).
- **Every picture-stage Panel costs the same 0.245 ms to measure,** whatever the picture: each finds the frame's mattes on its own (ADR 0029), four times over in the Window, about 1 ms a frame — [#188](https://github.com/TomZandt/scopecreep/issues/188).
- **Stops False Colour's cost is in its draw** (ADR 0019): nothing to measure, 0.25 ms when it fills a 4K Window, 0.01 ms in its default cell.
- Hidden for the record: the CIE plot (0.4 ms, 1.15 ms on the test card's few saturated colours) and the Colour Waveform (0.7 to 1.4 ms) would be over their share; neither is in the Window, so no issue.
- Draws are small at the default size (under 0.05 ms, the Vectorscope's 0.21 ms aside). A Panel filling a 4K Window costs 0.12 to 0.26 ms to draw, the CIE plot 0.64 ms and the Vectorscope 1.0 ms.
