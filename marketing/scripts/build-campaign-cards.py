#!/usr/bin/env python3
"""Build deterministic launch campaign cards from checked-in brand assets."""

from __future__ import annotations

from pathlib import Path
from textwrap import wrap

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parents[2]
ASSET_DIR = ROOT / "marketing" / "assets"

NAVY = "#001c30"
CREAM = "#fff0d3"
WHITE = "#fffaf0"
TEAL = "#018695"
CORAL = "#f14668"
ORANGE = "#ff7718"
GOLD = "#f7b842"


def font(size: int, bold: bool = False) -> ImageFont.FreeTypeFont:
    candidates = [
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf" if bold else "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Helvetica Bold.ttf" if bold else "/System/Library/Fonts/Supplemental/Helvetica.ttf",
        "/Library/Fonts/Arial Bold.ttf" if bold else "/Library/Fonts/Arial.ttf",
    ]
    for candidate in candidates:
        path = Path(candidate)
        if path.exists():
            return ImageFont.truetype(str(path), size)
    return ImageFont.load_default()


def rounded_box(draw: ImageDraw.ImageDraw, xy: tuple[int, int, int, int], fill: str, outline: str | None = None) -> None:
    draw.rounded_rectangle(xy, radius=34, fill=fill, outline=outline, width=4 if outline else 1)


def draw_wrapped(
    draw: ImageDraw.ImageDraw,
    text: str,
    xy: tuple[int, int],
    face: ImageFont.FreeTypeFont,
    fill: str,
    width_chars: int,
    line_gap: int,
) -> int:
    x, y = xy
    for line in wrap(text, width=width_chars):
        draw.text((x, y), line, font=face, fill=fill)
        y += face.size + line_gap
    return y


def fit_text(draw: ImageDraw.ImageDraw, text: str, max_width: int, start_size: int, bold: bool = True) -> ImageFont.FreeTypeFont:
    size = start_size
    while size >= 28:
        face = font(size, bold=bold)
        left, _, right, _ = draw.textbbox((0, 0), text, font=face)
        if right - left <= max_width:
            return face
        size -= 2
    return font(28, bold=bold)


def build_card(
    out_name: str,
    base_name: str,
    headline: str,
    subhead: str,
    command: str,
    badge: str,
    accent: str,
    square: bool = False,
) -> None:
    image = Image.open(ASSET_DIR / base_name).convert("RGB")
    draw = ImageDraw.Draw(image)
    width, height = image.size

    margin = int(width * 0.06)
    box_width = int(width * (0.55 if not square else 0.78))
    box_height = int(height * (0.72 if not square else 0.58))
    box = (margin, height - margin - box_height, margin + box_width, height - margin)

    overlay = Image.new("RGBA", image.size, (0, 0, 0, 0))
    overlay_draw = ImageDraw.Draw(overlay)
    rounded_box(overlay_draw, box, (255, 250, 240, 238), accent)
    image = Image.alpha_composite(image.convert("RGBA"), overlay).convert("RGB")
    draw = ImageDraw.Draw(image)

    badge_face = font(26 if not square else 24, bold=True)
    title_face = fit_text(draw, headline, box_width - 76, 72 if not square else 64, bold=True)
    sub_face = font(31 if not square else 28)
    mono_face = font(25 if not square else 22, bold=True)

    x = box[0] + 38
    y = box[1] + 34
    badge_box = (x, y, x + int(box_width * 0.52), y + 44)
    draw.rounded_rectangle(badge_box, radius=22, fill=accent)
    draw.text((x + 20, y + 9), badge, font=badge_face, fill=WHITE)

    y += 72
    draw.text((x, y), headline, font=title_face, fill=NAVY)
    y += title_face.size + 18
    y = draw_wrapped(draw, subhead, (x, y), sub_face, NAVY, 31 if not square else 30, 8)
    y += 22

    command_box = (x, y, box[2] - 38, min(y + 118, box[3] - 34))
    draw.rounded_rectangle(command_box, radius=18, fill=NAVY)
    draw.multiline_text(
        (command_box[0] + 20, command_box[1] + 18),
        command,
        font=mono_face,
        fill=GOLD,
        spacing=8,
    )

    image.save(ASSET_DIR / out_name, optimize=True)


def main() -> None:
    build_card(
        "homebrew-install-card-1200x675.png",
        "open-graph-1200x675.png",
        "Install with Homebrew",
        "macOS ARM64 tap for the Tree Ring Memory launch preview.",
        "brew tap TerminallyLazy/tree-ring\nbrew install tree-ring",
        "DISTRIBUTION",
        TEAL,
    )
    build_card(
        "rust-article-card-1200x675.png",
        "open-graph-1200x675.png",
        "Rust-native agent memory",
        "Workspace split, SQLite/FTS recall, explicit writes, audit, and Ratatui.",
        "read the Rust article",
        "RUST TOOLING",
        ORANGE,
    )
    build_card(
        "twir-submission-card-1200x675.png",
        "open-graph-1200x675.png",
        "Submitted to This Week in Rust",
        "Project/tooling update for Tree Ring Memory's Rust-native CLI.",
        "PR #8346",
        "COMMUNITY",
        CORAL,
    )
    build_card(
        "not-transcript-dump-card-1080x1080.png",
        "social-square-banner-1080x1080.png",
        "Not a transcript dump",
        "A lifecycle layer for scoped, explainable, auditable agent memory.",
        "tree-ring recall",
        "AGENT MEMORY",
        GOLD,
        square=True,
    )


if __name__ == "__main__":
    main()
