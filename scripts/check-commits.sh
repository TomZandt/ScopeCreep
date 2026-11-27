#!/usr/bin/env bash
# Check commit messages against Conventional Commits (see CONTRIBUTING.md).
#   scripts/check-commits.sh --message-file FILE   check one message (commit-msg hook)
#   scripts/check-commits.sh RANGE                 check first-parent commits in a git range, e.g. origin/main..HEAD
set -euo pipefail

pattern='^(feat|fix|perf|refactor|test|docs|build|ci|chore|revert)(\([a-z0-9-]+\))?!?: [^ ].*[^.]$'

check_subject() {
  local subject="$1" where="$2"
  if [[ ! "$subject" =~ $pattern ]]; then
    echo "Not a Conventional Commit ($where): $subject" >&2
    echo "  Expected: type(scope): summary   e.g. feat(app): show the luma waveform" >&2
    echo "  Types: feat fix perf refactor test docs build ci chore revert. See CONTRIBUTING.md." >&2
    return 1
  fi
}

if [[ "${1:-}" == "--message-file" ]]; then
  subject=$(grep -v '^#' "$2" | awk 'NF {print; exit}')
  check_subject "$subject" "this commit"
  exit
fi

range="${1:?usage: check-commits.sh RANGE | --message-file FILE}"
status=0
while IFS=' ' read -r sha subject; do
  check_subject "$subject" "${sha:0:7}" || status=1
done < <(git log --first-parent --format='%H %s' "$range")
exit $status
