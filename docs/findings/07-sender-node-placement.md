# #7 Stage 0: Place the probe Sender on timeline nodes in test projects (one-time)

> Copied from the GitHub issue comments on TomZandt/scopecreep#7 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-13)

Done (with a script assist). The user hand-placed the probe Sender in `scopecreep-test-dwg-p3d65` (timeline-level node `sender`, plus a separate `hidden-branch` timeline: Color Space Transform DWG/DI -> P3-D65 Gamma 2.4 (tone/gamut mapping None) -> `sender-hidden`, joined by a mixer, with the hidden node's **key output gain at 0** so the output is untouched). The harness `clone-sender` command then exported that project to a .drp and re-imported it as every other test setup, re-applying each setup's Colour Setup.

Verified per clone (7 setups): every Colour Setup key matches the earlier dump; `Timeline.GetNodeGraph()` reports one node labelled `sender` with the Probe Sender; the probe logs renders when the playhead moves; frame-1 float EXR renders match the reference renders exactly (max abs diff 0.0, 4 clips x 7 projects).

Caveats: `hidden-branch` exists only in `scopecreep-test-dwg-p3d65` (DWG -> P3-D65); other setups would need re-cloning with `--keep-hidden` and the CST retuned by hand. In ACES mode Resolve refuses to change the dormant Timeline/Output colour space values, so the clone switches to YRGB, sets both to Rec.709 (Scene), then back to acescct. ACES version (1.3 vs 2.0) remains unreadable/unsettable by script; Resolve defaulted to 2.0 where checked. One unexplained one-off render mismatch (max diff 9.0) on a first acescct-rec709 attempt before that fix; not reproducible.



## Update from #40 (2026-11-16): checking clones with the real Sender

Since the real Sender replaced the probe, `clone-sender` checks it instead of the probe:

- **Tool name.** `Timeline.GetNodeGraph().GetToolsInNode()` reports the real Sender as `OFX: scopecreep Sender`, and the development probe-alias build (`make -C sender PROBE_ALIAS=1 install`, which keeps the existing test projects working) as `OFX: scopecreep Sender (standing in for the probe)`. The hand-built `scopecreep-test-dwg-p3d65` holds the probe-alias build, so every clone shows the second name.
- **Frame evidence from the Feed.** The Sender counts every render call in the Feed header, and each Sender instance records its renders in the Feed's registry, whether or not an app is attached (`feed/PROTOCOL.md`). Reading `/dev/shm/scopecreep-feed`'s header before and after the scripted playhead moves needs no app and makes the Sender copy nothing. Moving the playhead to 8 frames across the four clips counted 16 Sender renders in every clone.
- **`--verify-only`** runs every check on a clone and then deletes it, leaving the test projects as they were.

Verified with `flock -o ~/scopecreep-testdata/resolve.lock python3 harness/harness.py clone-sender --verify-only` against Resolve 21.2 Studio, for all nine targets:

| Target | Colour Setup vs dump | `sender` node | Sender renders | EXR renders vs reference |
|---|---|---|---|---|
| yrgb-rec709g24, yrgb-rec709g24-full, dwg-pq-tmoff, dwg-pq-tmon, acescct-rec709, acescct-p3d65 | match | yes | 16 | 0.0 |
| acescct-pq | match | yes | 16 | 0.0 on the second run; first run differed (up to 9.3, see [#30](https://github.com/TomZandt/scopecreep/issues/30)) |
| dwg-rec709 | match | yes | 16 | differs (up to 4.8): its references were re-rendered with a stray clip grade ([#43](https://github.com/TomZandt/scopecreep/issues/43)); the clone has no grade |

The one-off ACEScct render mismatch seen in #7 recurred: intermittent, ACEScct clones only, caught by the check (the original project is kept). Its cause is still unknown and tracked in #30.

`harness.py dump --only X` now rewrites `colour-setup-summary.md` from the fresh dump of X plus the saved dumps of every other project, instead of only X.
