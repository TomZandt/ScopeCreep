# #14 The Feed reaches the app (clean picture)

> Copied from the closing comment on TomZandt/scopecreep#14.

Done on `integration/milestone1`. Milestone 1 tracer bullet: the Feed reaches the app.

- **Feed protocol v1.0** (`feed/PROTOCOL.md`): one shared-memory file `/dev/shm/scopecreep-feed`, versioned header + slot ring (FREE → WRITING → READY → READING), overwrite-oldest and busy drops counted, reader-present heartbeat (Sender idles with no app), single-reader lock with reclaim after a killed app, major-version refusal reported on both sides, 3 KB per slot reserved for the Colour Setup (#16). C header for the Sender, Rust crate for the app, tested against each other.
- **scopecreep Sender** (`sender/`): OFX, Resolve's CUDA render path, staging copy then async pinned copy into a Feed slot published from a host callback; never waits (no stream syncs, one try-lock); staging allocated only when an app first attaches; thumbnails never fed; no OpenEXR/Imath (enforced by `make test`).
- **App** (`app/`, Rust + wgpu): one window, Picture View of the latest Feed frame as sent (float32 texture, no conversion), quiet label, one-line notices (no Sender, waiting, version mismatch, busy, Sender error). `feed-synth` writes synthetic frames.

Commands: `cargo test` (44 Feed-seam tests incl. C interop), `cargo run -p scopecreep`, `cargo run -p scopecreep-feed --bin feed-synth`, `make -C sender && make -C sender test` (fake CUDA host), `make -C sender install`.

Verified in Resolve 21.2 Studio (`scopecreep-test-dwg-rec709`, 1080p24, Sender on the timeline node via a dev alias of the probe's OFX id):
- [x] Picture live in playback (24 Viewer Frames/s, 0 drops, ends on the stop frame), pause (last picture held), frame steps, reverse, scripted scrubs. **Not tested:** a real fast mouse scrub (Hyprland can't deliver arrows/drag to Resolve).
- [x] No app: no ring, no pinned or staging GPU memory; renders are idle passthroughs (0 copies, 0.035 ms mean). Not measured: GPU cost of the passthrough itself.
- [x] `kill -9` on the app mid-playback: Resolve 23.70 fps either way; Sender copies ~1 s then idles; a restarted app re-attaches; the app survives a Resolve restart. Not run: UHD, 50/60 fps, long runs; Resolve's own drop counter (not scriptable).
- [x] Version mismatch reported in the window (screenshot on a forged v2.3 Feed), covered by Rust, C and app tests.
- [x] Synthetic Feed writer drives the app (20 app tests at the Feed seam; screenshots).

Design decisions for later tickets:
- **Viewer Frames are decided in the app** (`app/src/viewer_gate.rs`) from pacing: repeats update but don't count; a run of 4+ frames each under half a frame period apart is a cache fill and ignored; steady playback shown at once; a lone frame held half a period. Limits: at 50/60 fps a cache fill is as fast as playback (needs the playhead from scripting, #16); a real cache fill was only tested synthetically; one playback-start frame was once misclassified.
- **Sender identity** is pid, host name/version, Sender version, instance number and timeline rate. Project/timeline names must come from scripting (#16).
- Header already carries the counters #19 needs; the waveform (#15) can read the same GPU float32 upload.

To see the real Sender in the existing test projects without clicks: move `/usr/OFX/Plugins/scopecreep_probe.ofx.bundle` aside, `make -C sender PROBE_ALIAS=1 install`, restart Resolve. Otherwise place **scopecreep Sender** on the last timeline node by hand.

