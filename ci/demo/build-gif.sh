#!/usr/bin/env bash
# build-gif.sh — Stitch three demo phases into a single GIF
# Usage: ./build-gif.sh [--phase2-only]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$SCRIPT_DIR/out"
DOCS="$SCRIPT_DIR/../../docs"
FONT="/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"

mkdir -p "$OUT" "$DOCS"

PHASE2_ONLY=false
for arg in "$@"; do
  case $arg in
    --phase2-only) PHASE2_ONLY=true ;;
  esac
done

# Encode a video with a text label overlay
encode() {
  local input="$1"
  local output="$2"
  local label="$3"
  ffmpeg -y -i "$input" \
    -vf "scale=960:-2,drawtext=fontfile='$FONT':text='$label':fontcolor=white:fontsize=24:x=20:y=20:box=1:boxcolor=black@0.5:boxborderw=5" \
    -c:v libx264 -preset fast -crf 23 -an \
    "$output"
}

if [ "$PHASE2_ONLY" = true ]; then
  echo "=== Phase 2 only mode ==="
  encode "$OUT/phase2.mp4" "$OUT/phase2-labeled.mp4" "2/3 Convert flow -> OpenAPI"

  # Generate GIF from phase2 only
  gifski --fps 12 --width 960 --quality 90 -o "$OUT/phase2.gif" \
    <(ffmpeg -y -i "$OUT/phase2-labeled.mp4" -vf "fps=12,scale=960:-2" -f image2pipe -vcodec ppm -)

  gifsicle -O3 --lossy=80 --output "$OUT/phase2-opt.gif" "$OUT/phase2.gif"
  echo "Phase 2 GIF: $(du -h "$OUT/phase2-opt.gif" | cut -f1)"
  exit 0
fi

echo "=== Encoding phases ==="
encode "$OUT/phase1.webm" "$OUT/phase1-labeled.mp4" "1/3 Capture traffic with mitmproxy"
encode "$OUT/phase2.mp4"  "$OUT/phase2-labeled.mp4" "2/3 Convert flow -> OpenAPI"
encode "$OUT/phase3.webm" "$OUT/phase3-labeled.mp4" "3/3 Browse the spec in Swagger UI"

echo "=== Stitching with xfade ==="
# Phase durations (approximate): phase1=10s, phase2=10s, phase3=5s
# xfade offsets are cumulative: offset1 = dur(phase1) - transition_duration
TRANSITION=0.5
P1_DUR=$(ffprobe -v quiet -show_entries format=duration -of csv=p=0 "$OUT/phase1-labeled.mp4")
P2_DUR=$(ffprobe -v quiet -show_entries format=duration -of csv=p=0 "$OUT/phase2-labeled.mp4")

OFFSET1=$(echo "$P1_DUR - $TRANSITION" | bc)
OFFSET2=$(echo "$P1_DUR + $P2_DUR - $TRANSITION * 2" | bc)

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
ffmpeg -y -i "$DOCS/demo.mp4" -vf "fps=12,scale=960:-2" -f image2pipe -vcodec ppm - | \
  gifski --fps 12 --width 960 --quality 90 -o "$OUT/demo-raw.gif" -

gifsicle -O3 --lossy=80 --output "$DOCS/demo.gif" "$OUT/demo-raw.gif"

GIF_SIZE=$(du -b "$DOCS/demo.gif" | cut -f1)
echo "Final GIF size: $(du -h "$DOCS/demo.gif" | cut -f1)"

# Size guard: if > 9 MB, note it (README patching done in CI workflow)
if [ "$GIF_SIZE" -gt 9437184 ]; then
  echo "WARNING: GIF exceeds 9 MB size budget. CI will patch README to use <video> fallback."
fi
