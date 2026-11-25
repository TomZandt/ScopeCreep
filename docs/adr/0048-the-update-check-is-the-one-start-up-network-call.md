# The Update check is the one start-up network call, and the user can turn it off

Date: 2026-11-25 (spec #234). Amends the "never goes online by itself" lines in ADR 0045 and the Help menu entry in GLOSSARY.md. See ADR 0043.

ScopeCreep never went online by itself. Testers then ran old builds and reported bugs already fixed, because nothing told them a newer Release existed.

## Decision

- At start-up the app makes one **Update check**: a single unauthenticated, read-only HTTPS GET of GitHub's list of Releases for TomZandt/ScopeCreep. It runs on its own thread with a short timeout (5 s), so neither the Feed nor the first frame waits for it.
- It sends nothing about the user or the project. The User-Agent is the generic `ScopeCreep/X.Y.Z`.
- Any failure (offline, blocked, rate-limited, malformed reply) says nothing.
- **Check for updates at start-up** is a setting, on by default. With it off the app never goes online by itself, and the Help menu's Check for updates entry runs a check on demand.
- The check is the only exception. The GitHub address lives in `links.rs`, with the Help menu's links.
- Only Releases that include the running platform's Installer are offered. Drafts and Test builds never are.
- The client is `ureq` with rustls, so there is no OpenSSL to bundle.

## Considered options

- **GitHub's "latest release" address.** It skips pre-releases, and every 0.y.z Release is one, so it would always answer "nothing".
- **Off by default.** Testers, the people it is for, would never see it.
- **Checking again during a session.** Rejected: nothing should interrupt a grade or a client session.

## Consequences

- The app now makes one network request at start-up unless the setting is off. Air-gapped suites should turn it off.
- Installing the update in place is a separate decision (#259).
