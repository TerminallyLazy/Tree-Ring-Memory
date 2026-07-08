# YouTube Upload Validation

Status: upload-ready but account-gated

Validated on: 2026-07-08 04:41 EDT

Source commit: `141895c056fa460f596ef92f931eda8e7498ada4`

## Generated Package

The upload package was rebuilt from:

```bash
sh marketing/youtube/build-demo-video.sh
```

Generated output directory:

```text
outputs/marketing/youtube-demo/
```

Primary upload files:

- Video: `outputs/marketing/youtube-demo/tree-ring-memory-demo.mp4`
- Thumbnail: `outputs/marketing/youtube-demo/thumbnail.png`
- Captions: `outputs/marketing/youtube-demo/captions.srt`
- Description: `outputs/marketing/youtube-demo/description.md`

## Media Checks

`ffprobe` reported:

- Video: H.264, 1920x1080, 30 fps, 3554 frames
- Audio: AAC, mono, 22050 Hz
- Duration: 118.466667 seconds
- File size: 2894902 bytes
- Overall bitrate: 195491 bit/s

Visual frames checked after rebuild:

- `outputs/marketing/youtube-demo/frame-00-00-02.jpg`
- `outputs/marketing/youtube-demo/frame-00-00-22.jpg`
- `outputs/marketing/youtube-demo/frame-00-00-44.jpg`
- `outputs/marketing/youtube-demo/frame-00-01-06.jpg`
- `outputs/marketing/youtube-demo/frame-00-01-32.jpg`
- `outputs/marketing/youtube-demo/frame-00-01-54.jpg`
- `outputs/marketing/youtube-demo/slides/slide-04.png`

Result: title, problem, ring model, runtime, CLI, privacy, and closing frames are readable and clear of footer overlap.

## Account Blocker

YouTube publishing still requires an owner-controlled YouTube channel session at `https://studio.youtube.com/`. Upload the MP4, thumbnail, description, captions, and tags from `marketing/youtube/`.
