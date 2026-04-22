#!/usr/bin/env bash
set -euo pipefail
BRANCH="$1"
DIR="$HOME/worktrees/mitm2openapi/$(echo $BRANCH | tr / -)"
cd ~/git/mitmproxy2swagger-rs
git fetch origin --prune
git worktree remove "$DIR"
git branch -D "$BRANCH" 2>/dev/null || true
# Clean up per-worktree cargo state (isolation rules 1 + 2)
rm -rf "/tmp/cargo-home/$(basename "$DIR")" "/tmp/cargo-target/$(basename "$DIR")"
