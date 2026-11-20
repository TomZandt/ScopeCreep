# #11 Stage 0: How Resolve treats the Sender (caching, Shift-D, placement)

> Copied from the GitHub issue comments on TomZandt/scopecreep#11 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-14)

Stage 0 check 5 (Resolve 21.2.0.6 Studio, probe on the timeline-level node `sender`, project `scopecreep-test-dwg-rec709`). Scripts: `harness/experiments/sender_behaviour/`.

1. **Cache renders while paused: yes, indistinguishable by property.** With the clip-level node cache Enabled and project cache Smart or User, an edit made Resolve cache in the background: the probe got **96 renders (one per frame 0..95) in ~1.2 s (~12 ms/frame vs 41.7 ms in playback) while the playhead stayed on frame 10**. They look like viewer renders: same thread, thumbnail=0, interactive/draft/sequential flags and page property unset. They can only be told apart by frame vs playhead, pacing and sequence pattern. No burst with: project cache none; Smart/User with node cache Auto; node cache Off. Viewer renders also repeat the same frame 1-3 times on first visit.
2. **Playback of cached frames: probe not skipped** (79 renders for 72 frames, nothing missing), but only a clip-level node cache could be tested (expected: the timeline-level probe sits downstream of it). `TimelineItem.SetColorOutputCache(True)` returns False everywhere, so a clip/timeline Color Output cache that might hide the probe is **untested**.
3. **Node cache mode Off by script: not possible on the timeline node** (`GetNodeCacheMode` None, `SetNodeCacheMode` False); works on clip nodes (1/0/-1), where Off/Auto stops the bursts but that isn't the node the Sender sits on. So it needs the UI, or the Sender must filter.
4. **Shift-D: the probe gets nothing** (0 renders bypassed, ~60 normal); toggling back restores a bit-identical frame. The Sender's scope would freeze on the last frame.
5. **Adjustment clip vs last timeline node: only a proxy.** Scripts can't place an adjustment clip above footage (it ripple-inserts into V1; V2/disabled/locked V1 all failed). Proxy: a probe on a clip-level node saw the ungraded source while the timeline probe saw the clip's grade; a plugin sees its node's input, *before that node's own grade/LUT* (a strong LUT on the same node didn't change the probe frame though the viewer changed). By pipeline order an adjustment-clip probe should miss timeline-node grades, **not measured**.
6. **Disable/pages/timelines:** disabled node = 0 renders (re-enable gives 2 on the current frame then normal). Page switches (Edit/Cut/Fusion/Fairlight/Deliver/Media) give 1-8 renders each (Cut gave small thumbnails); page property is unset everywhere (shortcuts: Shift+3 Cut, Shift+5 Fusion, Shift+6 Color). Timeline switch: ~5-6 renders at the new playhead; instances are created per timeline and kept (no CREATE/DESTROY).
7. **Thumbnails:** of 18,648 renders, 157 were thumbnails, **every one flagged `thumbnail=1`** (143x81, 208x117, 288x162, 92x46); no full-size render was ever flagged.

**Recommendation for the real Sender:** ignore `thumbnail=1`; don't try to detect cache renders by property (none exists); gate on playhead plus pacing (viewer frame = the most recent frame, or frames arriving near real time; a run of new frames at several times real time with a stationary playhead = cache fill, keep the last viewer frame); de-duplicate repeated renders; treat zero renders as "no frames" (Shift-D, disabled node, Fusion page), show a stale/frozen state, not an error.
**Placement guidance for users:** put the Sender in its own final timeline node with no grade or LUT on that node (the plugin sees the node's input); leave that node's cache mode Auto or Off (can't be set by script); don't use an adjustment clip if timeline grades are to be monitored; don't use Shift-D.

**Unverified / needs a human:** a clip/timeline-level Color Output render cache hiding the probe; setting the timeline node's cache to Off and its effect; a real adjustment clip and a strong grade on an earlier timeline node; Shift-D or scrubbing on the Edit/Cut pages; a screenshot of the bypassed viewer.



## Shift-D and scrubbing on the Edit and Cut pages (#27, checked 2026-11-20)

With the app running on `scopecreep-test-bench50` (Sender on the last timeline node), counting the Sender's renders from the Feed header around each action, on the Color, Edit and Cut pages alike:

- **Scrubbing** (moving the playhead to a new frame while paused) gives the Sender exactly one render per frame, and the app shows that frame.
- **Shift-D (bypass colour grades) gives the Sender no frames** (0 renders when scrubbing with it on); Resolve's own viewer shows the bypassed picture. The app keeps showing the last frame it had, and its status line (`paused · 0 dropped · <project>`) says nothing about it: from the app alone a colourist can't tell the scopes are stale. Shift-D again brings the Sender back on the next scrub (1 render).
- Screenshots of Resolve's bypassed viewer next to the app: `~/scopecreep-testdata/screenshots/27-shift-d-{color,edit,cut}.png` (not in git).
