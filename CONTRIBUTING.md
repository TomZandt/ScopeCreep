# Contributing

## Commits: Conventional Commits

Every commit message follows [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/):

```
<type>(<scope>): <summary in the imperative, lower case, no full stop>

<optional body: what and why>

<optional footers, e.g. Closes #14, BREAKING CHANGE: ...>
```

**Types**

| Type | Use for | Appears in the changelog |
|---|---|---|
| `feat` | Something a user can see or use | Added |
| `fix` | A bug fix | Fixed |
| `perf` | Faster or lighter, same behaviour | Changed |
| `refactor` | Code change with no behaviour change | no |
| `test` | Tests only | no |
| `docs` | Documentation, findings, ADRs, glossary | only if user-facing |
| `build` | Build system, dependencies, packaging | if it affects users |
| `ci` | CI configuration | no |
| `chore` | Anything else (tooling, housekeeping) | no |

**Scopes** name the part of the repo: `sender`, `feed`, `app`, `waveform`, `colour`, `accuracy`, `harness`, `prototype`, `docs`. Omit the scope when a change spans many.

**Breaking changes** add `!` after the type or scope (`feat(feed)!: ...`) and a `BREAKING CHANGE:` footer saying what breaks and how to move on.

Reference the ticket in a footer (`Refs #15`, or `Closes #15` on the change that finishes it). Merge commits use the same format, e.g. `feat(app): luma waveform (#15)`.

## Versions: Semantic Versioning

The product follows [Semantic Versioning 2.0.0](https://semver.org/). There is one version for the whole product (the app and the Sender ship together). It lives in the root `Cargo.toml` workspace version and in `accuracy/Cargo.toml`, and every release is tagged `vX.Y.Z` on `main`.

While the version is `0.y.z` (before the first public release):

- a breaking change bumps `y` (0.1.4 → 0.2.0);
- `feat`, `fix` and `perf` bump `z` (0.1.4 → 0.1.5);
- `1.0.0` is the first release sold to customers.

From `1.0.0` on: breaking → major, `feat` → minor, `fix`/`perf` → patch.

The **Feed protocol** has its own version (`feed/PROTOCOL.md`), separate from the product version. A breaking protocol change is also a breaking product change.

## Changelog

[`CHANGELOG.md`](CHANGELOG.md) follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/).

- Every user-visible change adds a line under `## [Unreleased]` in the same change, under Added, Changed, Deprecated, Removed, Fixed or Security.
- To release: move the Unreleased lines under a new `## [X.Y.Z] - YYYY-MM-DD` heading, bump the version in the files above, commit as `chore(release): vX.Y.Z`, and tag `vX.Y.Z` on `main`.

## Findings

Anything learned by testing (how Resolve behaves, what a measurement showed) is copied into [`docs/findings/`](docs/findings/) when its ticket closes. Decisions that meet the ADR criteria go in [`docs/adr/`](docs/adr/). See [`docs/findings/README.md`](docs/findings/README.md).

## Keeping the repo clean

- **Work to do lives in GitHub issues, never in repo files.** No to-do lists, "open checks" or TODO sections in docs. Anything a ticket can't verify becomes an issue labelled `ready-for-human` or `ready-for-agent` (or a comment on the planned ticket that will cover it). Code comments may say `TODO(#n)` only with an issue number.
- **The repo holds records:** findings (`docs/findings/`), decisions (`docs/adr/`), vocabulary (`GLOSSARY.md`), the changelog. Records state what is true; they don't track work.
- **Known limitations** a user would care about go in the changelog and README at each release.
- **One commit per ticket on the integration branch:** ticket branches are squash-merged with a Conventional Commit message (`feat(app): luma waveform (#15)`).
- **Merged branches and worktrees are deleted** once their work is on the integration branch or `main`.
- **No generated data in git:** test media, renders, dumps and logs live outside the repo (`~/scopecreep-testdata/`). Build outputs are git-ignored.
- **Enforced:** run `git config core.hooksPath .githooks` once per clone; the `commit-msg` hook and the `conventions` CI workflow both run `scripts/check-commits.sh`.
- **`main` is always releasable:** it only receives finished, reviewed work, with the changelog up to date.


## Definition of done

Finish every item, unprompted. Work is not done while any item is open.

### A ticket

1. **Tests green:** `cargo test --workspace` (plain `cargo test` skips the accuracy crate), `make -C sender test`, and `cd harness && python3 -m unittest discover -s tests`.
2. **Records written:** what testing showed in `docs/findings/<ticket>-*.md` with a row in its README; every decision that is hard to reverse, surprising or a real trade-off as an ADR in `docs/adr/`, listed in `README.md` under Documentation; new terms in `GLOSSARY.md`; a line under `## [Unreleased]` in `CHANGELOG.md` for anything a user can see.
3. **Loose ends filed:** anything unverified is a GitHub issue (`ready-for-human` when it needs a person at Resolve), linked from the findings.
4. **Merged and closed:** squash-merged with one Conventional Commit, pushed, the issue closed with a comment pointing at the commit and findings.
5. **Clean:** the ticket's branches (local and on GitHub) and worktrees deleted; Resolve left on the project that was open, with no scratch projects.

### A release

1. **Performance check passes:** `harness/experiments/uhd50_bench` (ADR 0003) on the build being released.
2. **Versioned:** the Unreleased lines moved under `## [X.Y.Z] - date` (known limitations in their own section), the version bumped, `chore(release): vX.Y.Z` merged to `main`, tagged `vX.Y.Z`, pushed.
3. **Published:** a GitHub release for the tag (pre-release while `0.y.z`) with the changelog section as notes and the Linux build attached (the app, the plain Sender bundle, `INSTALL.txt`, `SHA256SUMS`).
4. **Docs current:** README roadmap state, ADR list, findings index; the spec issue closed or its remaining tickets said.
5. **Issues triaged:** finished or obsolete tickets closed with a reason, labels right, links to deleted branches repointed at `main`.
6. **Clean everywhere:** only `main` locally and on GitHub; no worktrees or stashes; no ignored files left in the repo (`git status --ignored`); temporary runs removed from `~/scopecreep-testdata` (keep data the tests and findings use); in Resolve, the user's own projects at the top level and only the `scopecreep-test-*` projects in the `scopecreep` folder.
7. **Launcher works:** the app reinstalled to `~/.local/bin/scopecreep` (README, "Running the app from the app launcher"), and the Sender the test projects need installed (`make -C sender PROBE_ALIAS=1 install`).
