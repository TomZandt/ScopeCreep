# The first Release through the pipeline (spec #233, ticket #244)

v0.4.0 (2026-11-25) was the first Release made by CI: the `v0.4.0` tag ran `release.yml`, whose `platforms`, `build (linux)` and `publish` jobs all passed, and published a pre-release carrying `scopecreep-0.4.0-linux-x86_64.tar.gz` and `SHA256SUMS`.

| Check | Result |
|---|---|
| Download signed out from the public Releases page, `sha256sum -c SHA256SUMS` | OK |
| `install.sh install` (app to `~/.local/bin`, Sender to `/usr/OFX/Plugins`, asks for admin rights, says to restart Resolve) | Worked; the installed Sender is byte-identical to the one in the Release |
| `scopecreep --version` | `ScopeCreep 0.4.0 (Feed protocol 1.2)` |
| ScopeCreep appears in Resolve, the app shows the picture | Yes (Resolve 21 Studio, Linux, CUDA) |
| `uhd50_bench` (ADR 0003), project `scopecreep-test-bench50`, ScopeCreep on the Sender timeline's `sender` node, each configuration run on its own with the installed app | Passed (below) |

| Configuration | Resolve's fps readout (12 grabs over 60 s) | Sender renders/s |
|---|---|---|
| Sender off | 50 in all 12 | 0.00 |
| Sender on, no app | 50 in all readable grabs (one black grab) | 50.00 |
| Sender on, with the app | 50 in all 12 | 50.00; the app logged 3765 frames, 3706 shown |

Linux is the only platform in `packaging/platforms.txt`. The tag-publish path was also tried with a throwaway tag first: a tag that did not match the workspace version failed at "Check the tag" with nothing built or published, and `v0.3.99` published a pre-release whose notes began with the platform list and ended with the Full changelog link (see [242](242-release-ci.md)).
