#!/usr/bin/env bash
# run-l2.sh — Orchestrator for Level 2 integration test (crAPI target)
# Lifecycle: compose up → Playwright scenarios → generate → normalize → diff → compose down
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

FLOW="$SCRIPT_DIR/out/crapi.flow"
TEMPLATES="$SCRIPT_DIR/out/templates.yaml"
GENERATED="$SCRIPT_DIR/out/generated.yaml"
NORMALIZED="$SCRIPT_DIR/out/generated-normalized.yaml"
BASELINE="$REPO_ROOT/tests/golden/crapi-openapi.yaml"
PREFIX="http://crapi-web"

mkdir -p "$SCRIPT_DIR/out"

# mitmproxy:11 entrypoint drops privileges via `gosu mitmproxy`, ignoring any
# compose `user:` override. CI runners own the bind-mounted ./out at a uid that
# differs from the container user, so mitmdump hits EACCES on /flows/crapi.flow
# without these world-writable perms. Safe for test fixtures.
# chmod may fail locally when a prior docker run left files owned by root;
# ignore those failures — they don't affect CI where workspace is runner-owned.
touch "$SCRIPT_DIR/out/crapi.flow" 2>/dev/null || true
chmod 0777 "$SCRIPT_DIR/out" 2>/dev/null || true
chmod 0666 "$SCRIPT_DIR/out/crapi.flow" 2>/dev/null || true

cleanup() {
  echo "=== Teardown ==="
  cd "$SCRIPT_DIR"
  echo "--- mitmproxy logs ---"
  docker compose logs mitmproxy 2>&1 || true
  echo "--- container state ---"
  docker compose ps 2>&1 || true
  docker compose down -v || true
}
trap cleanup EXIT

echo "=== Starting crAPI stack ==="
cd "$SCRIPT_DIR"
docker compose up -d

echo "=== Waiting for crapi-web to become healthy ==="
for i in $(seq 1 120); do
  if docker compose ps crapi-web | grep -q healthy; then
    echo "Services healthy after ${i}s"
    break
  fi
  if [ "$i" -eq 120 ]; then
    echo "ERROR: crapi-web did not become healthy in 120s"
    docker compose logs
    exit 1
  fi
  sleep 1
done

echo "=== Starting playwright sidecar ==="
cd "$SCRIPT_DIR"
docker compose --profile test up -d playwright

echo "=== Running Playwright scenarios via sidecar ==="
docker compose exec -T playwright sh -c 'cd /work/tests/integration/level2 && npm ci --prefer-offline --no-audit && npx playwright test'

echo "=== Discovering endpoints ==="
"$REPO_ROOT/target/release/mitm2openapi" discover \
  -i "$FLOW" \
  -o "$TEMPLATES" \
  -p "$PREFIX"

# Remove 'ignore:' prefix from all discovered endpoints
sed -i 's/ignore://g' "$TEMPLATES"

echo "=== Generating OpenAPI spec ==="
"$REPO_ROOT/target/release/mitm2openapi" generate \
  -i "$FLOW" \
  -t "$TEMPLATES" \
  -o "$GENERATED" \
  -p "$PREFIX"

echo "=== Normalizing generated spec ==="
"$SCRIPT_DIR/normalize.sh" "$GENERATED" "$NORMALIZED"

echo "=== Running diff ==="
OASDIFF_BIN=""
if command -v oasdiff &>/dev/null; then
  OASDIFF_BIN="oasdiff"
else
  for f in /nix/store/*oasdiff*/bin/oasdiff; do [ -x "$f" ] && OASDIFF_BIN="$f" && break; done
fi
if [ -z "$OASDIFF_BIN" ]; then
  echo "ERROR: oasdiff not found"
  exit 1
fi
"$OASDIFF_BIN" breaking "$BASELINE" "$NORMALIZED" --fail-on ERR

echo "=== Level 2 integration test PASSED ==="
