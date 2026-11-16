# #16 Reading the Colour Setup by script while Resolve runs

What ticket #16 learned by testing DaVinci Resolve 21.2.0.6 Studio on Linux, with the app, its scripting helper (`app/resolve/colour_setup_helper.py`, ADR 0006) and the real Sender (probe-alias build) on the timeline node. Scripts used are described below; every Resolve step ran under the shared `~/scopecreep-testdata/resolve.lock`.

## Every harness test project reads correctly

One read per project through the app's own helper (`cargo run -p scopecreep --example read_colour_setup`, after loading the project):

| Project | Timeline | Label |
|---|---|---|
| yrgb-rec709g24 | scopecreep-yrgb-rec709g24 | Rec.709 Gamma 2.4 → Rec.709 Gamma 2.4 (from project) |
| dwg-rec709 | scopecreep-dwg-rec709 | DaVinci WG/Intermediate → Rec.709 Gamma 2.4 (from project) |
| dwg-p3d65 | hidden-branch | DaVinci WG/Intermediate → P3-D65 (gamma 2.6) (from project) |
| dwg-pq-tmoff | scopecreep-dwg-pq-tmoff | DaVinci WG/Intermediate → Rec.2100 ST2084 (from project) |
| dwg-pq-tmon | scopecreep-dwg-pq-tmon | DaVinci WG/Intermediate → Rec.2100 ST2084, DaVinci tone mapping (from project) |
| acescct-rec709 | scopecreep-acescct-rec709 | ACEScct → Rec.709 BT.1886 (from project, assumes ACES 2.0) |
| acescct-p3d65 | scopecreep-acescct-p3d65 | ACEScct → P3-D65 (gamma 2.6) (from project, assumes ACES 2.0) |
| acescct-pq | scopecreep-acescct-pq | ACEScct → Rec.2100 ST2084 (1000 nit) (from project, assumes ACES 2.0) |

All Studio, all video levels. In ACES modes Resolve still reports `colorSpaceTimeline`/`colorSpaceOutput` (as "Rec.709 (Scene)"); they mean nothing there and are ignored. An untouched new project reads "Rec.709 (Scene) → Rec.709 (Scene)" (YRGB).

## How long a read takes

- A read (current project, current timeline, its name, `Timeline.GetSetting()`) takes **7–17 ms** with Resolve stopped and settled; starting the helper and connecting adds about 15 ms.
- **Right after a project is loaded, reads are slow:** 2.2 s and 6.2 s were seen, and one read just after a load did not answer within the app's 5 s timeout (the app gave up, killed the helper and read fine 5 s later).
- **A read made during playback blocks until playback stops.** One read sent 1 s into an 8 s playback returned after 7.2 s, when playback was stopped. Playback itself held 24 fps throughout (UHD and 50/60 fps not tried).
- An open, idle scripting connection costs playback nothing (24 fps, no gaps).

## End to end (app + helper + Sender, `scopecreep-test-dwg-rec709`, 1080p24)

- Over three 5 s playbacks from the timeline start: **no read was started during playback** (the read counter did not move from play to stop), and playback held 24 fps.
- After stopping, the label showed "DaVinci WG/Intermediate → Rec.709 Gamma 2.4 (from project)" within ~1–3 s.
- `colorSpaceOutput` changed to P3-D65 by script while stopped: the label changed to "… → P3-D65 (gamma 2.6) (from project)" about 3 s later, with no new frame and nothing restarted; it changed back after the revert in the same way.
- With an ACES project the waveform's label followed the Output Colour Space (P3-D65) and the picture label read "ACEScct → P3-D65 (gamma 2.6) (from project, assumes ACES 2.0)".
- In one run a read was due at the moment play was pressed. Playback then stalled for over a second mid-play, the app took the stall for a stop and read again during playback; that run averaged about 15 fps. It did not recur in three later runs, so the cause is not proven, but the app now waits for 10 s of quiet after any read that the Sender rendered during.

## Pitfalls found

- A helper started from a process that held `flock`'s descriptor inherited it; once stuck in a Resolve call and orphaned, it held the shared Resolve lock for everyone. The helper now inherits no descriptors and dies with its parent. Run commands that start long-lived children with `flock -o`.
- Resolve's process name is `GUI Thread`, so `pgrep -x resolve` finds nothing; match the command line (`pgrep -f '^/opt/resolve/bin/resolve$'`).
- After a restart Resolve showed "Missing Packages Detected" (Continue) and a "Problem Report" window; scripting worked once the first was dismissed.

## Not verified in Resolve

- The manual override on the Sender node: Resolve's scripting API cannot set OFX parameters, so it was tested with the fake OFX host (`make -C sender test`) and at the Feed seam (`app/tests/colour_setup_seam.rs`), not by a person in Resolve's UI.
- Resolve's free version, external scripting off, UHD or 50/60 fps timelines, and the Fusion page / Shift-D (no Sender renders, so playback cannot be told from a stop).
