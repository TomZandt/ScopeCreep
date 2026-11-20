#!/usr/bin/env bash
# File a loose end as an issue only if nothing open already covers it ("Loose ends", CONTRIBUTING.md).
#   scripts/file-loose-end.sh --title "..." --body-file f.md [--label bug] [--label ready-for-agent] [--new]
# Prints open issues and queue boxes sharing words with the title; exits 1 without creating when any match,
# unless --new says the match was read and does not cover it.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
title="" body="" new=0 labels=()
while [ $# -gt 0 ]; do
  case "$1" in
    --title) title=$2; shift 2;;
    --body-file) body=$2; shift 2;;
    --label) labels+=(--label "$2"); shift 2;;
    --new) new=1; shift;;
    *) echo "unknown argument: $1" >&2; exit 2;;
  esac
done
[ -n "$title" ] && [ -n "$body" ] || { echo "usage: $0 --title T --body-file F [--label L] [--new]" >&2; exit 2; }
matches=$(gh issue list --state open --limit 300 --json number,title,labels,body | TITLE="$title" python3 -c '
import json, os, re, sys
stop = set("the a an and or of to in on at for with from is are be by that this when with check new add".split())
words = lambda t: {w for w in re.findall(r"[a-z0-9]{4,}", t.lower()) if w not in stop}
want = words(os.environ["TITLE"])
for i in json.load(sys.stdin):
    queue = any(l["name"] == "queue" for l in i["labels"])
    texts = [("issue", i["title"])]
    if queue:
        texts += [("box", l) for l in i["body"].splitlines() if l.lstrip().startswith("- [ ]")]
    for kind, t in texts:
        shared = want & words(t)
        if len(shared) >= 2 or (want and len(shared) == len(want)):
            num = i["number"]; hit = ", ".join(sorted(shared))
            print("#%s (%s) %s  [%s]" % (num, kind, t[:110], hit))
')
if [ -n "$matches" ]; then
  echo "Open items that may already cover this:"; echo "$matches"
  if [ "$new" -ne 1 ]; then
    echo "Comment on or tick one of them, or rerun with --new if none covers it." >&2
    exit 1
  fi
fi
gh issue create --title "$title" --body-file "$body" "${labels[@]}"
