# Release CI: building the Sender without a GPU (spec #233, tickets #242 and #243)

The choices behind `.github/workflows/release.yml`. **The first Test build ("Run workflow", 2026-11-25, run 45693622532 on `main`) passed: every step below ran on a GitHub runner. The tag-publish path (`publish` job) has not run yet.**

| Choice | Why | Status |
|---|---|---|
| Runner `ubuntu-22.04` | Its apt carries `nvidia-cuda-dev` (CUDA 11.5), and the runner image pins it. | Verified in CI |
| CUDA runtime from apt `nvidia-cuda-dev`, no GPU, no `nvcc`, no driver | The Sender only includes `cuda_runtime.h` and links `libcudart_static.a`; it has no `.cu` files. The Makefile wants `CUDA_HOME/include` and `lib64`, so the workflow symlinks `/usr/include` and `/usr/lib/x86_64-linux-gnu` into a temporary `CUDA_HOME`. | Verified in CI |
| Official OpenFX 1.5.1 (`OFX_Release_1.5.1`) for the headers | It has `ofxGPURender.h`, which the Sender needs. | Verified in CI |
| `sender/ci/ofxImageEffectExt.h` stand-in | The official repository lacks Resolve's `ofxImageEffectExt.h`. The stand-in declares only what the Sender uses, `kOfxImageClipPropThumbnail`, with Resolve's value. It is copied beside the official headers in CI only; a local build uses Resolve's own header. | Verified in CI (Test build passed); if the Sender starts using another Resolve-only name, the build fails and the stand-in needs it |
| Built without a GPU, so not run | CI proves the Sender compiles and links and the Installer packages. It does not prove the Sender works: the UHD 50 check at Resolve (spec #233, `GLOSSARY` Release) still gates adding a platform to `packaging/platforms.txt`. | By design |
| Tag check runs in the `platforms` job on a tag run, and again in `publish` | A tag that is not the workspace version, or has no changelog section, fails before any build. | Tested locally (`check-release-tag.sh`) |

## Installed from the Test build, checked at Resolve (2026-11-25)

The Installer from that Test build was downloaded with `gh run download`, checked against `SHA256SUMS` and installed with `install.sh` (app to `~/.local/bin`, Sender to `/usr/OFX/Plugins`). Resolve 21 Studio on Linux, CUDA, project `scopecreep-test-bench50`, ScopeCreep on the Sender timeline's `sender` node, then `harness/experiments/uhd50_bench` (ADR 0003), each configuration run on its own:

| Configuration | Resolve's fps readout (12 grabs over 60 s) | Sender renders/s |
|---|---|---|
| Sender off | 50 in all 12 | 0.00 |
| Sender on, no app | 50 in all readable grabs (one black grab) | 50.00 |
| Sender on, with the app | 50 in all 12 | 49.99; the app logged 3764 frames, 3703 shown |

The Sender held 50 fps and the app received the Feed. The bench starts `target/release/scopecreep` (a local build), so the Sender was the CI one and the app was the repo's build, not the installed copy. Not covered: the `publish` job and a real tagged Release (#243, #244).

Bench pitfalls found on the way: Resolve scripting blocks while playback runs, so the bench hangs at connect if Resolve is still playing (stop it first, ADR 0005); the fps readout position is set with `SCOPECREEP_BENCH_GRAB` and must match the Resolve window; and a Sender timeline without ScopeCreep on its node reads 0 renders/s in every configuration.
