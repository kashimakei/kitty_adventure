"""
Cut animation frames out of the painted sprite sheets in assets/source/.

The sheets are loose layouts on a flat grey background, so instead of a grid this:
  1. keys out the grey background,
  2. finds each separate sprite (connected blob of pixels), dropping the text labels and
     little motion marks,
  3. assigns blobs to animations by which labelled region of the sheet they sit in,
  4. scales every frame of a character by the same factor, so it's the same size in all poses,
  5. writes <out>/<animation>_<n>.png plus <out>/anchors.json, which records the point in each
     frame the game pins to the character's hitbox (feet, gripping paw, or head), so poses
     line up from frame to frame.

Run from the repo root:  python tools/slice_sprites.py [character ...]
       numbered previews of every blob, for editing regions:  python tools/slice_sprites.py --index <dir>
"""
import json
import os
import sys

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
import pygame

# The grey around the sprites is keyed generously, but only where it connects to the sheet's edge;
# Kitty's white fur has lavender-grey shading that a generous key would punch holes in.
OUTSIDE_TOLERANCE = 34
# Grey trapped between legs or inside a tail curl is flat, so a tight key finds it anywhere;
# patches smaller than ENCLOSED_MIN_AREA are highlights in the fur, not gaps
ENCLOSED_TOLERANCE = 10
ENCLOSED_MIN_AREA = 150
EDGE_TRIM = 2               # px shaved off each sprite to drop the grey-blended fringe
MIN_SPRITE_AREA = 6000      # px; anything smaller is a label letter or a motion mark

# Right-facing poses anchor this many game px behind the nose, roughly mid-body, so the head
# stays steady while the tail swings. Front-facing poses anchor on their centre.
NOSE_TO_CENTER = 40
GRIP_BAND = 6  # px below the topmost pixel averaged to find a gripping paw
HEAD_WHITE = 215  # an eagle's head is the near-white feathers in the front half of the frame

# Per character: its sheets, background grey, which frame sets the scale and how tall it should
# be in game pixels, and its animations. Sheets painted at different sizes can set
# "match_sheet_areas" to be scaled so the character's average silhouette area matches the
# reference sheet's (silhouette area hardly changes with pose). Each animation is
#   name -> (sheet, (left, top, right, bottom), anchor mode)
# and collects every blob whose centre falls inside the region, left to right.
CHARACTERS = {
    "kitty": {
        "out": "assets/kitty",
        "background": (186, 186, 186),
        "sheets": {
            "core": "assets/source/kitty_sheet_core.jpg",
            "transitions": "assets/source/kitty_sheet_transitions.jpg",
            "hang_climb": "assets/source/kitty_sheet_hang_climb.jpg",
        },
        "reference": ("walk", 0),
        "target_height": 96,  # the size everything else is judged against
        "animations": {
            # Core sheet
            "walk":      ("core", (0, 40, 1500, 340), "nose"),
            "jump_prep": ("core", (0, 380, 830, 680), "nose"),
            "mid_jump":  ("core", (860, 340, 2000, 720), "nose"),
            "dangle":    ("core", (100, 700, 720, 1150), "center"),
            "clean":     ("core", (850, 780, 2000, 1110), "nose"),
            "run":       ("core", (0, 1170, 1300, 1460), "nose"),
            "sleep":     ("core", (1320, 1160, 2000, 1470), "nose"),
            # Transitions sheet (the overlapping jump-launch pair and stick-dangling poses are skipped)
            "run_start": ("transitions", (1480, 60, 2000, 330), "nose"),
            "land":      ("transitions", (980, 400, 2000, 690), "nose"),
            "sit":       ("transitions", (140, 1170, 470, 1480), "nose"),
            "wave":      ("transitions", (580, 1170, 900, 1480), "nose"),
            "groom":     ("transitions", (950, 1190, 1660, 1480), "nose"),
            "sit_alert": ("transitions", (1640, 1170, 2000, 1480), "nose"),
            # Hang & climb sheet
            "hang":      ("hang_climb", (0, 50, 1330, 580), "grip"),
            "let_go":    ("hang_climb", (1330, 50, 2000, 580), "center"),
            "climb":     ("hang_climb", (0, 600, 2000, 1116), "nose"),
        },
    },
    "dog": {
        "out": "assets/dog",
        "background": (200, 200, 200),
        "sheets": {
            "behaviours": "assets/source/dog_sheet_behaviours.jpg",
            "jump": "assets/source/dog_sheet_jump.jpg",
            "run": "assets/source/dog_sheet_run.jpg",
        },
        "reference": ("sniff", 0),
        "target_height": 96,  # standing dog about the same size as Kitty walking
        "match_sheet_areas": True,
        "nose_to_center": 42,
        "animations": {
            "sniff":   ("behaviours", (0, 80, 2400, 480), "nose"),
            "menace":  ("behaviours", (0, 500, 2400, 900), "nose"),
            "bark":    ("behaviours", (0, 920, 2400, 1330), "nose"),
            "scratch": ("behaviours", (0, 1380, 2400, 1792), "nose"),
            "prep":    ("jump", (0, 60, 2752, 500), "nose"),
            "air":     ("jump", (0, 500, 2752, 1000), "nose"),
            "land":    ("jump", (0, 1050, 2752, 1536), "nose"),
            # The gallop runs across two rows; the game strings run_a and run_b together
            "run_a":   ("run", (0, 150, 2752, 750), "nose"),
            "run_b":   ("run", (0, 800, 2752, 1450), "nose"),
        },
    },
    "eagle": {
        "out": "assets/eagle",
        "background": (201, 201, 201),
        "sheets": {"eagle": "assets/source/eagle_sheet.jpg"},
        "reference": ("flap", 0),
        "target_height": 150,  # wings up; eagles are the biggest thing in the sky
        "animations": {
            "flap":  ("eagle", (0, 60, 2752, 540), "head"),
            "dive":  ("eagle", (0, 560, 2752, 1030), "head"),
            "talon": ("eagle", (0, 1060, 2752, 1536), "head"),
        },
    },
}


def load_sheet(path):
    return pygame.image.load(path).convert_alpha()


def sprite_masks(sheet, background_colour):
    """Foreground mask of the sheet with the grey background keyed out and edges trimmed."""
    loose = pygame.mask.from_threshold(sheet, background_colour, (OUTSIDE_TOLERANCE,) * 3 + (255,))
    background = loose.connected_component((2, 2))
    flat = pygame.mask.from_threshold(sheet, background_colour, (ENCLOSED_TOLERANCE,) * 3 + (255,))
    for gap in flat.connected_components(ENCLOSED_MIN_AREA):
        background.draw(gap, (0, 0))
    # Grow the background a little (equivalently, erode the sprites) to remove grey fringes
    kernel = pygame.mask.Mask((EDGE_TRIM * 2 + 1, EDGE_TRIM * 2 + 1), fill=True)
    grown = background.convolve(kernel, None, (-EDGE_TRIM, -EDGE_TRIM))
    grown.invert()
    return grown


def find_blobs(sheet, background_colour):
    """(rect, mask) for every sprite-sized blob, top to bottom then left to right."""
    foreground = sprite_masks(sheet, background_colour)
    blobs = []
    for component in foreground.connected_components(MIN_SPRITE_AREA):
        rect = component.get_bounding_rects()[0]
        if rect.width > sheet.get_width() // 2:
            continue  # the sheet's darker border, not a sprite
        blobs.append((rect, component))
    blobs.sort(key=lambda b: (b[0].centery // 150, b[0].centerx))
    return blobs


def index_image(name, sheet, blobs, path):
    """Save the sheet with every blob boxed and numbered, for editing regions."""
    out = sheet.copy()
    font = pygame.font.SysFont(None, 48)
    for i, (rect, mask) in enumerate(blobs):
        pygame.draw.rect(out, (255, 0, 0), rect, 3)
        out.blit(font.render(str(i), True, (0, 0, 255)), rect.topleft)
        print(f"{name} #{i}: {tuple(rect)}  area={mask.count()}")
    pygame.image.save(out, path)


def cut_frame(sheet, rect, mask):
    """The blob's pixels with everything outside its mask transparent."""
    frame = sheet.subsurface(rect).copy()
    alpha = mask.to_surface(setcolor=(255, 255, 255, 255), unsetcolor=(0, 0, 0, 0))
    frame.blit(alpha, (0, 0), rect, special_flags=pygame.BLEND_RGBA_MULT)
    return frame


def anchor_for(frame, mode, nose_to_center=NOSE_TO_CENTER):
    """The point the game pins to the hitbox: under the body at the lowest pixel ("nose",
    "center"), the topmost gripping paw ("grip"), or the middle of a white head ("head")."""
    w, h = frame.get_size()
    if mode == "center":
        return (w // 2, h - 1)
    if mode == "grip":
        solid = pygame.mask.from_surface(frame, 127)
        top = next(y for y in range(h) if any(solid.get_at((x, y)) for x in range(w)))
        xs = [x for y in range(top, min(h, top + GRIP_BAND)) for x in range(w) if solid.get_at((x, y))]
        return (round(sum(xs) / len(xs)), top)
    if mode == "head":
        points = [(x, y) for y in range(h) for x in range(w * 2 // 5, w)
                  if all(c >= HEAD_WHITE for c in frame.get_at((x, y))[:3]) and frame.get_at((x, y))[3] > 200]
        return (round(sum(p[0] for p in points) / len(points)), round(sum(p[1] for p in points) / len(points)))
    return (max(0, w - 1 - nose_to_center), h - 1)


def slice_character(name, spec):
    sheets = {sheet: load_sheet(path) for sheet, path in spec["sheets"].items()}
    blobs = {sheet: find_blobs(surface, spec["background"]) for sheet, surface in sheets.items()}

    # Collect each animation's blobs from its region
    picked = {}
    for anim, (sheet_name, (left, top, right, bottom), mode) in spec["animations"].items():
        region = [b for b in blobs[sheet_name]
                  if left <= b[0].centerx < right and top <= b[0].centery < bottom]
        region.sort(key=lambda b: b[0].centerx)
        if not region:
            sys.exit(f"No sprites found for {name} {anim!r}; check its region with --index")
        picked[anim] = (sheet_name, region, mode)

    ref_anim, ref_index = spec["reference"]
    ref_sheet = picked[ref_anim][0]
    scale = spec["target_height"] / picked[ref_anim][1][ref_index][0].height
    sheet_scales = {sheet: scale for sheet in sheets}
    if spec.get("match_sheet_areas"):
        def mean_area(sheet):
            areas = [mask.count() for sheet_name, region, _ in picked.values() if sheet_name == sheet
                     for _, mask in region]
            return sum(areas) / len(areas)
        for sheet in sheets:
            sheet_scales[sheet] = scale * (mean_area(ref_sheet) / mean_area(sheet)) ** 0.5

    out_dir = spec["out"]
    os.makedirs(out_dir, exist_ok=True)
    for old in os.listdir(out_dir):
        if old.endswith(".png"):
            os.remove(os.path.join(out_dir, old))
    manifest = {"scale": scale, "animations": {}, "anchors": {}}
    for anim, (sheet_name, region, mode) in picked.items():
        names = []
        for i, (rect, mask) in enumerate(region):
            frame = cut_frame(sheets[sheet_name], rect, mask)
            frame_scale = sheet_scales[sheet_name]
            size = (max(1, round(rect.width * frame_scale)), max(1, round(rect.height * frame_scale)))
            frame = pygame.transform.smoothscale(frame, size)
            frame_name = f"{anim}_{i}.png"
            pygame.image.save(frame, os.path.join(out_dir, frame_name))
            manifest["anchors"][frame_name] = anchor_for(frame, mode, spec.get("nose_to_center", NOSE_TO_CENTER))
            names.append(frame_name)
        manifest["animations"][anim] = names
        print(f"{name} {anim:10} {len(names)} frame(s)")
    with open(os.path.join(out_dir, "anchors.json"), "w") as f:
        json.dump(manifest, f, indent=2)


def main():
    pygame.init()
    pygame.display.set_mode((1, 1))
    args = sys.argv[1:]
    if "--index" in args:
        i = args.index("--index")
        out_dir = args[i + 1] if len(args) > i + 1 else "."
        names = [a for a in args if a in CHARACTERS] or list(CHARACTERS)
        for name in names:
            spec = CHARACTERS[name]
            for sheet_name, path in spec["sheets"].items():
                sheet = load_sheet(path)
                index_image(f"{name}/{sheet_name}", sheet, find_blobs(sheet, spec["background"]),
                            os.path.join(out_dir, f"index_{name}_{sheet_name}.png"))
        return
    for name in [a for a in args if a in CHARACTERS] or list(CHARACTERS):
        slice_character(name, CHARACTERS[name])


if __name__ == "__main__":
    main()
