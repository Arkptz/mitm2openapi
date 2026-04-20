#!/usr/bin/env bash
# build-gif.sh — Stitch three demo phases into a single GIF
# Usage: ./build-gif.sh [--phase2-only]
# Parameters per ADR Decision 8: gifski 12fps 960px q90, gifsicle -O3 --lossy=80
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$SCRIPT_DIR/out"
DOCS="$SCRIPT_DIR/../../docs"
# Find font: prefer standard path, fall back to nix store
FONT="/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
if [ ! -f "$FONT" ]; then
  FONT=$(find /nix/store -maxdepth 4 -name "DejaVuSans-Bold.ttf" 2>/dev/null | head -1)
fi
if [ -z "$FONT" ] || [ ! -f "$FONT" ]; then
  echo "WARNING: DejaVuSans-Bold.ttf not found, using drawtext without fontfile"
  FONT=""
fi

mkdir -p "$OUT" "$DOCS"

PHASE2_ONLY=false
for arg in "$@"; do
  case $arg in
    --phase2-only) PHASE2_ONLY=true ;;
  esac
done

# Encode a video with a text label overlay.
# Optional 4th arg: playback speed multiplier (default 1). Phase 1 is a long
# UI walkthrough recorded at human pace; we speed it up here for the GIF
# rather than rushing the Playwright test.
encode() {
  local input="$1"
  local output="$2"
  local label="$3"
  local speed="${4:-1}"
  local drawtext_filter
  if [ -n "$FONT" ]; then
    drawtext_filter="drawtext=fontfile='$FONT':text='$label':fontcolor=white:fontsize=24:x=20:y=20:box=1:boxcolor=black@0.5:boxborderw=5"
  else
    drawtext_filter="drawtext=text='$label':fontcolor=white:fontsize=24:x=20:y=20:box=1:boxcolor=black@0.5:boxborderw=5"
  fi
  local speed_filter=""
  if [ "$speed" != "1" ]; then
    speed_filter="setpts=PTS/${speed},"
  fi
  ffmpeg -y -i "$input" \
    -vf "${speed_filter}scale=960:-2,$drawtext_filter" \
    -c:v libx264 -preset fast -crf 23 -an \
    "$output"
}

if [ "$PHASE2_ONLY" = true ]; then
  echo "=== Phase 2 only mode ==="
  encode "$OUT/phase2.mp4" "$OUT/phase2-labeled.mp4" "2/3 Convert flow -> OpenAPI"

  FRAMES_DIR=$(mktemp -d)
  trap 'rm -rf "$FRAMES_DIR"' EXIT
  ffmpeg -y -i "$OUT/phase2-labeled.mp4" -vf "fps=12,scale=960:-2" "$FRAMES_DIR/frame%04d.png"
  gifski --fps 12 --width 960 --quality 90 -o "$OUT/phase2.gif" "$FRAMES_DIR"/frame*.png

  gifsicle -O3 --lossy=80 --output "$OUT/phase2-opt.gif" "$OUT/phase2.gif"
  echo "Phase 2 GIF: $(du -h "$OUT/phase2-opt.gif" | cut -f1)"
  exit 0
fi

echo "=== Encoding phases ==="
encode "$OUT/phase1.webm" "$OUT/phase1-labeled.mp4" "1/3 Capture traffic with mitmproxy" 3
encode "$OUT/phase2.mp4"  "$OUT/phase2-labeled.mp4" "2/3 Convert flow -> OpenAPI"
encode "$OUT/phase3.webm" "$OUT/phase3-labeled.mp4" "3/3 Browse the spec in Swagger UI"

echo "=== Stitching with xfade ==="
# Phase durations (approximate): phase1=10s, phase2=10s, phase3=5s
# xfade offsets are cumulative: offset1 = dur(phase1) - transition_duration
TRANSITION=0.5
P1_DUR=$(ffprobe -v quiet -show_entries format=duration -of csv=p=0 "$OUT/phase1-labeled.mp4")
P2_DUR=$(ffprobe -v quiet -show_entries format=duration -of csv=p=0 "$OUT/phase2-labeled.mp4")

OFFSET1=$(awk "BEGIN {printf \"%.3f\", $P1_DUR - $TRANSITION}")
OFFSET2=$(awk "BEGIN {printf \"%.3f\", $P1_DUR + $P2_DUR - $TRANSITION * 2}")

ffmpeg -y \
  -i "$OUT/phase1-labeled.mp4" \
  -i "$OUT/phase2-labeled.mp4" \
  -i "$OUT/phase3-labeled.mp4" \
  -filter_complex "
    [0][1]xfade=transition=fade:duration=${TRANSITION}:offset=${OFFSET1}[v01];
    [v01][2]xfade=transition=fade:duration=${TRANSITION}:offset=${OFFSET2}[vout]
  " \
  -map "[vout]" -c:v libx264 -preset fast -crf 23 -an \
  "$DOCS/demo.mp4"

echo "=== Generating GIF with gifski ==="
FRAMES_DIR=$(mktemp -d)
trap 'rm -rf "$FRAMES_DIR"' EXIT
ffmpeg -y -i "$DOCS/demo.mp4" -vf "fps=12,scale=960:-2" "$FRAMES_DIR/frame%04d.png"
gifski --fps 12 --width 960 --quality 90 -o "$OUT/demo-raw.gif" "$FRAMES_DIR"/frame*.png

gifsicle -O3 --lossy=80 --output "$DOCS/demo.gif" "$OUT/demo-raw.gif"

GIF_SIZE=$(du -b "$DOCS/demo.gif" | cut -f1)
echo "Final GIF size: $(du -h "$DOCS/demo.gif" | cut -f1)"

# Size guard: if > 9 MB, note it (README patching done in CI workflow)
if [ "$GIF_SIZE" -gt 9437184 ]; then
  echo "WARNING: GIF exceeds 9 MB size budget. CI will patch README to use <video> fallback."
fi
