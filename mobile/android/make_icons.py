#!/usr/bin/env python3
"""Generate the app's launcher icons from Kitty's art: python3 make_icons.py

For every density this writes the adaptive icon's layers (ic_launcher_foreground.png, Kitty's head;
ic_launcher_background.png, a warm glow), which mipmap-anydpi/ic_launcher*.xml put together and
every Android 8+ launcher uses, and the plain round badge (ic_launcher.png, ic_launcher_round.png)
for anything that can't show adaptive icons. The PNGs are committed; rerun this only when the art
changes. Needs Pillow.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

HERE = Path(__file__).resolve().parent
ART = HERE.parent.parent / "assets" / "kitty.png"
RES = HERE / "app" / "src" / "main" / "res"

# Kitty's head in kitty.png (978x723, Kitty leaping to the right): everything right of her tail
# down to the cheeks, then an ellipse round her chin fluff that leaves out her body and paws.
# The mask is feathered so the cut through her shoulders fades out instead of showing an edge.
HEAD_TOP = [(385, 0), (978, 0), (978, 440), (560, 440), (470, 425), (398, 380)]
CHIN = (480, 330, 950, 535)
HEAD_FEATHER = 12

# Launcher icon sizes in px per density: legacy icons are 48 dp, adaptive layers 108 dp.
DENSITIES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0, "xxhdpi": 3.0, "xxxhdpi": 4.0}
LEGACY_DP = 48
ADAPTIVE_DP = 108
# The legacy badge is a 44 dp circle, as Android's own round icons are; Kitty stays inside it.
BADGE_DP = 44
LEGACY_HEAD_RADIUS_DP = 21
# Launchers crop the adaptive layers to at most a 72 dp shape, and never cut into the middle
# 66 dp circle, so Kitty stays inside that and the glow reaches its edge colour at 72 dp.
ADAPTIVE_HEAD_RADIUS_DP = 33
ADAPTIVE_GLOW_RADIUS_DP = 36

# Warm cream in the middle to marigold at the edge, with a darker rim round the legacy badge.
CENTRE = (255, 240, 196)
EDGE = (255, 190, 92)
RIM = (226, 132, 52)

# Drawn this many times larger, then scaled down, for smooth edges.
SUPERSAMPLE = 4


def glow(size: int, radius: float) -> Image.Image:
    """A square filled with a radial gradient from CENTRE to EDGE at `radius`, and EDGE beyond."""
    centre = size / 2
    gradient = Image.new("RGBA", (size, size))
    pixels = gradient.load()
    for y in range(size):
        for x in range(size):
            t = min(1.0, ((x - centre) ** 2 + (y - centre) ** 2) ** 0.5 / radius)
            t = t * t  # stay light for longer around Kitty's face
            pixels[x, y] = tuple(round(c + (e - c) * t) for c, e in zip(CENTRE, EDGE)) + (255,)
    return gradient


def badge(size: int, diameter: float) -> Image.Image:
    """A round, rimmed badge of the glow, centred on a transparent square."""
    radius = diameter / 2
    centre = size / 2
    box = (centre - radius, centre - radius, centre + radius, centre + radius)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse(box, fill=255)
    result = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    result.paste(glow(size, radius), mask=mask)
    rim_width = max(1, round(diameter * 0.025))
    ImageDraw.Draw(result).ellipse(box, outline=RIM + (255,), width=rim_width)
    return result


def kitty_head() -> Image.Image:
    """Kitty's head cut out of the art with soft edges, cropped to fit."""
    art = Image.open(ART).convert("RGBA")
    mask = Image.new("L", art.size, 0)
    draw = ImageDraw.Draw(mask)
    draw.polygon(HEAD_TOP, fill=255)
    draw.ellipse(CHIN, fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(HEAD_FEATHER))
    alpha = Image.composite(art.getchannel("A"), Image.new("L", art.size, 0), mask)
    art.putalpha(alpha)
    return art.crop(alpha.getbbox())


def reach(head: Image.Image) -> float:
    """How far the head's solid pixels (her ear tips, mostly) reach from its centre, in px."""
    width, height = head.size
    cx, cy = width / 2, height / 2
    farthest = 0.0
    for i, a in enumerate(head.getchannel("A").tobytes()):
        if a > 128:
            y, x = divmod(i, width)
            farthest = max(farthest, (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2)
    return farthest**0.5


def place_head(canvas: Image.Image, head: Image.Image, radius: float) -> None:
    """Centre the head on the canvas, scaled so it stays within `radius` px of the middle."""
    scale = radius / reach(head)
    scaled = head.resize((round(head.width * scale), round(head.height * scale)), Image.LANCZOS)
    canvas.alpha_composite(scaled, ((canvas.width - scaled.width) // 2, (canvas.height - scaled.height) // 2))


def legacy_icon(head: Image.Image, px: int) -> Image.Image:
    big = px * SUPERSAMPLE
    dp = big / LEGACY_DP
    icon = badge(big, BADGE_DP * dp)
    place_head(icon, head, LEGACY_HEAD_RADIUS_DP * dp)
    return icon.resize((px, px), Image.LANCZOS)


def adaptive_foreground(head: Image.Image, px: int) -> Image.Image:
    big = px * SUPERSAMPLE
    layer = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    place_head(layer, head, ADAPTIVE_HEAD_RADIUS_DP * big / ADAPTIVE_DP)
    return layer.resize((px, px), Image.LANCZOS)


def adaptive_background(px: int) -> Image.Image:
    # Smooth enough drawn at size; launchers scale and mask it themselves.
    return glow(px, ADAPTIVE_GLOW_RADIUS_DP * px / ADAPTIVE_DP).convert("RGB")


def main() -> None:
    head = kitty_head()
    for density, scale in DENSITIES.items():
        folder = RES / f"mipmap-{density}"
        folder.mkdir(parents=True, exist_ok=True)
        icon = legacy_icon(head, round(LEGACY_DP * scale))
        icon.save(folder / "ic_launcher.png", optimize=True)
        icon.save(folder / "ic_launcher_round.png", optimize=True)
        layer_px = round(ADAPTIVE_DP * scale)
        adaptive_foreground(head, layer_px).save(folder / "ic_launcher_foreground.png", optimize=True)
        adaptive_background(layer_px).save(folder / "ic_launcher_background.png", optimize=True)
        print(f"{folder.relative_to(HERE)}: {icon.width} px icons, {layer_px} px adaptive layers")


if __name__ == "__main__":
    main()
