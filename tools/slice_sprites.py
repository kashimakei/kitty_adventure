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
     line up from frame to frame,
  6. for enemies, writes <out>/<animation>_<n>_hit.png: a mask of the parts that can hurt Kitty
     (an eagle's beak and talons, a dog's paws and mouth).

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
# A perched critter pins its feet under its head and chest, which stay put while wings, tails and
# beaks swing out: what's left after wearing away this fraction of the frame's height from every
# edge, down this fraction of the way from its top
BODY_WEAR = 0.09
BODY_UPPER = 0.55

# Damage masks. An eagle's beak and feet are the only golden-yellow parts; the mask grows a few px
# to take in the dark claw tips. A dog's paws are cream like its chest, so they're found by
# position instead: the lowest strip of each pose. Its mouth is a disc around the nose: the
# frontmost solid dark blob (the thin dark outline doesn't survive a 1px erosion; the nose does).
CLAW_REACH = 3        # px the yellow mask grows to cover claws
PAW_BAND = 0.14       # fraction of the frame's height, from the bottom, that counts as paws
MOUTH_RADIUS = 14     # px
NOSE_DARK = 110       # max channel value for nose-black pixels

# Per character: its sheets, background grey, which frame sets the scale and how tall it should
# be in game pixels, and its animations. Sheets painted at different sizes can set
# "match_sheet_areas" to be scaled so the character's average silhouette area matches the
# reference sheet's (silhouette area hardly changes with pose). A pose painted lighter or darker
# than its siblings can be recoloured to match one of them with "tone_match". Each animation is
#   name -> (sheet, (left, top, right, bottom), anchor mode[, scale multiplier])
# and collects every blob whose centre falls inside the region, left to right.
CHARACTERS = {
    "kitty": {
        "out": "assets/kitty",
        "background": (186, 186, 186),
        "sheets": {
            "core": "assets/source/kitty_sheet_core.jpg",
            "transitions": "assets/source/kitty_sheet_transitions.jpg",
            "hang_climb": "assets/source/kitty_sheet_hang_climb.jpg",
            # Gemini, with the core sheet as reference; its bottom row just echoes the reference
            "pounce": "assets/source/kitty_sheet_pounce.jpg",
            # Gemini, from the owner: the charging crouch with the tail swishing, and a crouch
            # sinking from a stalk to belly-low
            "swish": "assets/source/kitty_sheet_swish.jpg",
            "crouch": "assets/source/kitty_sheet_crouch.jpg",
        },
        "reference": ("walk", 0),
        "target_height": 88,  # the size everything else is judged against
        "nose_to_center": 37,
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
            # Pounce sheet: the spring out of the crouch
            "pounce":    ("pounce", (0, 840, 2400, 1400), "nose", 0.6),
            # Swish sheet: crouched to charge, the tail swishing (two rows of three). Kitty is
            # painted bigger on it than on the pounce and crouch sheets
            "swish_a":   ("swish", (0, 0, 2400, 860), "nose", 0.52),
            "swish_b":   ("swish", (0, 860, 2400, 1792), "nose", 0.52),
            # Crouch sheet: the stalk she settles into as a charge starts (top left). The rest of
            # the sheet sinks on to belly-low with a tail or two, for later
            "stalk":     ("crouch", (0, 0, 800, 860), "nose", 0.6),
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
        "target_height": 104,  # a standing dog is a little bigger than Kitty
        "match_sheet_areas": True,
        "nose_to_center": 45,
        "damage": "paws_mouth",
        # Sitting poses rest the curled tail on the ground; only the front of the paw strip counts
        "paw_strip_from": {"scratch": 0.35},
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
        "damage": "yellow",
        "animations": {
            "flap":  ("eagle", (0, 60, 2752, 540), "head"),
            "dive":  ("eagle", (0, 560, 2752, 1030), "head"),
            "talon": ("eagle", (0, 1060, 2752, 1536), "head"),
        },
    },
    # Enemies, friends, pickups and foliage generated in Gemini with the kitty and dog sheets as
    # style references
    "squirrel": {
        "out": "assets/squirrel",
        "background": (187, 187, 187),
        "sheets": {"squirrel": "assets/source/squirrel_sheet.jpg"},
        "reference": ("run", 0),
        "target_height": 58,
        "nose_to_center": 26,
        "damage": "paws_mouth",
        "nose_rows": (0.3, 0.92),  # skip the dark ear tufts above the snout
        "animations": {
            "run":   ("squirrel", (0, 40, 2400, 480), "nose"),
            # stops short of the little acorns drawn after the last throw pose
            "throw": ("squirrel", (0, 480, 2050, 900), "nose"),
            "idle":  ("squirrel", (0, 900, 2400, 1360), "nose"),
            "hurt":  ("squirrel", (0, 1360, 2400, 1792), "nose"),
        },
    },
    "snake": {
        "out": "assets/snake",
        "background": (187, 187, 187),
        "sheets": {"snake": "assets/source/snake_sheet.jpg"},
        "reference": ("slither", 0),
        "target_height": 50,
        "damage": "mouth",
        "animations": {
            "slither": ("snake", (0, 100, 2400, 480), "center"),
            "coiled":  ("snake", (0, 480, 2400, 920), "center"),
            "strike":  ("snake", (0, 920, 2400, 1380), "center"),
            "hurt":    ("snake", (0, 1380, 2400, 1792), "center"),
        },
    },
    "crow": {
        "out": "assets/crow",
        "background": (187, 187, 187),
        "sheets": {"crow": "assets/source/crow_sheet.jpg"},
        "reference": ("fly", 0),
        "target_height": 78,
        "damage": "yellow",
        "animations": {
            "fly":     ("crow", (0, 60, 2400, 500), "centroid"),
            "dive":    ("crow", (0, 500, 2400, 940), "centroid"),
            "perched": ("crow", (0, 940, 2400, 1370), "center"),
            "hurt":    ("crow", (0, 1370, 2400, 1792), "centroid"),
        },
    },
    "friends": {
        "out": "assets/friends",
        "background": (185, 185, 185),
        "sheets": {"friends": "assets/source/friends_sheet.jpg",
                   "storm": "assets/source/friends_storm_sheet.jpg"},
        "reference": ("pip_help", 0),
        "target_height": 76,
        # The storm sheet (the friends frightened, tumbling on the wind) is drawn at its own scale
        "match_sheet_areas": True,
        # Pip's fur is nearly the background grey: key gently and leave enclosed grey alone
        "outside_tolerance": 20,
        "enclosed_tolerance": None,
        # Pip's last cheer pose is painted paler than the rest; the one before is the same pose
        "tone_match": {"pip_cheer_3": "pip_cheer_2"},
        # The hedgehog row also holds a faded copy of the reference kitty around x 550-1000
        "animations": {
            "pip_help":    ("friends", (0, 0, 560, 450), "center"),
            "pip_cheer":   ("friends", (560, 0, 2400, 450), "center"),
            "hazel_help":  ("friends", (0, 450, 560, 780), "center"),
            "hazel_cheer": ("friends", (1000, 450, 2400, 780), "center"),
            "mochi_help":  ("friends", (0, 780, 560, 1100), "center"),
            "mochi_cheer": ("friends", (560, 780, 2400, 1100), "center"),
            "sunny_help":  ("friends", (0, 1100, 560, 1400), "center"),
            "sunny_cheer": ("friends", (560, 1100, 2400, 1400), "center"),
            "luna_help":   ("friends", (0, 1400, 560, 1792), "center"),
            "luna_cheer":  ("friends", (560, 1400, 2400, 1792), "center"),
            "pip_storm":   ("storm", (0, 0, 1792, 510), "center"),
            "hazel_storm": ("storm", (0, 510, 1792, 1000), "center"),
            "mochi_storm": ("storm", (0, 1000, 1792, 1465), "center"),
            "sunny_storm": ("storm", (0, 1465, 1792, 1950), "center"),
            "luna_storm":  ("storm", (0, 1950, 1792, 2400), "center"),
        },
    },
    "clues": {
        "out": "assets/clues",
        # Mint rather than grey, so the grey fur keys cleanly
        "background": (168, 215, 199),
        "sheets": {"clues": "assets/source/clues_sheet.jpg"},
        "reference": ("carrot", 0),
        "target_height": 100,
        # One object per column, two rows; each region stops above the object's label
        "animations": {
            "carrot":  ("clues", (0, 60, 480, 570), "middle"),
            "fur":     ("clues", (480, 60, 960, 570), "middle"),
            "quill":   ("clues", (960, 60, 1440, 570), "middle"),
            "berry":   ("clues", (1440, 60, 1920, 570), "middle"),
            "seeds":   ("clues", (1920, 60, 2400, 570), "middle"),
            "pouch":   ("clues", (0, 850, 480, 1310), "middle"),
            "feather": ("clues", (480, 850, 960, 1310), "middle"),
            "crust":   ("clues", (960, 850, 1440, 1310), "middle"),
            "bell":    ("clues", (1440, 850, 1920, 1310), "middle"),
            "ribbon":  ("clues", (1920, 850, 2400, 1310), "middle"),
        },
    },
    "helpers": {
        "out": "assets/helpers",
        # Mint, like the clue sheet
        "background": (168, 215, 197),
        "sheets": {"helpers": "assets/source/helpers_sheet.jpg"},
        "reference": ("robin_idle", 0),
        # Drawn smaller than this on the clue card, so they stay sharp on a phone
        "target_height": 160,
        # One helper per row: perched and listening, then pointing the way and excited, talking
        "animations": {
            "robin_idle":      ("helpers", (0, 0, 597, 504), "body"),
            "robin_talk":      ("helpers", (597, 0, 1792, 504), "body"),
            "owl_idle":        ("helpers", (0, 504, 597, 987), "body"),
            "owl_talk":        ("helpers", (597, 504, 1792, 987), "body"),
            "pika_idle":       ("helpers", (0, 987, 597, 1473), "body"),
            "pika_talk":       ("helpers", (597, 987, 1792, 1473), "body"),
            "kingfisher_idle": ("helpers", (0, 1473, 597, 1938), "body"),
            "kingfisher_talk": ("helpers", (597, 1473, 1792, 1938), "body"),
            "bat_idle":        ("helpers", (0, 1938, 597, 2400), "body"),
            "bat_talk":        ("helpers", (597, 1938, 1792, 2400), "body"),
        },
    },
    "items": {
        "out": "assets/items",
        "background": (185, 185, 185),
        "sheets": {"items": "assets/source/items_sheet.jpg"},
        "reference": ("fish", 0),
        "target_height": 30,
        # The silver fish are nearly the background grey: key gently and leave enclosed grey alone
        "outside_tolerance": 22,
        "enclosed_tolerance": None,
        # The fish row also holds a faded copy of the reference kitty, and the sparkle burst
        # frames are too faint to key, so only the big star is used (animated in code)
        "animations": {
            "fish":  ("items", (0, 100, 1250, 420), "middle"),
            "gold":  ("items", (0, 500, 1250, 820), "middle"),
            "heart": ("items", (0, 950, 1250, 1300), "middle"),
            "star":  ("items", (1250, 950, 1750, 1300), "middle"),
            "nest":  ("items", (0, 1380, 1250, 1792), "center", 1.4),
            "acorn": ("items", (1250, 1380, 2400, 1792), "middle", 0.45),
        },
    },
    "foliage": {
        "out": "assets/foliage",
        "background": (184, 184, 184),
        "sheets": {"foliage": "assets/source/foliage_sheet.jpg"},
        "reference": ("green", 5),
        "target_height": 56,
        "animations": {
            "green":   ("foliage", (0, 80, 2400, 420), "middle"),
            "autumn":  ("foliage", (0, 500, 2400, 850), "middle"),
            "twig":    ("foliage", (0, 900, 2400, 1300), "base"),
            "falling": ("foliage", (0, 1400, 2400, 1792), "middle", 0.45),
        },
    },
}


def load_sheet(path):
    return pygame.image.load(path).convert_alpha()


def sprite_masks(sheet, spec):
    """Foreground mask of the sheet with the grey background keyed out and edges trimmed.
    Characters with grey fur can lower "outside_tolerance" and set "enclosed_tolerance" to None
    so the key doesn't eat into them."""
    background_colour = spec["background"]
    outside = spec.get("outside_tolerance", OUTSIDE_TOLERANCE)
    enclosed = spec.get("enclosed_tolerance", ENCLOSED_TOLERANCE)
    loose = pygame.mask.from_threshold(sheet, background_colour, (outside,) * 3 + (255,))
    background = loose.connected_component((2, 2))
    if enclosed is not None:
        flat = pygame.mask.from_threshold(sheet, background_colour, (enclosed,) * 3 + (255,))
        for gap in flat.connected_components(ENCLOSED_MIN_AREA):
            background.draw(gap, (0, 0))
    # Grow the background a little (equivalently, erode the sprites) to remove grey fringes
    kernel = pygame.mask.Mask((EDGE_TRIM * 2 + 1, EDGE_TRIM * 2 + 1), fill=True)
    grown = background.convolve(kernel, None, (-EDGE_TRIM, -EDGE_TRIM))
    grown.invert()
    return grown


def find_blobs(sheet, spec):
    """(rect, mask) for every sprite-sized blob, top to bottom then left to right."""
    foreground = sprite_masks(sheet, spec)
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
    "center"), the topmost gripping paw ("grip"), the middle of a white head ("head"), the
    frame's middle ("middle"), its centre of mass ("centroid"), its bottom-left ("base"), or the
    lowest pixel under the head and chest ("body")."""
    w, h = frame.get_size()
    if mode == "center":
        return (w // 2, h - 1)
    if mode == "middle":
        return (w // 2, h // 2)
    if mode == "base":
        return (0, h - 1)
    if mode == "centroid":
        cx, cy = pygame.mask.from_surface(frame, 127).centroid()
        return (cx, cy)
    if mode == "grip":
        solid = pygame.mask.from_surface(frame, 127)
        top = next(y for y in range(h) if any(solid.get_at((x, y)) for x in range(w)))
        xs = [x for y in range(top, min(h, top + GRIP_BAND)) for x in range(w) if solid.get_at((x, y))]
        return (round(sum(xs) / len(xs)), top)
    if mode == "body":
        return (round(upper_body_x(pygame.mask.from_surface(frame, 127), w, h)), h - 1)
    if mode == "head":
        points = [(x, y) for y in range(h) for x in range(w * 2 // 5, w)
                  if all(c >= HEAD_WHITE for c in frame.get_at((x, y))[:3]) and frame.get_at((x, y))[3] > 200]
        return (round(sum(p[0] for p in points) / len(points)), round(sum(p[1] for p in points) / len(points)))
    return (max(0, w - 1 - nose_to_center), h - 1)


def upper_body_x(solid, w, h):
    """The middle of a critter's head and chest, across: the solid pixels left once the thin
    parts (wings, tails, beaks) are worn away, in the upper part of what's left."""
    r = max(2, round(h * BODY_WEAR))
    # A summed-area table of the solid pixels; a pixel survives if the square around it is solid
    table = [[0] * (w + 1) for _ in range(h + 1)]
    for y in range(h):
        run = 0
        for x in range(w):
            run += solid.get_at((x, y))
            table[y + 1][x + 1] = table[y][x + 1] + run
    side = 2 * r + 1
    kept = [(x, y) for y in range(r, h - r) for x in range(r, w - r)
            if table[y + r + 1][x + r + 1] - table[y - r][x + r + 1] - table[y + r + 1][x - r] + table[y - r][x - r] == side * side]
    if not kept:
        return w / 2
    top = min(y for _, y in kept)
    cut = top + (max(y for _, y in kept) - top) * BODY_UPPER
    upper = [x for x, y in kept if y <= cut]
    return sum(upper) / len(upper)


def is_golden(colour):
    r, g, b = colour[:3]
    return r >= 200 and 120 <= g <= 225 and b <= 110 and r - b >= 110


def find_nose(frame, solid, rows=(0.0, 0.92)):
    """Centre of the frontmost thick dark blob of a right-facing pose, searching the band of rows
    given as fractions of the height (above the paws; below a squirrel's dark ear tufts)."""
    w, h = frame.get_size()
    def dark(x, y):
        return solid.get_at((x, y)) and max(frame.get_at((x, y))[:3]) <= NOSE_DARK
    for x in range(w - 2, 0, -1):
        ys = [y for y in range(max(1, int(h * rows[0])), int(h * rows[1]))
              if all(dark(x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1))]
        if ys:
            return x, sum(ys) / len(ys)
    return w - 1, h * 0.3


def damage_mask(frame, rule, paw_strip_from=0.0, nose_rows=(0.0, 0.92)):
    """White where this frame can hurt Kitty, transparent elsewhere."""
    w, h = frame.get_size()
    solid = pygame.mask.from_surface(frame, 127)
    hit = pygame.mask.Mask((w, h))
    if rule == "yellow":
        for y in range(h):
            for x in range(w):
                if solid.get_at((x, y)) and is_golden(frame.get_at((x, y))):
                    hit.set_at((x, y))
        kernel = pygame.mask.Mask((CLAW_REACH * 2 + 1,) * 2, fill=True)
        hit = solid.overlap_mask(hit.convolve(kernel, None, (-CLAW_REACH, -CLAW_REACH)), (0, 0))
    elif rule in ("paws_mouth", "mouth"):
        if rule == "paws_mouth":
            for y in range(h - round(h * PAW_BAND), h):
                for x in range(int(w * paw_strip_from), w):
                    if solid.get_at((x, y)):
                        hit.set_at((x, y))
        nose_x, nose_y = find_nose(frame, solid, nose_rows)
        cx, cy = nose_x - 4, nose_y + 4  # nose and the jaws just behind and below it
        for y in range(h):
            for x in range(w):
                if (x - cx) ** 2 + (y - cy) ** 2 <= MOUTH_RADIUS ** 2 and solid.get_at((x, y)):
                    hit.set_at((x, y))
    return hit.to_surface(setcolor=(255, 255, 255, 255), unsetcolor=(0, 0, 0, 0))


def match_tones(frame, like):
    """`frame` recoloured so each channel's spread of values over its solid pixels matches
    `like`'s (histogram matching)."""
    def channels(surface):
        w, h = surface.get_size()
        solid = [c for c in (surface.get_at((x, y)) for y in range(h) for x in range(w)) if c[3] >= 128]
        return [[c[i] for c in solid] for i in range(3)]

    def cumulative(values):
        counts = [0] * 256
        for v in values:
            counts[v] += 1
        total, run, shares = len(values), 0, []
        for n in counts:
            run += n
            shares.append(run / total)
        return shares

    tables = []
    for mine, theirs in zip(channels(frame), channels(like)):
        mine, theirs = cumulative(mine), cumulative(theirs)
        table, j = [], 0
        for v in range(256):
            while j < 255 and theirs[j] < mine[v]:
                j += 1
            table.append(j)
        tables.append(table)
    out = frame.copy()
    w, h = out.get_size()
    for y in range(h):
        for x in range(w):
            r, g, b, a = out.get_at((x, y))
            if a:
                out.set_at((x, y), (tables[0][r], tables[1][g], tables[2][b], a))
    return out


def slice_character(name, spec):
    sheets = {sheet: load_sheet(path) for sheet, path in spec["sheets"].items()}
    blobs = {sheet: find_blobs(surface, spec) for sheet, surface in sheets.items()}

    # Collect each animation's blobs from its region
    picked = {}
    for anim, (sheet_name, (left, top, right, bottom), mode, *extra) in spec["animations"].items():
        region = [b for b in blobs[sheet_name]
                  if left <= b[0].centerx < right and top <= b[0].centery < bottom]
        region.sort(key=lambda b: b[0].centerx)
        if not region:
            sys.exit(f"No sprites found for {name} {anim!r}; check its region with --index")
        picked[anim] = (sheet_name, region, mode, extra[0] if extra else 1.0)

    ref_anim, ref_index = spec["reference"]
    ref_sheet = picked[ref_anim][0]
    scale = spec["target_height"] / picked[ref_anim][1][ref_index][0].height
    sheet_scales = {sheet: scale for sheet in sheets}
    if spec.get("match_sheet_areas"):
        def mean_area(sheet):
            areas = [mask.count() for sheet_name, region, _, _ in picked.values() if sheet_name == sheet
                     for _, mask in region]
            return sum(areas) / len(areas)
        for sheet in sheets:
            sheet_scales[sheet] = scale * (mean_area(ref_sheet) / mean_area(sheet)) ** 0.5

    out_dir = spec["out"]
    os.makedirs(out_dir, exist_ok=True)
    for old in os.listdir(out_dir):
        if old.endswith(".png"):
            os.remove(os.path.join(out_dir, old))
    manifest = {"scale": scale, "animations": {}, "anchors": {}, "hits": {}}
    for anim, (sheet_name, region, mode, multiplier) in picked.items():
        names = []
        for i, (rect, mask) in enumerate(region):
            frame = cut_frame(sheets[sheet_name], rect, mask)
            frame_scale = sheet_scales[sheet_name] * multiplier
            size = (max(1, round(rect.width * frame_scale)), max(1, round(rect.height * frame_scale)))
            frame = pygame.transform.smoothscale(frame, size)
            frame_name = f"{anim}_{i}.png"
            pygame.image.save(frame, os.path.join(out_dir, frame_name))
            manifest["anchors"][frame_name] = anchor_for(frame, mode, spec.get("nose_to_center", NOSE_TO_CENTER))
            if spec.get("damage"):
                hit_name = f"{anim}_{i}_hit.png"
                strip_from = spec.get("paw_strip_from", {}).get(anim, 0.0)
                mask = damage_mask(frame, spec["damage"], strip_from, spec.get("nose_rows", (0.0, 0.92)))
                pygame.image.save(mask, os.path.join(out_dir, hit_name))
                manifest["hits"][frame_name] = hit_name
            names.append(frame_name)
        manifest["animations"][anim] = names
        print(f"{name} {anim:10} {len(names)} frame(s)")
    for frame_name, like in spec.get("tone_match", {}).items():
        path = os.path.join(out_dir, f"{frame_name}.png")
        like_frame = pygame.image.load(os.path.join(out_dir, f"{like}.png"))
        pygame.image.save(match_tones(pygame.image.load(path), like_frame), path)
        print(f"{name} {frame_name} recoloured to match {like}")
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
                index_image(f"{name}/{sheet_name}", sheet, find_blobs(sheet, spec),
                            os.path.join(out_dir, f"index_{name}_{sheet_name}.png"))
        return
    for name in [a for a in args if a in CHARACTERS] or list(CHARACTERS):
        slice_character(name, CHARACTERS[name])


if __name__ == "__main__":
    main()
