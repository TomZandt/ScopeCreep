# #15 Luma waveform: keeping up with UHD float

> What #15 measured while building the luma waveform. Machine: RTX 3070 Ti (8 GB), Linux, Hyprland. Resolve 21.2 was open on the same GPU (about 5–6.5 GB of graphics memory in use by other work) but was not used: the Feed came from `feed-synth`.

## Method

`feed-synth --size 3840x2160 --fps N --path <private feed>` played the moving test card into a Feed; the app (release build) ran with `SCOPECREEP_STATUS_LOG=1 scopecreep --feed <private feed>`, which prints once a second the frames the Feed published (`fed`), the Viewer Frames the app showed, the frames put on the GPU and measured by the waveform (`scope_frames`), frames the viewer gate ignored as cache renders, and the Feed's drop count. Each run lasted 20 s; the numbers are the last status line.

## Results (UHD 3840×2160, RGBA 32-bit float, 133 MB per frame)

| fps | fed | Viewer Frames | measured by the waveform | Feed drops | ignored as cache renders |
|---|---|---|---|---|---|
| 24 | 455 | 453 | 453 | 0 (2 in an earlier run, both at attach) | 0 |
| 25 | 473 | 463 | 463 | 0 | 10 |
| 50 | 949 | 930 | 930 | 0 | 19 |
| 60 | 1138 | 1137 | 1137 | 0 | 1 |

- Every Viewer Frame was put on the GPU and measured over every pixel; the app never fell behind the Feed at any of the four rates.
- Headroom: asked for 120 fps, `feed-synth` itself managed about 88 fps; the app measured all but 4 of 770 frames.
- The "ignored as cache renders" frames are the viewer gate (#14) reading bunched `render_ns` stamps from `feed-synth` when the synthetic writer itself ran late (two frames written less than half a period apart look like a cache fill). They are not frames the app failed to process.

## What made it fit

- **Before #15** the Picture View uploaded each frame with `queue.write_texture`, which took **about 20 ms per UHD frame** on the Feed thread (a new staging allocation each time). At 60 fps that showed about 49 frames a second, with about 11 Feed drops a second.
- **Now** each frame is copied once into one of two staging buffers that stay mapped between frames, then copied to the frame texture on the GPU; the Picture View and the waveform read that one texture. At most two frames are in flight; beyond that the Feed thread waits for the GPU and the Feed drops (and counts) frames instead. The Sender never waits.
- `feed-synth` redrew the whole UHD test card on the CPU every frame and could only manage about 35 fps; it now redraws only the band the marker moves along.

## Waveform conventions (what the tests assert)

- Luma = R·wr + G·wg + B·wb on the values as they arrive (encoded, no linearisation), in 32-bit float. Code = luma × 1023, rounded to the nearest code. Below code −0.5 (and NaN) counts in the "below 0" row; at or above 1023.5 in the "above 1023" row.
- One waveform column per picture column; 1026 counts per column.
- Weights at 4 decimals, derived from primaries and white point: Rec.709 0.2126 / 0.7152 / 0.0722; P3-D65 0.2290 / 0.6917 / 0.0793; Rec.2020 (and Rec.2100) 0.2627 / 0.6780 / 0.0593. Match Resolve uses Rec.709 weights for Rec.709, P3-D65 and P3-DCI, and Rec.2020 weights for Rec.2020 and Rec.2100.
- A single pixel out of range lights its edge row at no less than 80 % of full trace brightness; anywhere in the plot, any non-zero count shows at no less than 25 %.

## Limits

- Measured with `feed-synth`, not Resolve: the GPU was shared with an idle Resolve, but Resolve was not playing, so the combined load of Resolve's own rendering and the app at UHD 60 was not measured here.
- The Measured Space is not known in the app yet: the waveform assumes Rec.709 and says so in its label.
