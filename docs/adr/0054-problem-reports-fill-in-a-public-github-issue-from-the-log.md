# Problem reports fill in a public GitHub issue from the log, and the user only has to submit it

Date: 2026-11-27. Amends the "one network call" line in ADR 0048 and the Help menu entry in GLOSSARY.md.

Public builds for Linux, macOS and Windows reach people we cannot ask "what GPU, what OS, what happened?". Report a problem sent only the version and Feed protocol.

## Decision

- The app keeps a rolling, size-capped log file (about 3 MB) in the platform data folder, always on, local only. Errors, warnings and panics are marked so they can be pulled out on their own. A panic hook writes a **Crash report** to disk.
- Logging runs through the whole app on one layer with levels, replacing bare `eprintln`, and the Resolve helper writes to the same folder. The Sender writes its own small capped log into that folder too (its load inside Resolve, the Resolve and host versions, the GPU path, Feed creation, ring growth, drops and errors), and Diagnostics include its last lines. It is a separate slice so a late Sender log does not hold the release.
- Nothing is intrusive. After a crash, or when the log holds errors the user has not seen reported, the next launch shows one quiet line with two buttons (Report, Dismiss), never a modal and never twice for the same thing.
- Report opens the prefilled GitHub issue (the one Report a problem already opens) with the Diagnostics filled in: versions, OS and GPU, and the errors, warnings or crash backtrace pulled from the log, redacted. The user reads it on GitHub and presses Submit; there is no typing. If it is too long for a URL, the full text goes on the clipboard with a line saying to paste it.
- The wording says plainly what happens: "This fills in a GitHub issue for the developer to fix the problem. The details are used only to fix bugs, nothing else." Because the user sees the whole report before it is public, this is the consent, and there is no first-run question.
- We run no server and ship no token. The only network calls stay the Update check and the browser opening GitHub.
- Issues on the public repo are public, so Diagnostics redact the home folder and user name and never include project or clip names, paths or picture data: colourists work under NDA. No zip attachment, for the same reason.

## Considered options

- **A relay we host that files issues without a GitHub account.** Truly automatic, but we would own hosting, abuse limits and the privacy duty for strangers' data. The owner chose GitHub and the public repo only. Revisit if the GitHub account requirement visibly costs reports.
- **A first-run opt-in, then silent sending.** Needs the relay above.
- **A token in the binary posting straight to GitHub.** Anyone can extract it from a public build, like a key under the mat of a public building.
- **Auto-sending crashes by default.** Rejected: unprompted upload of strangers' data.
- **Native minidumps** (GPU driver and Resolve-side crashes). Deferred as a per-platform job.

## Consequences

- A user needs a GitHub account and one click to Submit, so some people will dismiss it; something is better than the nothing we get today.
- A hard crash inside Resolve or the GPU driver leaves only the Sender log's last lines; native crash dumps are a later job.
- The preview window offers two optional buttons, Save a frame and Show log folder. The user attaches either by dragging it into the issue; the app attaches nothing, and a line says issues are public and client footage must not be attached.
