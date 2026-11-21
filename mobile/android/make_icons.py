#!/usr/bin/env python3
"""Generate the app's launcher icons from Kitty's art: python3 make_icons.py

Writes, for every density, the legacy icons (ic_launcher.png, ic_launcher_round.png: Kitty's face
on a warm round badge) and the adaptive icon's foreground (ic_launcher_foreground.png), whose
background is drawable/ic_launcher_background.xml. The PNGs are committed; rerun this only when
the art changes.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

HERE = Path(__file__).resolve().parent
ART = HERE.parent.parent / "assets" / "kitty.png"
RES = HERE / "app" / "src" / "main" / "res"

# Kitty's head in kitty.png (978x723, Kitty leaping to the right): everything right of her tail
# down to the cheeks, then her chin fluff, which an ellipse picks out from her body and paws.
HEAD_TOP = [(385, 0), (978, 0), (978, 445), (430, 445), (385, 400)]
CHIN = (430, 320, 950, 540)
HEAD_SOFTNESS = 5

# Launcher icon sizes in px per density: legacy icons are 48 dp, adaptive layers 108 dp.
DENSITIES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0, "xxhdpi": 3.0, "xxxhdpi": 4.0}
LEGACY_DP = 48
ADAPTIVE_DP = 108

# The badge: warm cream in the middle to marigold at the edge, with a darker rim. Keep in step
# with drawable/ic_launcher_background.xml.
CENTRE = (255, 240, 196)
EDGE = (255, 190, 92)
RIM = (226, 132, 52)

# Drawn this many times larger, then scaled down, for smooth edges.
SUPERSAMPLE = 4


def radial_badge(size: int, diameter: float) -> Image.Image:
    """A round badge filled with a radial gradient, centred on a transparent square."""
    radius = diameter / 2
    centre = size / 2
    gradient = Image.new("RGBA", (size, size))
    pixels = gradient.load()
    for y in range(size):
        for x in range(size):
            t = min(1.0, ((x - centre) ** 2 + (y - centre) ** 2) ** 0.5 / radius)
            t = t * t  # stay light for longer around Kitty's face
            pixels[x, y] = tuple(round(c + (e - c) * t) for c, e in zip(CENTRE, EDGE)) + (255,)
    mask = Image.new("L", (size, size), 0)
    box = (centre - radius, centre - radius, centre + radius, centre + radius)
    ImageDraw.Draw(mask).ellipse(box, fill=255)
    badge = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    badge.paste(gradient, mask=mask)
    rim_width = max(1, round(diameter * 0.025))
    ImageDraw.Draw(badge).ellipse(box, outline=RIM + (255,), width=rim_width)
    return badge


def kitty_head() -> Image.Image:
    """Kitty's head cut out of the art with soft edges, cropped to fit."""
    art = Image.open(ART).convert("RGBA")
    mask = Image.new("L", art.size, 0)
    draw = ImageDraw.Draw(mask)
    draw.polygon(HEAD_TOP, fill=255)
    draw.ellipse(CHIN, fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(HEAD_SOFTNESS))
    alpha = Image.composite(art.getchannel("A"), Image.new("L", art.size, 0), mask)
    art.putalpha(alpha)
    return art.crop(alpha.point(lambda a: 255 if a > 8 else 0).getbbox())


def place_head(canvas: Image.Image, head: Image.Image, size: float, drop: float) -> None:
    """Scale Kitty's head to `size` px across its longer side and centre it, `drop` px low."""
    scale = size / max(head.size)
    scaled = head.resize((round(head.width * scale), round(head.height * scale)), Image.LANCZOS)
    x = (canvas.width - scaled.width) // 2
    y = round((canvas.height - scaled.height) / 2 + drop)
    canvas.alpha_composite(scaled, (x, y))


def legacy_icon(head: Image.Image, px: int) -> Image.Image:
    big = px * SUPERSAMPLE
    icon = radial_badge(big, big * 44 / 48)  # 44 dp circle in the 48 dp square, as Android's own
    place_head(icon, head, big * 0.70, big * 0.02)
    return icon.resize((px, px), Image.LANCZOS)


def adaptive_foreground(head: Image.Image, px: int) -> Image.Image:
    big = px * SUPERSAMPLE
    layer = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    # Launchers mask the 108 dp layer down to a 66 dp safe zone (circle, squircle...), so Kitty
    # stays within that.
    place_head(layer, head, big * 62 / 108, big * 0.01)
    return layer.resize((px, px), Image.LANCZOS)


def main() -> None:
    head = kitty_head()
    for density, scale in DENSITIES.items():
        folder = RES / f"mipmap-{density}"
        folder.mkdir(parents=True, exist_ok=True)
        icon = legacy_icon(head, round(LEGACY_DP * scale))
        icon.save(folder / "ic_launcher.png", optimize=True)
        icon.save(folder / "ic_launcher_round.png", optimize=True)
        foreground = adaptive_foreground(head, round(ADAPTIVE_DP * scale))
        foreground.save(folder / "ic_launcher_foreground.png", optimize=True)
        print(f"{folder.relative_to(HERE)}: {icon.width}px icons, {foreground.width}px foreground")


if __name__ == "__main__":
    main()
