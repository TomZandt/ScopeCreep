# #13 Stage 0: Report and decisions

> Copied from the GitHub issue comments on TomZandt/scopecreep#13 so the findings live in the repo. The issue is the original; this is the durable record.

## Comment (2026-11-14)

Done on `integration/stage0-milestone1` (commit 6c7fb6b): `docs/stage0-report.md`; ADRs 0001-0004 updated; new ADR 0005 (Colour Setup from the scripting API and manual override, not OFX); GLOSSARY gains **Viewer Frame** and the **Match Resolve** luma mode.

Decisions: exact Colour Setups (published maths, proven in #10) are YRGB Rec.709, DWG/DI to Rec.709/P3-D65/PQ (tone mapping None) and ACES 2.0 to Rec.709/P3-D65/PQ; everything else is labelled approximated until verified. Hidden branch (#9) does not work as built, so it is parked. Colour Setup: scripting API then manual override (OFX gives nothing); ACES version unreadable. Transport: CUDA path, async copy into a pinned /dev/shm ring, never wait (ADR 0003 holds at UHD 23.976-60 fps). Non-viewer renders: ignore thumbnail=1, gate cache renders by playhead and pacing, de-duplicate. Luma: primaries-derived default plus a Match Resolve mode (Resolve uses Rec.709 weights for P3).

Every later ticket was checked against the report and commented where it needs changing: #14 (identity, CUDA, gating), #15 (weights), #16 (priority order, scripting hazard), #17 (exact/approximated list, Resolve behaviours), #18 (no data-levels project, scope toggles untested), #19 (cache bursts, Shift-D, identity), #20 (method and baseline).

Open product questions are listed at the end of the report (default luma weights; how the Rust app reads the Python/Lua scripting API).


