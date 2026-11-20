# scopecreep

A personal commercial product for professional colourists and editors, not employer work: do not apply any employer branding, SOPs or skills here.

## Commands

```sh
cargo test --workspace                          # all Rust tests; plain `cargo test` skips the accuracy crate; seam tests need the GPU
make -C sender test                             # the Sender against a fake CUDA host (GPU, no Resolve)
cd harness && python3 -m unittest discover -s tests
cargo fmt && cargo clippy --workspace --all-targets -- -D warnings   # CI runs both; the pre-commit hook runs fmt --check
scripts/check-done.sh                           # the mechanical Definition-of-done items
cargo run -p scopecreep-feed --bin feed-synth   # a moving test card into the Feed, no Resolve needed
git config core.hooksPath .githooks             # once per clone (commit-msg and pre-commit hooks)
```

## Layout

`feed/` (shared-memory Feed protocol, spec in `feed/PROTOCOL.md`), `sender/` (C++ OFX plugin), `app/` (Rust + wgpu; tests drive it at the Feed seam in `app/tests/*_seam.rs`), `accuracy/`, `harness/` (Python Resolve experiments, including `uhd50_bench`), `prototype/` (Stage 0 probe).

## UI

A tool, not a sales pitch: sensible defaults read from the project, with a manual override tucked away; legends are horizontal strips; show only text a colourist acts on; toggles are toolbar chips and amounts are sliders. Check new UI against `docs/research/panel-controls.md` (ADR 0027).

## Gotchas

- UI chrome draws only through style tokens in `app/src/style.rs`; `app/tests/style_check.rs` fails on a raw colour or size literal (ADR 0026). Measurement colours are exempt.
- The `scopecreep-test-*` Resolve projects carry the old probe, not the Sender: the user puts ScopeCreep on each one's `sender` node once (README, "Test projects"). Claude can't restart Resolve, so ask the user to.
- Keep Resolve checks as simple as the question: the UHD 50 loop with the Sender off, on, and on with the app (`harness/experiments/uhd50_bench`), driven by a short script. No long agent runs or new benchmark tooling.
- Agent worktrees live in `.claude/worktrees/` (git-ignored, often locked); after merging, run `git worktree remove --force --force <path>` and delete the branch.

## Agent skills

### Issue tracker

Issues live in GitHub Issues on TomZandt/scopecreep (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`. See `docs/agents/domain.md`.

## Commits, versions and changelog

Conventional Commits, Semantic Versioning (one product version, starting 0.0.0) and Keep a Changelog. Every commit, including merge commits, uses a Conventional Commit message. Every user-visible change adds a line under `## [Unreleased]` in `CHANGELOG.md` in the same change. See `CONTRIBUTING.md`.

## Findings

Anything learned by testing is copied into `docs/findings/` when its ticket closes (see `docs/findings/README.md`). Anything not yet verified gets a home from "Loose ends" in `CONTRIBUTING.md` (run `scripts/file-loose-end.sh` before filing), never a list in the repo. Read the relevant findings and `docs/stage0-report.md` before working on the Sender, the Feed or colour.

## Definition of done

Before reporting a ticket, a spec run or a release finished, run `scripts/check-done.sh` (ADR list, findings index, changelog, launcher copy: it prints what is open and fixes nothing), then complete every item of "Definition of done" in `CONTRIBUTING.md`, without being asked. After an implement-spec run, the spec-level records are yours too: the spec issues (progress comment, close when done) and the glossary. To fix a stale launcher: `cargo build --release -p scopecreep && install -Dm755 target/release/scopecreep ~/.local/bin/scopecreep`.

## Keeping the repo clean

Follow "Keeping the repo clean" in `CONTRIBUTING.md`: work to do lives only in GitHub issues; ticket branches are squash-merged with one Conventional Commit; merged branches and worktrees are deleted; no generated data in git; `main` is always releasable. `scripts/check-commits.sh` checks commit messages (also run by the `commit-msg` hook and CI).

## Review

The reviewer reads `CODING_STANDARDS.md` (judgement calls only; mechanical rules are checks).
