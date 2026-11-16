# scopecreep

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

Anything learned by testing is copied into `docs/findings/` when its ticket closes (see `docs/findings/README.md`). Anything not yet verified becomes a GitHub issue, never a list in the repo. Read the relevant findings and `docs/stage0-report.md` before working on the Sender, the Feed or colour.

## Keeping the repo clean

Follow "Keeping the repo clean" in `CONTRIBUTING.md`: work to do lives only in GitHub issues; ticket branches are squash-merged with one Conventional Commit; merged branches and worktrees are deleted; no generated data in git; `main` is always releasable. `scripts/check-commits.sh` checks commit messages (also run by the `commit-msg` hook and CI).

