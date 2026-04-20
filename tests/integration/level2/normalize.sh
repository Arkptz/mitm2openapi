#!/usr/bin/env bash
# normalize.sh — Filter generated spec to only API paths (crAPI target)
set -euo pipefail

INPUT="${1:?Usage: normalize.sh <input.yaml> [output.yaml]}"
OUTPUT="${2:-${INPUT%.yaml}-normalized.yaml}"

echo "=== Normalizing $INPUT ==="

cp "$INPUT" "$OUTPUT"

# Find yq: prefer system PATH, then nix store (yq-go)
YQ_BIN=""
if command -v yq &>/dev/null; then
  YQ_BIN="yq"
else
  NIX_YQ=""
  for f in /nix/store/*yq-go*/bin/yq; do [ -x "$f" ] && NIX_YQ="$f" && break; done
  if [ -n "$NIX_YQ" ]; then
    YQ_BIN="$NIX_YQ"
  fi
fi

if [ -n "$YQ_BIN" ]; then
  # Delete static/non-API paths
  "$YQ_BIN" -i '
    del(.paths[] | select(
      (key | test("^/static/")) or
      (key | test("^/images/")) or
      (key | test("^/mailhog/")) or
      (key | test("\\.js$")) or
      (key | test("\\.css$")) or
      (key | test("\\.map$")) or
      (key | test("\\.(woff|woff2|ttf|svg|png|ico|webp|gif|jpg|jpeg)$")) or
      (key | test("^/health$")) or
      (key | test("^/manifest\\.json$")) or
      (key | test("^/robots\\.txt$")) or
      (key | test("^/favicon\\.ico$")) or
      (key | test("^/sockjs-node")) or
      (key == "/")
    ))
  ' "$OUTPUT" 2>/dev/null || true
else
  echo "WARNING: yq not found, skipping static-asset path filtering"
fi

# Normalize session-specific path segments to {id}
# Matches path components that are 15+ alphanumeric characters (UUIDs, post IDs, etc.)
# This ensures golden comparison is stable across runs
sed -E \
  -e 's|/([A-Za-z0-9]{15,})([[:space:]]*:)$|/{id}\2|g' \
  -e 's|/([A-Za-z0-9]{15,})$|/{id}|g' \
  "$OUTPUT" >/tmp/normalize-tmp.yaml && mv /tmp/normalize-tmp.yaml "$OUTPUT"

echo "=== Normalized to $OUTPUT ==="
