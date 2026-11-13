# Probe Sender (PROTOTYPE, throwaway) - issue #4

Stage 0 check 1: does Resolve's OFX colour management tell a plugin the colour space of each frame?
Only the findings carry forward; this code does not.

    make            # build scopecreep_probe.ofx.bundle
    make install    # copy to /usr/OFX/Plugins (restart Resolve afterwards; it scans plugins at startup)
    make test       # offline: EXR bit-exact round trip + fake-host drive of the built plugin
    make cuda-check # toolchain smoke test (nvcc at /opt/cuda)

Log: `/tmp/scopecreep-probe/probe.log` (override dir with `SCOPECREEP_PROBE_DIR`).
Frame dump: `touch /tmp/scopecreep-probe/dump.trigger`; the next render writes
`frame_inst<N>_t<time>_<W>x<H>.exr` (32-bit float RGBA, uncompressed, bit-exact; rows flipped to top-down).
Colour style declared to the host: `SCOPECREEP_PROBE_STYLE=ocio|full|core|basic|none` (default ocio),
set in the environment Resolve is launched from.
Script: `python3 scripts/apply_probe.py [--apply]` lists the effect (and applies it with `--apply`).
