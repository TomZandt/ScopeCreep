# Brief for the agent on the Windows machine

You are working on the Windows port of ScopeCreep (spec #235, tickets #247 to #252, `gh issue view 235`). The code was written on a Linux machine that cannot compile or run Windows code. Your job is to be the native speaker: build it, run it, find what is wrong, and report or fix it. Read `CLAUDE.md`, `docs/porting-workflow.md` and ADRs 0002, 0003 and 0043 first. This is a personal product: no employer branding, SOPs or skills.

## Machine setup (once)

- Windows 10 or 11, NVIDIA GPU with a current driver (`nvidia-smi` must work), DaVinci Resolve Studio, GPU processing set to **CUDA** (Preferences > Memory and GPU).
- Install: Rust (MSVC toolchain), Visual Studio Build Tools with the C++ workload, the CUDA toolkit, Python 3, Git, the `gh` CLI.
- Resolve: Preferences > System > General > External scripting using = **Local** (`docs/findings/05-external-scripting.md`).
- `git clone` TomZandt/ScopeCreep, `git checkout integration/235-windows-port`, then `git config core.hooksPath .githooks`.
- Resolve's scripting lives under `C:\ProgramData\Blackmagic Design\DaVinci Resolve\Support\Developer\Scripting` and `fusionscript.dll` under `C:\Program Files\Blackmagic Design\DaVinci Resolve`. If a path differs on this machine, say so: that is a finding.

## Rules

- You cannot restart Resolve. Ask the user to when a check needs it.
- Work on a branch off `integration/235-windows-port` (e.g. `windows-fixes`). Never push to `main`. Push fixes to that branch only when the user says to.
- Conventional Commits, one fix per commit, `(#<ticket>)` in the subject. Fix Windows-only bugs yourself, keep Linux behaviour unchanged, and never weaken a test to make it pass.
- Resolve never waits for ScopeCreep (ADR 0003): if a fix risks Resolve's playback rate, stop and report.
- Keep Resolve checks as simple as the question: the UHD 50 loop with the Sender off, on, and on with the app (`harness/experiments/uhd50_bench`). No new benchmark tooling.
- Do not close issues. Comment on them with what you saw. A person closes Windows tickets.
- Treat what you did not run as unverified, and say so.

## Checks, in this order (stop at the first failure and fix or report it)

1. **Build.** `cargo build --release --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Report every compile error with the file and line.
2. **#248 Feed and app.** Run `cargo run -p scopecreep-feed --bin feed-synth`, then the app. Check: the moving test card in every Panel; the Feed is the named mapping `Local\scopecreep-feed`; `SCOPECREEP_FEED` overrides it; settings, pages and hotkeys appear under `%APPDATA%\ScopeCreep` and survive a restart; no console window; sharp and correctly sized at 200% display scaling; closing feed-synth gives the "no Feed" state.
3. **#249 Sender.** From a Developer Command Prompt (x64): `cmake -S sender -B sender\build\cmake` (add `-DOFX_SDK=<folder>` if Resolve's OpenFX headers are not at the default in `sender/CMakeLists.txt`: a different path is a finding), `cmake --build sender\build\cmake --config Release`, then `ctest --test-dir sender\build\cmake -C Release --output-on-failure`: the fake-host test on the GPU, the guest test, and `sender_links_only_system_libraries` (CUDA runtime static, nothing else linked; `dumpbin /dependents` on the `.ofx` shows the same). The bundle is `sender\build\scopecreep_sender.ofx.bundle`. `cmake --install sender\build\cmake --config Release` (admin) copies it to `C:\Program Files\Common Files\OFX\Plugins`; restart Resolve (ask the user), find ScopeCreep under Effects > OpenFX, put it on a node, and check the app shows the picture. Write findings as you go.
4. **#250 Colour Setup and harness.** The Colour Setup line must match the project's real settings. Kill the app from Task Manager: the Resolve helper process must die with it. Run `cd harness && python -m unittest discover -s tests`, then start `uhd50_bench` and confirm it drives the loop.
5. **#251 Installer.** From a manual `release.yml` Test build, run the setup `.exe`. Check: asks to close Resolve if running; app in Program Files, Sender in the Common Files OFX folder; Start menu entry; Add or Remove Programs entry; `INSTALL.txt` explains SmartScreen and NVIDIA/CUDA; uninstall removes everything.
6. **#252 UHD 50.** From the installed build, run `uhd50_bench` with the Sender off, on, and on with the app. Pass criteria are in ADR 0003 and the existing Linux findings (`docs/findings/12-uhd-copy-cost.md`). GPU monitoring uses `nvidia-smi`. Copy the results into a new file in `docs/findings/` and add it to the index.

## What to report back to the user, per ticket

- Pass or fail for each acceptance criterion in the ticket.
- Exact errors, versions (Windows, driver, CUDA, Resolve) and file paths.
- What you changed, as commit hashes.
- What you could not check.
