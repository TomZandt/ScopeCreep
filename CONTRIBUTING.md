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
