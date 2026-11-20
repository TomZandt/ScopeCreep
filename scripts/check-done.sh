#!/usr/bin/env bash
# Mechanical items of the "Definition of done" in CONTRIBUTING.md.
#   scripts/check-done.sh    run from the repo root; CI sets CI=true to skip the launcher check
# Exits 1 and lists every item that is open.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
fail=0
open() { echo "OPEN: $1" >&2; fail=1; }

# Every ADR is listed in README.md under Documentation.
for f in docs/adr/[0-9]*.md; do
  n=$(basename "$f")
  grep -qF "docs/adr/$n" README.md || open "README.md does not list ADR $n"
done

# Every findings file has a row in the findings index.
for f in docs/findings/[0-9]*.md; do
  n=$(basename "$f")
  grep -qF "($n)" docs/findings/README.md || open "docs/findings/README.md has no row for $n"
done

# User-visible commits since the last tag need a line under [Unreleased].
tag=$(git describe --tags --abbrev=0 2>/dev/null || true)
range=${tag:+$tag..HEAD}
if [ -n "$(git log --first-parent --format=%s ${range:-HEAD} | grep -E '^(feat|fix|perf)(\(|!|:)' || true)" ]; then
  unreleased=$(awk '/^## \[Unreleased\]/{f=1;next} /^## \[/{f=0} f' CHANGELOG.md | grep -c '^- ')
  [ "$unreleased" -gt 0 ] || open "feat/fix/perf commits since ${tag:-the start} but nothing under ## [Unreleased] in CHANGELOG.md"
fi

# The launcher copy of the app is not older than the last commit that touched the app.
if [ -z "${CI:-}" ]; then
  bin="$HOME/.local/bin/scopecreep"
  last=$(git log -1 --format=%ct -- app feed accuracy Cargo.toml Cargo.lock)
  if [ ! -x "$bin" ]; then
    open "launcher app missing: cargo build --release -p scopecreep && install -Dm755 target/release/scopecreep $bin"
  elif [ "$(stat -c %Y "$bin")" -lt "$last" ]; then
    open "launcher app is older than the last app commit: cargo build --release -p scopecreep && install -Dm755 target/release/scopecreep $bin"
  fi
fi

# Checks waiting for a person: open ready-for-human issues plus unticked boxes on the queue issues.
# Past the cap, finish checks before starting another spec. Skipped in CI and without gh.
if [ -z "${CI:-}" ] && command -v gh >/dev/null; then
  cap=${MAX_HUMAN_CHECKS:-25}
  waiting=$(gh issue list --state open --label ready-for-human --limit 300 --json labels,body 2>/dev/null | python3 -c '
import json, sys
n = 0
for i in json.load(sys.stdin):
    if any(l["name"] == "queue" for l in i["labels"]):
        n += sum(1 for l in i["body"].splitlines() if l.lstrip().startswith("- [ ]"))
    else:
        n += 1
print(n)' 2>/dev/null || true)
  if [ -n "$waiting" ]; then
    echo "Checks waiting for a person: $waiting (cap $cap)"
    [ "$waiting" -le "$cap" ] || open "$waiting checks wait for a person (cap $cap): tick or drop some before the next spec"
  fi
fi

[ "$fail" -eq 0 ] && echo "Definition-of-done checks pass."
exit "$fail"
