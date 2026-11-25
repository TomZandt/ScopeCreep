# Contributing

Contributions are welcome. Sending one (a pull request, patch or design) means you agree to section 5 of the [ScopeCreep Licence](LICENSE.md): you have the right to send it, and it may be built into ScopeCreep and licensed with it, including under commercial licences. [LICENSING.md](LICENSING.md) explains the licence in plain English.

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

- **Work to do lives in GitHub issues, never in repo files.** No to-do lists, "open checks" or TODO sections in docs. Leftover work follows "Loose ends" below. Code comments may say `TODO(#n)` only with an issue number.
- **The repo holds records:** findings (`docs/findings/`), research from public sources (`docs/research/`), decisions (`docs/adr/`), vocabulary (`GLOSSARY.md`), the changelog. Records state what is true; they don't track work.
- **Known limitations** a user would care about go in the changelog and README at each release.
- **One commit per ticket on the integration branch:** ticket branches are squash-merged with a Conventional Commit message (`feat(app): luma waveform (#15)`).
- **Merged branches and worktrees are deleted** once their work is on the integration branch or `main`.
- **No generated data in git:** test media, renders, dumps and logs live outside the repo (`~/scopecreep-testdata/`). Build outputs are git-ignored.
- **Enforced:** run `git config core.hooksPath .githooks` once per clone; the `commit-msg` hook and the `conventions` CI workflow both run `scripts/check-commits.sh`.
- **`main` is always releasable:** it only receives finished, reviewed work, with the changelog up to date.


## Loose ends

Anything a ticket leaves unverified or undone goes to exactly one home. Search first: `scripts/file-loose-end.sh` lists open issues and queue boxes that match, and refuses to create a duplicate.

| Leftover | Home |
|---|---|
| A bug, or work an agent can do | Its own issue (`bug` or `ready-for-agent`), only if the search finds no match. If it matches, add a comment there. |
| A check at the app (look, feel, HiDPI, UHD 50 cost) | A box on the standing queue #219: "standard app check (`docs/checks/app-check.md`), plus <the specifics>". |
| A check that needs Resolve (Project Settings, caches, Sender controls) | A box on the standing queue #120. |
| A minor note nobody needs to act on | A line in the ticket's findings file. |

The queues carry the `queue` label and never close. A box is ticked, with what was seen, when its check is done; a box untouched after about three specs is done or dropped as `wontfix`. A spec has no check tickets of its own beyond what its slices need to build.

## Definition of done

Finish every item, unprompted. Work is not done while any item is open.

### A ticket

1. **Tests green:** `cargo test --workspace` (plain `cargo test` skips the accuracy crate), `make -C sender test`, and `cd harness && python3 -m unittest discover -s tests`.
2. **Records written** where the ticket produces them (a ticket with no visible change and no test findings needs no findings file, ADR or changelog line; the close comment names the records skipped). ADR list, findings index and changelog are checked by `scripts/check-done.sh`. What testing showed in `docs/findings/<ticket>-*.md` with a row in its README; every decision that is hard to reverse, surprising or a real trade-off as an ADR in `docs/adr/`, listed in `README.md` under Documentation; new terms in `GLOSSARY.md`; a line under `## [Unreleased]` in `CHANGELOG.md` for anything a user can see.
3. **Loose ends filed:** every leftover has one home, chosen as in "Loose ends" below, and the findings link to it.
4. **Merged and closed:** squash-merged with one Conventional Commit, pushed, the issue closed with a comment pointing at the commit and findings.
5. **Clean:** the ticket's branches (local and on GitHub) and worktrees deleted; Resolve left on the project that was open, with no scratch projects.

### A Test build

Actions, "release", "Run workflow" on any branch builds the Installer for every platform in `packaging/platforms.txt` and keeps it as a workflow artifact for 30 days (`.tar.gz` and `SHA256SUMS`). It never publishes. The Sender builds without Resolve or a GPU: CUDA runtime from apt, the official OpenFX headers at the tag pinned in `.github/workflows/release.yml`, and `sender/ci/ofxImageEffectExt.h` standing in for Resolve's extension header. To bump OpenFX, change `OFX_REF` and check the Sender still builds.

### A release

1. **Performance check passes, per platform:** `harness/experiments/uhd50_bench` (ADR 0003) on the build being released, on each platform in `packaging/platforms.txt`, before its tag. A platform is added to that file only once its check has passed on that build (GLOSSARY, Release). A Test build gives the Installer to try on a platform's machine first.
2. **Versioned:** the Unreleased lines moved under `## [X.Y.Z] - date` (known limitations in their own section), the version bumped, `chore(release): vX.Y.Z` merged to `main`.
3. **Tagged:** `git tag vX.Y.Z` on that commit and push the tag. CI does the rest: the `release` workflow builds the Installer for every platform in `packaging/platforms.txt`, and its `publish` job fails, publishing nothing, if the tag does not match the workspace version or `CHANGELOG.md` has no section for it (`scripts/check-release-tag.sh`). Otherwise it publishes a GitHub pre-release (while `0.y.z`) named by the version, with the changelog section as notes, a Full changelog link, the platforms included, each Installer and `SHA256SUMS`. A platform not in the list is never attached. Check the Release page afterwards.
4. **Docs current:** README roadmap state, ADR list, findings index; the spec issue closed or its remaining tickets said.
5. **Issues triaged:** finished or obsolete tickets closed with a reason, labels right, links to deleted branches repointed at `main`.
6. **Clean everywhere:** only `main` locally and on GitHub; no worktrees or stashes; no ignored files left in the repo (`git status --ignored`); temporary runs removed from `~/scopecreep-testdata` (keep data the tests and findings use); in Resolve, the user's own projects at the top level and only the `scopecreep-test-*` projects in the `scopecreep` folder.
7. **Launcher works:** the app reinstalled to `~/.local/bin/scopecreep` (README, "Running the app from the app launcher"), and the Sender installed (`make -C sender install`; restart Resolve, and the test projects need ScopeCreep on their `sender` node, README "Test projects").
