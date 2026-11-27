# #254, #255, #256: the app, the Metal Sender and the helper on the Mac

What the first run on the Mac showed on 2026-11-27: Mac mini (Mac16,10), Apple M4, macOS 26.6.2, DaVinci Resolve Studio 21.1.1, Python 3.9.6. Everything was built on the Mac from `integration/236-macos-port`.

## Worked

- The app builds and passes `cargo clippy` and `cargo test` on the Mac (the Metal GPU adapter was present, no seam test skipped). The picture from Resolve reaches the app and the Colour Setup line matched the project; the Resolve scripting library is at `/Applications/DaVinci Resolve/DaVinci Resolve.app/Contents/Libraries/Fusion/fusionscript.so`.
- The Sender builds as an arm64 bundle linked only to Metal, Foundation, CoreFoundation, libc++, libSystem and libobjc, and ad-hoc signs. Its fake-host test passes: frames arrive bit-exact, `render` returns at once while the GPU is busy (60 renders, worst call 0.194 ms), and the frame is not yet in the Feed when it returns.
- The Colour Setup helper never outlives the app: after `kill -9` of the app the helper and its watcher were gone in 0.02 s; after Cmd+Q, too. The helper runs as `python3 -u -c <script>`, so `pgrep -fl colour_setup_helper` finds nothing; look for it as a child of the app (`ps -axo pid,ppid,command`).
- The harness reads the Feed (`renders` counted up with the app attached, 0 in 5 s with Resolve idle) and `uhd50_bench` starts and drives the loop. At 1080p 50 (Timeline 1, 250 frames, 62 s each): Sender off 0 renders/s, on 50.00, on with the app 50.00. Resolve's indicator read 50. The bench needs Accessibility and Screen Recording for the terminal.

## Did not work, or not settled

- **UHD 50 on this Mac, Sender enabled: Resolve played 18 fps against 50 fps with the Sender disabled** (seen by eye in Resolve, not logged; the bench could not run UHD because the test project and EXR frames were not on the Mac). This misses the bar in ADR 0003. See #258.
- A shared-memory name over 31 bytes fails on macOS: the GPU test rig built `/sctest-<pid>-<test name>`. It now hashes the name.
- clang, unlike gcc, rejects every unused `static inline` function in `scopecreep_feed.h` when it is the main file of a C++ check. The test passes `-Wno-unused-function`.
- The Linux packaging and install script tests use GNU `tar --sort=name`, which macOS bsdtar rejects; they now skip off Linux.
- Metal's `pow` is off on one dark PQ value in the Output Conversion (worst 5.1e-4), see #263.
