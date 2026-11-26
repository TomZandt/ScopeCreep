# Porting to a platform we can't build on the dev machine

How the Windows (#235) and macOS (#236) ports are run. The dev machine is Linux: it can write the platform code but not compile or run it. Like writing a translation without hearing the language spoken, it needs a native speaker to read it aloud before it counts.

## Roles

| Where | Does |
|---|---|
| Linux dev machine | All platform code, per-platform modules, the build scripts, the Installer script, the workflow job. Linux behaviour never changes. |
| Hosted CI runner (no GPU) | Feed tests, clippy and a build of the app, Sender and Installer. Standard runners are free on this public repo. |
| The target machine (GPU, Resolve Studio) | Builds the branch, runs `feed-synth`, the fake-host test, Resolve and `uhd50_bench`. The only place a ticket's acceptance criteria are proved. |

## Order

1. **Prefactor first** (#247): every platform-specific call goes behind one per-platform module, so a port adds a file instead of editing call sites. Merged to the integration branch before any port code.
2. **Write all the port code on Linux, then push once.** One batch of compile errors on the target machine beats one per ticket, and the tickets depend on each other anyway (the Sender needs the Feed, the Installer needs the Sender). Nothing is pushed until the Installer ticket is written. No per-ticket pushes, no draft PR in the meantime: `rust.yml` and `conventions.yml` run on every push.
3. **Build on the target machine** from a checkout of the integration branch. Compile errors go back to the dev machine and are fixed in one pass.
4. **Test one ticket per session, in ticket order**: Feed and app with `feed-synth`, then the Sender and its fake-host test, then Colour Setup and the bench harness in Resolve.
5. **The Installer is the last check, not the feedback loop.** Run a manual `release.yml` Test build, install the setup file on the target machine and check install, the Start menu or Applications entry, and uninstall.
6. **The UHD 50 bench is run from that installed Test build** (Sender off, on, on with the app). Findings go in `docs/findings/`. Only a pass adds the platform to the included platforms.

## Closing tickets

A ticket whose criteria need the target machine stays open until a person has confirmed them there. Until then it gets a progress comment saying what is written and what is untested. Do not close a ticket on CI alone.

## Setting up a target machine (once)

Rust (the platform's default toolchain), the C++ build tools, the GPU toolkit the Sender needs, Python, Git, and Resolve Studio with external scripting set to Local (see `docs/findings/05-external-scripting.md`). Clone the repo and `git config core.hooksPath .githooks`.
