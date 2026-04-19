#!/usr/bin/env bash
# run-l2.sh — Orchestrator for Level 2 integration test
# Lifecycle: compose up → seed → Playwright scenarios → generate → normalize → diff → compose down
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

FLOW="$SCRIPT_DIR/out/toolshop.flow"
TEMPLATES="$SCRIPT_DIR/out/templates.yaml"
GENERATED="$SCRIPT_DIR/out/generated.yaml"
NORMALIZED="$SCRIPT_DIR/out/generated-normalized.yaml"
BASELINE="$REPO_ROOT/tests/golden/toolshop-openapi-v1.9.json"
PREFIX="http://localhost:8091"

mkdir -p "$SCRIPT_DIR/out"

cleanup() {
  echo "=== Teardown ==="
  cd "$SCRIPT_DIR"
  docker compose down -v || true
}
trap cleanup EXIT

echo "=== Starting Toolshop stack ==="
cd "$SCRIPT_DIR"
docker compose up -d

echo "=== Waiting for services ==="
for i in $(seq 1 90); do
  if docker compose ps laravel-api | grep -q healthy; then
    echo "Services healthy after ${i}s"
    break
  fi
  if [ "$i" -eq 90 ]; then
    echo "ERROR: Services did not become healthy in 90s"
    docker compose logs
    exit 1
  fi
  sleep 1
done

echo "=== Running seed ==="
docker compose exec -T laravel-api php artisan migrate:refresh --seed

echo "=== Running Playwright scenarios ==="
cd "$SCRIPT_DIR"
npx playwright test

echo "=== Discovering endpoints ==="
"$REPO_ROOT/target/release/mitm2openapi" discover \
  -i "$FLOW" \
  -o "$TEMPLATES" \
  -p "$PREFIX"

# Remove 'ignore:' prefix from all discovered endpoints
sed -i 's/^  ignore: /  /g' "$TEMPLATES"

echo "=== Generating OpenAPI spec ==="
"$REPO_ROOT/target/release/mitm2openapi" generate \
  -i "$FLOW" \
  -t "$TEMPLATES" \
  -o "$GENERATED" \
  -p "$PREFIX" \
  --exclude-headers "Origin,Access-Control-Allow-Origin,Access-Control-Allow-Methods,Access-Control-Allow-Headers,Access-Control-Allow-Credentials"

echo "=== Normalizing generated spec ==="
"$SCRIPT_DIR/normalize.sh" "$GENERATED" "$NORMALIZED"

echo "=== Running diff ==="
oasdiff diff "$BASELINE" "$NORMALIZED" --fail-on BREAKING

echo "=== Level 2 integration test PASSED ==="
