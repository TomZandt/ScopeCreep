# #7 Stage 0: Place the probe Sender on timeline nodes in test projects (one-time)

> Copied from the GitHub issue comments on TomZandt/scopecreep#7 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-13)

Done (with a script assist). The user hand-placed the probe Sender in `scopecreep-test-dwg-p3d65` (timeline-level node `sender`, plus a separate `hidden-branch` timeline: Color Space Transform DWG/DI -> P3-D65 Gamma 2.4 (tone/gamut mapping None) -> `sender-hidden`, joined by a mixer, with the hidden node's **key output gain at 0** so the output is untouched). The harness `clone-sender` command then exported that project to a .drp and re-imported it as every other test setup, re-applying each setup's Colour Setup.

Verified per clone (7 setups): every Colour Setup key matches the earlier dump; `Timeline.GetNodeGraph()` reports one node labelled `sender` with the Probe Sender; the probe logs renders when the playhead moves; frame-1 float EXR renders match the reference renders exactly (max abs diff 0.0, 4 clips x 7 projects).

Caveats: `hidden-branch` exists only in `scopecreep-test-dwg-p3d65` (DWG -> P3-D65); other setups would need re-cloning with `--keep-hidden` and the CST retuned by hand. In ACES mode Resolve refuses to change the dormant Timeline/Output colour space values, so the clone switches to YRGB, sets both to Rec.709 (Scene), then back to acescct. ACES version (1.3 vs 2.0) remains unreadable/unsettable by script; Resolve defaulted to 2.0 where checked. One unexplained one-off render mismatch (max diff 9.0) on a first acescct-rec709 attempt before that fix; not reproducible.


