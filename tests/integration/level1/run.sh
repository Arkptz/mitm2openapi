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
CURATED_TEMPLATES="$SCRIPT_DIR/fixtures/templates.yaml"
TEMPLATES="$SCRIPT_DIR/out/templates.yaml"
GENERATED="$SCRIPT_DIR/out/generated.yaml"
PREFIX="http://petstore:8080"

mkdir -p "$SCRIPT_DIR/out"

# The mitmproxy:11 image drops privileges to its internal `mitmproxy` user via
# gosu in the entrypoint, ignoring any docker-compose `user:` override. On CI
# runners the bind-mounted fixtures dir is owned by a uid that differs from
# mitmproxy's, so `mitmdump -w /flows/petstore.flow` hits EACCES. World-writable
# perms on the fixture dir + flow file are safe for a test fixture and let any
# container uid write to them.
mkdir -p "$SCRIPT_DIR/fixtures"
touch "$SCRIPT_DIR/fixtures/petstore.flow"
chmod 0777 "$SCRIPT_DIR/fixtures"
chmod 0666 "$SCRIPT_DIR/fixtures/petstore.flow"

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

echo "=== Starting curl sidecar ==="
docker compose --profile test up -d curl

echo "=== Waiting for mitmproxy to accept connections ==="
for i in $(seq 1 60); do
  if docker compose exec -T curl curl -sf --connect-timeout 2 --max-time 5 --proxy http://mitmproxy:8081 -o /dev/null http://petstore:8080/api/v3/openapi.json 2>/dev/null; then
    echo "mitmproxy ready after ${i}s"
    break
  fi
  if [ "$i" -eq 60 ]; then
    echo "ERROR: mitmproxy not accepting connections after 60s"
    echo "--- mitmproxy logs ---"
    docker compose logs mitmproxy || true
    echo "--- container state ---"
    docker compose ps || true
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

echo "=== Generating OpenAPI spec (curated templates) ==="
"$REPO_ROOT/target/release/mitm2openapi" generate \
  -i "$FLOW" \
  -t "$CURATED_TEMPLATES" \
  -o "$GENERATED" \
  -p "$PREFIX"

echo "=== Running diff ==="
if [ "$STRICT" = true ]; then
  "$SCRIPT_DIR/diff-strict.sh" "$REPO_ROOT/tests/golden/petstore-v3.yaml" "$GENERATED"
else
  "$SCRIPT_DIR/diff-naive.sh" "$REPO_ROOT/tests/golden/petstore-v3.yaml" "$GENERATED"
fi

echo "=== Level 1 integration test PASSED ==="
