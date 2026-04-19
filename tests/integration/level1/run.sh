#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
STRICT=false

for arg in "$@"; do
  case $arg in
    --strict) STRICT=true ;;
  esac
done

FLOW="$SCRIPT_DIR/fixtures/petstore.flow"
TEMPLATES="$SCRIPT_DIR/out/templates.yaml"
GENERATED="$SCRIPT_DIR/out/generated.yaml"
PREFIX="http://localhost:8080"

mkdir -p "$SCRIPT_DIR/out"

cleanup() {
  echo "=== Teardown ==="
  cd "$SCRIPT_DIR"
  docker compose down -v || true
}
trap cleanup EXIT

echo "=== Starting Petstore + mitmproxy ==="
cd "$SCRIPT_DIR"
docker compose up -d

echo "=== Waiting for Petstore healthcheck ==="
for i in $(seq 1 60); do
  if docker compose ps petstore | grep -q "healthy"; then
    echo "Petstore healthy after ${i}s"
    break
  fi
  if [ "$i" -eq 60 ]; then
    echo "ERROR: Petstore did not become healthy in 60s"
    exit 1
  fi
  sleep 1
done

echo "=== Running seed ==="
"$REPO_ROOT/ci/petstore/seed.sh"

echo "=== Discovering endpoints ==="
"$REPO_ROOT/target/release/mitm2openapi" discover \
  -i "$FLOW" \
  -o "$TEMPLATES" \
  -p "$PREFIX"

sed -i 's/^  ignore: /  /g' "$TEMPLATES"

echo "=== Generating OpenAPI spec ==="
"$REPO_ROOT/target/release/mitm2openapi" generate \
  -i "$FLOW" \
  -t "$TEMPLATES" \
  -o "$GENERATED" \
  -p "$PREFIX"

echo "=== Running diff ==="
if [ "$STRICT" = true ]; then
  "$SCRIPT_DIR/diff-strict.sh" "$REPO_ROOT/tests/golden/petstore-v3.yaml" "$GENERATED"
else
  "$SCRIPT_DIR/diff-naive.sh" "$REPO_ROOT/tests/golden/petstore-v3.yaml" "$GENERATED"
fi

echo "=== Level 1 integration test PASSED ==="
