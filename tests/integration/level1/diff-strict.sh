#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

BASELINE="${1:-$REPO_ROOT/tests/golden/petstore-v3.yaml}"
GENERATED="${2:-$SCRIPT_DIR/out/generated.yaml}"

echo "=== Strict diff (breaking --fail-on WARN) ==="
echo "Baseline: $BASELINE"
echo "Generated: $GENERATED"

oasdiff breaking "$BASELINE" "$GENERATED" --fail-on WARN
echo "=== Strict diff PASSED ==="
