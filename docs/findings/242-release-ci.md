# Release CI: building the Sender without a GPU (spec #233, tickets #242 and #243)

The choices behind `.github/workflows/release.yml`. **None of this has run on GitHub yet: the workflow was written and its scripts tested locally (harness tests), but the first real CI run is unverified.** The first Test build ("Run workflow") is the check; until it passes, treat every row as a plan.

| Choice | Why | Status |
|---|---|---|
| Runner `ubuntu-22.04` | Its apt carries `nvidia-cuda-dev` (CUDA 11.5), and the runner image pins it. | Unverified in CI |
| CUDA runtime from apt `nvidia-cuda-dev`, no GPU, no `nvcc`, no driver | The Sender only includes `cuda_runtime.h` and links `libcudart_static.a`; it has no `.cu` files. The Makefile wants `CUDA_HOME/include` and `lib64`, so the workflow symlinks `/usr/include` and `/usr/lib/x86_64-linux-gnu` into a temporary `CUDA_HOME`. | Unverified in CI |
| Official OpenFX 1.5.1 (`OFX_Release_1.5.1`) for the headers | It has `ofxGPURender.h`, which the Sender needs. | Unverified in CI |
| `sender/ci/ofxImageEffectExt.h` stand-in | The official repository lacks Resolve's `ofxImageEffectExt.h`. The stand-in declares only what the Sender uses, `kOfxImageClipPropThumbnail`, with Resolve's value. It is copied beside the official headers in CI only; a local build uses Resolve's own header. | Unverified in CI; if the Sender starts using another Resolve-only name, the build fails and the stand-in needs it |
| Built without a GPU, so not run | CI proves the Sender compiles and links and the Installer packages. It does not prove the Sender works: the UHD 50 check at Resolve (spec #233, `GLOSSARY` Release) still gates adding a platform to `packaging/platforms.txt`. | By design |
| Tag check runs in the `platforms` job on a tag run, and again in `publish` | A tag that is not the workspace version, or has no changelog section, fails before any build. | Tested locally (`check-release-tag.sh`) |
