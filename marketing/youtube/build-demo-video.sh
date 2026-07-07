#!/bin/sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
OUT_DIR="$ROOT_DIR/outputs/marketing/youtube-demo"
SLIDE_SOURCE="$ROOT_DIR/marketing/youtube/slides.html"
VOICEOVER="$ROOT_DIR/marketing/youtube/voiceover.txt"
CAPTIONS="$ROOT_DIR/marketing/youtube/captions.srt"
DESCRIPTION="$ROOT_DIR/marketing/youtube/description.md"
THUMBNAIL="$ROOT_DIR/marketing/assets/youtube-thumbnail-1920x1080.png"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
VIDEO_DURATION="${VIDEO_DURATION:-132}"

mkdir -p "$OUT_DIR/slides"

if [ ! -x "$CHROME" ]; then
  echo "Google Chrome not found at: $CHROME" >&2
  echo "Set CHROME=/path/to/chrome and rerun." >&2
  exit 1
fi

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "ffmpeg is required." >&2
  exit 1
fi

i=1
while [ "$i" -le 7 ]; do
  "$CHROME" \
    --headless \
    --disable-gpu \
    --hide-scrollbars \
    --window-size=1920,1080 \
    --screenshot="$OUT_DIR/slides/slide-0$i.png" \
    "file://$SLIDE_SOURCE?slide=$i" >/dev/null 2>&1
  i=$((i + 1))
done

cat > "$OUT_DIR/concat.txt" <<EOF
file '$OUT_DIR/slides/slide-01.png'
duration 15
file '$OUT_DIR/slides/slide-02.png'
duration 14
file '$OUT_DIR/slides/slide-03.png'
duration 18
file '$OUT_DIR/slides/slide-04.png'
duration 18
file '$OUT_DIR/slides/slide-05.png'
duration 20
file '$OUT_DIR/slides/slide-06.png'
duration 17
file '$OUT_DIR/slides/slide-07.png'
duration 16
file '$OUT_DIR/slides/slide-07.png'
EOF

if command -v say >/dev/null 2>&1; then
  say -r 148 -o "$OUT_DIR/voiceover.aiff" -f "$VOICEOVER"
  AUDIO_INPUT="$OUT_DIR/voiceover.aiff"
  VIDEO_DURATION="$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$AUDIO_INPUT")"
else
  AUDIO_INPUT=""
fi

if [ -n "$AUDIO_INPUT" ]; then
  ffmpeg -y \
    -f concat -safe 0 -i "$OUT_DIR/concat.txt" \
    -i "$AUDIO_INPUT" \
    -t "$VIDEO_DURATION" \
    -vf "scale=1920:1080,format=yuv420p" \
    -r 30 \
    -c:v libx264 \
    -c:a aac \
    -shortest \
    "$OUT_DIR/tree-ring-memory-demo.mp4"
else
  ffmpeg -y \
    -f concat -safe 0 -i "$OUT_DIR/concat.txt" \
    -t "$VIDEO_DURATION" \
    -vf "scale=1920:1080,format=yuv420p" \
    -r 30 \
    -c:v libx264 \
    "$OUT_DIR/tree-ring-memory-demo.mp4"
fi

cp "$CAPTIONS" "$OUT_DIR/captions.srt"
cp "$DESCRIPTION" "$OUT_DIR/description.md"
cp "$THUMBNAIL" "$OUT_DIR/thumbnail.png"

cat > "$OUT_DIR/upload-checklist.txt" <<EOF
Tree Ring Memory YouTube upload package

Video: $OUT_DIR/tree-ring-memory-demo.mp4
Thumbnail: $OUT_DIR/thumbnail.png
Description: $OUT_DIR/description.md
Captions: $OUT_DIR/captions.srt

Before upload:
- Watch the MP4 end to end.
- Confirm captions line up well enough for first launch.
- Add landing URL in end-screen/card: https://terminallylazy.github.io/Tree-Ring-Memory/
- Pin feedback issue: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26
EOF

echo "Wrote $OUT_DIR/tree-ring-memory-demo.mp4"
