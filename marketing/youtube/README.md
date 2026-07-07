# Tree Ring Memory YouTube Upload Package

This folder contains the upload-ready source package for the first Tree Ring
Memory demo video.

## Primary Video

- Title: `Tree Ring Memory: Local-first memory lifecycle for AI agents`
- Thumbnail: `marketing/assets/youtube-thumbnail-1920x1080.png`
- Description: `description.md`
- Captions: `captions.srt`
- Tags: `tags.txt`
- Voiceover: `voiceover.txt`
- Slide source: `slides.html`
- Build script: `build-demo-video.sh`

## Build

From the repository root:

```bash
sh marketing/youtube/build-demo-video.sh
```

The rendered demo is about two minutes. The script writes render outputs under:

```text
outputs/marketing/youtube-demo/
```

Expected final video:

```text
outputs/marketing/youtube-demo/tree-ring-memory-demo.mp4
```

The script uses:

- Google Chrome for deterministic slide screenshots;
- `say` for local macOS voiceover generation when available;
- `ffmpeg` for final video assembly.

It does not record private shell history, real project memory, or account data.

## Upload Checklist

- Upload `tree-ring-memory-demo.mp4`.
- Set thumbnail to `marketing/assets/youtube-thumbnail-1920x1080.png`.
- Paste `description.md`.
- Upload `captions.srt`.
- Add tags from `tags.txt`.
- Add end-screen/card link to `https://terminallylazy.github.io/Tree-Ring-Memory/`.
- Pin a comment pointing to the feedback issue:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26`.

## Shorts

Use `shorts.md` for three short-form cuts after the main video is live.
