"""
Turn the painted parallax layers in assets/source/parallax/ into game textures in assets/parallax/.

Gemini painted each layer on flat magenta, so this:
  1. keys the magenta out, recovering the true colour of soft edges (a pixel there is part
     scenery, part magenta) so no pink fringe is left,
  2. splices out the repeated strip some layers came back with (see SPANS),
  3. trims the empty rows above the scenery and halves the size,
  4. cuts the clouds sheet into one PNG per cloud.

Run from the repo root:  python tools/key_layers.py
"""
import os

import numpy as np
import pygame
from PIL import Image

SOURCE = "assets/source/parallax"
OUT = "assets/parallax"

# Rows of each source to keep, top to bottom. The mountains came back with a second copy of
# themselves 384 rows lower whose peaks are cut off by a magenta seam (which smudges up to about
# row 520): keep the top copy's peaks and splice on the bottom copy from the matching row, which
# hides the seam. The hills' second copy is complete, so only it is kept.
SPANS = {
    "far_mountains": [(0, 480), (480 + 384, None)],
    "mid_hills": [(430, None)],
    "near_forest": [(0, None)],
}
NAMES = {"far_mountains": "far", "mid_hills": "mid", "near_forest": "near"}

# Magenta-ness (min(red, blue) - green) at or below SOLID is scenery; at or above CLEAR is
# background; in between is a soft edge
SOLID, CLEAR = 50.0, 140.0
# Scenery is never more magenta than this (the clouds' lavender shading is about 30); anything
# more is pink fringe left by JPEG ringing, pulled back to it
MAX_MAGENTA = 45.0
MAGENTA = np.array([255.0, 0.0, 255.0])
SCALE = 0.5
CLOUD_MIN_AREA = 2000  # px at full size; smaller blobs are JPEG specks


def key(rgb):
    """RGBA float array from an RGB one with the magenta keyed out."""
    rgb = rgb.astype(float)
    magenta = np.minimum(rgb[..., 0], rgb[..., 2]) - rgb[..., 1]
    alpha = np.clip((CLEAR - magenta) / (CLEAR - SOLID), 0.0, 1.0)
    # Un-mix the background from soft edges: seen = alpha * colour + (1 - alpha) * magenta
    a = np.maximum(alpha, 0.05)[..., None]
    colour = np.clip((rgb - (1.0 - a) * MAGENTA) / a, 0.0, 255.0)
    excess = np.maximum(np.minimum(colour[..., 0], colour[..., 2]) - colour[..., 1] - MAX_MAGENTA, 0.0)
    colour[..., 0] -= excess
    colour[..., 2] -= excess
    colour[alpha == 0.0] = 0.0
    return np.dstack([colour, alpha * 255.0])


def to_image(rgba):
    return Image.fromarray(rgba.round().astype(np.uint8), "RGBA")


def shrink(image):
    size = (round(image.width * SCALE), round(image.height * SCALE))
    # Resample premultiplied so transparent pixels don't bleed into the edges
    return image.convert("RGBa").resize(size, Image.LANCZOS).convert("RGBA")


def layer(name):
    rgb = np.asarray(Image.open(os.path.join(SOURCE, f"{name}.jpg")).convert("RGB"))
    rgb = np.concatenate([rgb[top:bottom] for top, bottom in SPANS[name]])
    rgba = key(rgb)
    filled = np.nonzero(rgba[..., 3].max(axis=1) > 8)[0]
    rgba = rgba[max(0, filled[0] - 2):]
    path = os.path.join(OUT, f"{NAMES[name]}.png")
    shrink(to_image(rgba)).save(path)
    print(path)


def clouds():
    rgba = key(np.asarray(Image.open(os.path.join(SOURCE, "clouds.jpg")).convert("RGB")))
    image = to_image(rgba)
    mask = pygame.mask.from_surface(pygame.image.frombytes(image.tobytes(), image.size, "RGBA"), 127)
    found = mask.connected_components(CLOUD_MIN_AREA)
    found.sort(key=lambda m: -m.count())  # biggest first
    # Grow each cloud's mask a little to keep its soft edge, then cut it out on its own, so
    # a neighbour poking into its bounding box doesn't come along
    grow = pygame.mask.Mask((9, 9), fill=True)
    for i, cloud in enumerate(found):
        cloud = cloud.convolve(grow, pygame.mask.Mask(cloud.get_size()), (-4, -4))
        keep = pygame.surfarray.array2d(cloud.to_surface(setcolor=(255, 255, 255, 255), unsetcolor=(0, 0, 0, 0))) != 0
        alone = rgba.copy()
        alone[..., 3] *= keep.T
        r = cloud.get_bounding_rects()[0]
        crop = to_image(alone).crop((r.x, r.y, r.right, r.bottom))
        path = os.path.join(OUT, f"cloud_{i}.png")
        shrink(crop).save(path)
        print(path, crop.size)


def main():
    os.makedirs(OUT, exist_ok=True)
    for old in os.listdir(OUT):
        if old.endswith(".png"):
            os.remove(os.path.join(OUT, old))
    for name in SPANS:
        layer(name)
    clouds()


if __name__ == "__main__":
    main()
