#!/usr/bin/env bash
# normalize.sh — Strip OPTIONS preflight and CORS headers from generated spec
# Toolshop's Angular SPA sends CORS preflight to the Laravel API,
# which generates OPTIONS path entries that are noise for our diff.
set -euo pipefail

INPUT="${1:?Usage: normalize.sh <input.yaml> [output.yaml]}"
OUTPUT="${2:-${INPUT%.yaml}-normalized.yaml}"

echo "=== Normalizing $INPUT ==="

# Copy input to output
cp "$INPUT" "$OUTPUT"

# Remove OPTIONS paths (CORS preflight)
# yq approach: delete paths that start with options
if command -v yq &> /dev/null; then
  yq -i 'del(.paths.*.options)' "$OUTPUT"
else
  # Fallback: python/sed-based removal
  echo "WARNING: yq not found, attempting sed-based normalization"
  # Remove blocks starting with '    options:' until next HTTP method or path
  sed -i '/^    options:/,/^    [a-z]*:\|^  \//{/^    options:/d;/^      /d}' "$OUTPUT" || true
fi

# Remove CORS-related headers from all responses
# Access-Control-Allow-Origin, Access-Control-Allow-Methods, etc.
if command -v yq &> /dev/null; then
  yq -i '
    del(.. | select(key == "Access-Control-Allow-Origin")) |
    del(.. | select(key == "Access-Control-Allow-Methods")) |
    del(.. | select(key == "Access-Control-Allow-Headers")) |
    del(.. | select(key == "Access-Control-Max-Age")) |
    del(.. | select(key == "Access-Control-Allow-Credentials"))
  ' "$OUTPUT" 2>/dev/null || true
fi

# Remove Origin header from request parameters
if command -v yq &> /dev/null; then
  yq -i 'del(.paths.*.*.parameters.[] | select(.name == "Origin"))' "$OUTPUT" 2>/dev/null || true
fi

echo "=== Normalized to $OUTPUT ==="
