# Coding standards

Read at review, not at implementation. Mechanical rules are checks, not text here: `cargo fmt`, `cargo clippy -D warnings`, `app/tests/style_check.rs` (style tokens) and `scripts/check-commits.sh` run in the pre-commit hook and CI. This file holds the judgement calls.

## Reversal

A change that alters an earlier behaviour (how a Panel treats the footage's shape, zoom, channels, defaults) names the ADR it supersedes or adds a new one, and says so in the changelog. Search `docs/adr/` and `git log -S` for the behaviour first; an undocumented reversal is a finding.

## Deferred work

Anything a ticket leaves out (a refactor, an edge case, a check) is a GitHub issue linked from the PR or commit body. A summary line such as "left out the larger refactors" without an issue number is a finding.

## Panel UI

New Panel UI follows the UI section of `CLAUDE.md`; a deviation needs an ADR.
