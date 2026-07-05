from __future__ import annotations

import math
import random
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"

BG_TOP = (22, 18, 38)
BG_BOTTOM = (8, 10, 22)
CYAN = (77, 231, 223)
MAGENTA = (246, 74, 186)
GOLD = (255, 199, 87)
CREAM = (255, 239, 194)
WOOD_DARK = (80, 43, 28)
WOOD_MID = (165, 92, 45)
WOOD_LIGHT = (232, 154, 75)


def font(name: str, size: int) -> ImageFont.FreeTypeFont:
    candidates = [
        Path("/System/Library/Fonts/Supplemental") / name,
        Path("/Library/Fonts") / name,
        Path("/System/Library/Fonts") / name,
    ]
    for path in candidates:
        if path.exists():
            return ImageFont.truetype(str(path), size)
    return ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial Black.ttf", size)


FONT_DISPLAY = "/System/Library/Fonts/Supplemental/Phosphate.ttc"
FONT_BODY = "/System/Library/Fonts/Supplemental/Futura.ttc"
FONT_BOLD = "/System/Library/Fonts/Supplemental/Arial Black.ttf"


def gradient(size: tuple[int, int]) -> Image.Image:
    width, height = size
    image = Image.new("RGB", size)
    pixels = image.load()
    for y in range(height):
        t = y / max(height - 1, 1)
        for x in range(width):
            vx = x / max(width - 1, 1)
            glow = max(0, 1 - math.hypot(vx - 0.72, t - 0.34) * 1.75)
            r = int(BG_TOP[0] * (1 - t) + BG_BOTTOM[0] * t + glow * 22)
            g = int(BG_TOP[1] * (1 - t) + BG_BOTTOM[1] * t + glow * 12)
            b = int(BG_TOP[2] * (1 - t) + BG_BOTTOM[2] * t + glow * 48)
            pixels[x, y] = (min(r, 255), min(g, 255), min(b, 255))
    return image


def glow_line(layer: Image.Image, points: list[tuple[int, int]], color: tuple[int, int, int], width: int) -> None:
    draw = ImageDraw.Draw(layer)
    for scale, alpha in [(5, 34), (3, 58), (1, 225)]:
        rgba = (*color, alpha)
        draw.line(points, fill=rgba, width=max(1, width * scale), joint="curve")


def rink_floor(base: Image.Image, horizon: int) -> None:
    width, height = base.size
    glow = Image.new("RGBA", base.size, (0, 0, 0, 0))
    colors = [CYAN, MAGENTA, GOLD]
    vanishing = (int(width * 0.62), horizon)
    for index, x in enumerate(range(-width // 3, width + width // 3, width // 8)):
        color = colors[index % len(colors)]
        glow_line(glow, [(x, height + 80), vanishing], color, 2)
    for index, y in enumerate(range(horizon + 38, height + 140, 54)):
        t = (y - horizon) / max(height - horizon, 1)
        inset = int(40 + t * 220)
        color = colors[index % len(colors)]
        glow_line(glow, [(inset, y), (width - inset // 2, y + int(t * 24))], color, 2)
    base.alpha_composite(glow)


def starbursts(draw: ImageDraw.ImageDraw, size: tuple[int, int], count: int, seed: int) -> None:
    random.seed(seed)
    width, height = size
    for _ in range(count):
        x = random.randint(24, width - 24)
        y = random.randint(20, height - 20)
        radius = random.choice([3, 4, 5, 7])
        color = random.choice([CYAN, MAGENTA, GOLD, CREAM])
        draw.line([(x - radius, y), (x + radius, y)], fill=(*color, 130), width=1)
        draw.line([(x, y - radius), (x, y + radius)], fill=(*color, 130), width=1)
        if radius > 4:
            draw.ellipse((x - 1, y - 1, x + 1, y + 1), fill=(*color, 220))


def ring_disk(size: int, seed: int) -> Image.Image:
    random.seed(seed)
    pad = int(size * 0.09)
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.ellipse((pad - 26, pad - 26, size - pad + 26, size - pad + 26), outline=(*CYAN, 70), width=32)
    gd.ellipse((pad - 42, pad - 42, size - pad + 42, size - pad + 42), outline=(*MAGENTA, 54), width=20)
    glow = glow.filter(ImageFilter.GaussianBlur(14))
    image.alpha_composite(glow)

    draw = ImageDraw.Draw(image)
    bbox = (pad, pad, size - pad, size - pad)
    draw.ellipse(bbox, fill=WOOD_DARK + (255,), outline=(*GOLD, 230), width=max(3, size // 80))
    center = size // 2
    max_radius = (size - pad * 2) // 2 - 4

    for step in range(max_radius, 18, -max(10, size // 34)):
        jitter = random.randint(-4, 5)
        inset = center - step + jitter
        color_pick = random.choice([WOOD_MID, WOOD_LIGHT, GOLD, (112, 62, 35)])
        alpha = random.randint(130, 235)
        line_width = random.choice([2, 3, 4, 5])
        draw.ellipse(
            (inset, inset + random.randint(-3, 3), size - inset, size - inset + random.randint(-3, 3)),
            outline=(*color_pick, alpha),
            width=line_width,
        )

    for angle, color in [(28, MAGENTA), (112, CYAN), (226, GOLD)]:
        r1 = max_radius * 0.16
        r2 = max_radius * 0.86
        a = math.radians(angle)
        x1 = int(center + math.cos(a) * r1)
        y1 = int(center + math.sin(a) * r1)
        x2 = int(center + math.cos(a) * r2)
        y2 = int(center + math.sin(a) * r2)
        draw.line([(x1, y1), (x2, y2)], fill=(*color, 190), width=max(2, size // 120))

    draw.ellipse((center - 32, center - 26, center + 34, center + 28), fill=(255, 184, 91, 245))
    draw.ellipse((center - 17, center - 14, center + 18, center + 15), fill=(62, 34, 30, 230))
    return image


def neon_text(
    base: Image.Image,
    xy: tuple[int, int],
    text: str,
    font_obj: ImageFont.FreeTypeFont,
    fill: tuple[int, int, int] = CREAM,
    glow_color: tuple[int, int, int] = MAGENTA,
    stroke: int = 2,
) -> None:
    x, y = xy
    layer = Image.new("RGBA", base.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    for blur, alpha, offset in [(12, 120, 0), (5, 150, 0), (0, 255, 0)]:
        temp = Image.new("RGBA", base.size, (0, 0, 0, 0))
        td = ImageDraw.Draw(temp)
        td.text(
            (x + offset, y + offset),
            text,
            font=font_obj,
            fill=(*fill, alpha),
            stroke_width=stroke,
            stroke_fill=(*glow_color, alpha),
        )
        if blur:
            temp = temp.filter(ImageFilter.GaussianBlur(blur))
        layer.alpha_composite(temp)
    base.alpha_composite(layer)


def make_logo() -> Image.Image:
    size = 1024
    base = gradient((size, size)).convert("RGBA")
    rink_floor(base, 610)
    draw = ImageDraw.Draw(base)
    starbursts(draw, base.size, 54, 42)
    disk = ring_disk(620, 7)
    base.alpha_composite(disk, (202, 126))

    title_font = ImageFont.truetype(FONT_DISPLAY, 86)
    body_font = ImageFont.truetype(FONT_BOLD, 35)
    neon_text(base, (105, 778), "TREE RING", title_font, CREAM, CYAN, 2)
    neon_text(base, (228, 870), "MEMORY", title_font, GOLD, MAGENTA, 2)
    ImageDraw.Draw(base).text((294, 949), "LOCAL-FIRST AGENT RECALL", font=body_font, fill=(*CREAM, 210))
    return base


def make_banner() -> Image.Image:
    width, height = 1600, 640
    base = gradient((width, height)).convert("RGBA")
    rink_floor(base, 390)
    draw = ImageDraw.Draw(base)
    starbursts(draw, base.size, 70, 77)

    disk = ring_disk(475, 11)
    base.alpha_composite(disk, (105, 88))

    title_font = ImageFont.truetype(FONT_DISPLAY, 120)
    title_font_2 = ImageFont.truetype(FONT_DISPLAY, 132)
    body_font = ImageFont.truetype(FONT_BOLD, 36)
    small_font = ImageFont.truetype(FONT_BOLD, 24)

    neon_text(base, (594, 112), "TREE RING", title_font, CREAM, CYAN, 2)
    neon_text(base, (596, 244), "MEMORY", title_font_2, GOLD, MAGENTA, 2)
    draw.rounded_rectangle((612, 420, 1438, 490), radius=20, fill=(16, 20, 38, 188), outline=(*CYAN, 150), width=2)
    draw.text((646, 436), "LOCAL-FIRST AGENT MEMORY FRAMEWORK", font=body_font, fill=(*CREAM, 235))
    draw.text((650, 512), "rings  scars  seeds  heartwood  recall", font=small_font, fill=(*MAGENTA, 230))

    for i, color in enumerate([CYAN, MAGENTA, GOLD]):
        y = 565 + i * 18
        draw.line([(628, y), (1310 + i * 42, y - 34)], fill=(*color, 180), width=4)
    return base


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    logo = make_logo()
    banner = make_banner()
    logo.save(ASSETS / "tree-ring-memory-logo.png", optimize=True)
    banner.save(ASSETS / "tree-ring-memory-banner.png", optimize=True)


if __name__ == "__main__":
    main()
