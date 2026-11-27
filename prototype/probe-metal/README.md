# Metal probe (PROTOTYPE, throwaway) - issue #253

Answers, on an Apple Silicon Mac, before the macOS Sender is designed (#236): how does Resolve hand a plugin each
frame under Metal (buffer type, storage mode, command queue), what does it cost to copy a UHD float frame (about 132.7 MB)
into POSIX shared memory, and is it better to blit into our own buffer first or straight into the shared memory?
Only the findings carry forward; this code does not. **It was written on Linux and has never been compiled.** Expect
to fix a small compile error or two; send them back and they get fixed.

The probe passes the picture through unchanged (a GPU blit on Resolve's own queue, not waited for), logs every render, and for
the first 30 full-size renders times the copy into shared memory three ways. Those timing copies wait for the GPU, so
Resolve will play slower while they run; that is expected (the real Sender never waits).

## 1. Build (Xcode command-line tools installed: `xcode-select --install`)

    cd prototype/probe-metal
    make test     # optional: the pure helpers, no Metal needed
    make          # build/scopecreep_metal_probe.ofx.bundle, arm64, ad-hoc signed

If it cannot find `ofxCore.h`, find Resolve's headers and pass them:
`find /Library/Application\ Support/Blackmagic\ Design -name ofxCore.h`, then `make OFX_SDK="<that folder>"`.
Check the result is arm64: `file build/scopecreep_metal_probe.ofx.bundle/Contents/MacOS/scopecreep_metal_probe.ofx`.

## 2. Install

    make install      # copies the bundle into /Library/OFX/Plugins (asks for your password)

Quit Resolve completely (Cmd-Q) and start it again; it only scans OFX plugins at start-up. Remove later with `make uninstall`.

## 3. Run in Resolve

Open a UHD (3840x2160) timeline, ideally 50 fps (the Sender's real target; use the `scopecreep-test-*` UHD 50 project if it
is there). In the Color page add the OpenFX effect "scopecreep Metal Probe" (OpenFX panel, group "scopecreep") on a node.
Play the clip for 10 to 20 seconds, and scrub a little. Do it once more with Resolve's GPU processing mode set to Metal
(Preferences > Memory and GPU) if it is on Auto, and note which you used.

## 4. Where the log lands

`/tmp/scopecreep-probe/probe_metal.log` (set `SCOPECREEP_PROBE_DIR` in the environment Resolve is started from to move it).
`/tmp` is `/private/tmp` on macOS. Useful lines:

    grep -E "LOAD|DESCRIBE" /tmp/scopecreep-probe/probe_metal.log     # host + GPU
    grep -E "RENDER" ... | head -20                                   # how the frame arrives (look at "metalEnabled" and "data=")
    grep -E "COPY #|SUMMARY" ...                                      # the timings

Key lines to read: `metalEnabled=1` (did the host use Metal at all?), `data=MTLBuffer ... storage=... cpuVisible=...` (buffer type
and storage mode), `queue:` (the command queue), `COPY ... A/B/C` (the three copy routes; the SUMMARY lines have n, mean, min,
max, median in ms). Route B printing `returned nil` means a buffer cannot wrap the shared memory, which settles that route.
Per-render lines can be silenced with `SCOPECREEP_PROBE_QUIET=1`; `SCOPECREEP_PROBE_COPIES=N` changes the 30; `touch
/tmp/scopecreep-probe/copy.trigger` runs another batch while Resolve plays. The shared memory is `/scopecreep-probe-metal`.

Send back the log (or the SUMMARY lines plus a RENDER block), the Mac model, macOS and Resolve versions, and the playback
settings, for the findings and the recommendation (blit to own buffer, or straight into a Feed slot).
