#!/usr/bin/env bash
set -euo pipefail
STAGE="$1"
case "$STAGE" in
  5) branches=(fix/urlencoding-utf8 fix/unknown-http-methods fix/atomic-output-write chore/remove-param-regex-flag chore/tnetstring-diagnostics);;
  7) branches=(chore/fuzz-tnetstring refactor/error-non-exhaustive chore/cleanup-production-unwraps refactor/share-type-hints fix/schema-heuristics fix/error-recovery-warnings chore/clippy-deny-indexing feat/strict-mode ci/benchmark-regression);;
  *) echo "Unknown stage $STAGE"; exit 1;;
esac
git fetch origin
git checkout main && git pull --ff-only
for b in "${branches[@]}"; do
  dir="$HOME/worktrees/mitm2openapi/$(echo $b | tr / -)"
  git worktree add "$dir" -b "$b" main
done
git worktree list
