
import random
import math
from constants import *

def get_breeze_strength(time_elapsed, variability=5.0):
    """
    Calculates breeze strength as a slow oscillating sine wave with a quicker gust on top.
    The value smoothly increases and decreases over time; `variability` sets how strong it gets.
    """
    base_strength = 1.5  # Base force of the wind
    cycle_speed = 0.5  # Speed of oscillation 
    gust = 0.3 * math.sin(1.7 * time_elapsed + 1.0)
    
    return base_strength + variability * (math.sin(cycle_speed * time_elapsed) + gust)

def difficulty_profile(level, difficulty):
    """
    Everything that makes a level harder. Branch sizes stay natural at every level; the ratchet is
    wider spacing, bendier wood, stronger wind, and more (and faster) enemies.
    """
    climb = (level - 1) * difficulty / 2.0  # 0 on level 1, 9 on level 10 at normal difficulty
    eagle_wait = max(0.35, 1 - 0.08 * max(0, level - 4))
    return {
        "gap_extra": difficulty * level * 10,
        "stiffness": max(0.5, 0.95 - 0.035 * climb),
        "wind": 3.0 + 0.6 * climb,
        "dog_start_level": 2,
        "max_dogs": max(1, min(4, difficulty - 1 + (level + 1) // 3)),
        "dog_interval_ms": {1: 8000, 2: 5000, 3: 3000}.get(difficulty, 5000) * max(0.4, 1 - 0.07 * (level - 1)),
        "dog_speed": 1 + 0.08 * (level - 1),
        "eagle_start_level": 4,
        "max_eagles": 1 if level < 8 else 2,
        "eagle_interval_ms": (int(5000 * eagle_wait), int(10000 * eagle_wait)),
        "eagle_speed": 1 + 0.06 * max(0, level - 4),
    }

def tree_positions(world_width):
    """x-centres of the trees: one hugging each edge of the world, the rest evenly spaced between."""
    edge = TRUNK_WIDTH // 2 + 5
    count = round((world_width - 2 * edge) / TREE_SPACING) + 1
    spacing = (world_width - 2 * edge) / (count - 1)
    return [round(edge + i * spacing) for i in range(count)]

def branch_slots(tree_count):
    """(tree index, branch direction) pairs: edge trees grow inward, the rest both ways."""
    slots = []
    for i in range(tree_count):
        if i > 0:
            slots.append((i, -1))
        if i < tree_count - 1:
            slots.append((i, 1))
    return slots

def branch_span(tree_xs, slot, length):
    """Horizontal extent (left, right) a branch covers."""
    x = tree_xs[slot[0]]
    return (min(x, x + slot[1] * length), max(x, x + slot[1] * length))

def horizontal_gap(a, b):
    """Distance between two spans, 0 if they overlap."""
    return max(0, max(a[0], b[0]) - min(a[1], b[1]))

def generate_branches(max_branches, world_width, screen_height, level=1, difficulty=1):
    """
    Lays out the level, bottom to top. A guaranteed-climbable path of `max_branches` wanders
    sideways across the world (its last branch is the goal), and filler branches around it offer
    detours and dead ends. Lengths and thicknesses vary naturally; harder levels space branches
    further apart and make the wood bendier (see `difficulty_profile`).
    Returns a list of dicts describing each branch.
    """
    profile = difficulty_profile(level, difficulty)
    tree_xs = tree_positions(world_width)
    slots = branch_slots(len(tree_xs))
    # Branches stop short of the neighbouring trunk instead of poking through it
    max_length = tree_xs[1] - tree_xs[0] - TRUNK_WIDTH // 2 - 25
    base_min_gap = 120
    base_max_gap = 150

    def new_spec(slot, y, thin, stiffness, length=None):
        if thin:
            # Thin wood already bends a lot, so it only gets half the extra floppiness
            stiffness = 1 - (1 - stiffness) * 0.5
        return {
            "tree": slot[0],
            "direction": slot[1],
            "y": y,
            "length": length or min(random.randint(*BRANCH_LENGTH), max_length),
            "thickness": random.uniform(*(BRANCH_THIN_DEPTH if thin else BRANCH_STURDY_DEPTH)),
            "strength": random.uniform(*THIN_BRANCH_STRENGTH) * KITTY_WEIGHT if thin else 0.0,
            "stiffness": stiffness,
            "gnarl": random.uniform(0.4, 1.0),
            "final": False,
        }

    # Gaps first, so each path branch knows how far it is to the one above
    gaps = []
    for i in range(max_branches):
        progress_fraction = i / float(max_branches)
        min_gap = base_min_gap + int(profile["gap_extra"] * progress_fraction)
        max_gap = base_max_gap + int(profile["gap_extra"] * progress_fraction)
        gaps.append(min(MAX_BRANCH_GAP, random.randint(min_gap, max_gap)))

    # The main path: every branch is within a jump of the one below it
    screen_centres = [world_width * (k + 0.5) / WORLD_SCREENS for k in range(WORLD_SCREENS)]
    prev_span = (world_width / 2 - 50, world_width / 2 + 50)  # Kitty's spot on the ground
    last_slot = None
    # Head out to one side screen, then across to the other, then wherever
    first_side = random.choice([screen_centres[0], screen_centres[-1]])
    other_side = screen_centres[-1] if first_side == screen_centres[0] else screen_centres[0]
    targets = [first_side, other_side] + [random.choice(screen_centres) for _ in range(max_branches)]
    path = []
    y = screen_height
    for i, gap in enumerate(gaps):
        y -= gap
        target_x = targets[i // 4]
        length = min(random.randint(*BRANCH_LENGTH), max_length)
        reachable = [s for s in slots if s != last_slot
                     and horizontal_gap(branch_span(tree_xs, s, length), prev_span) <= MAX_BRANCH_HOP]
        if random.random() < 0.7:
            slot = min(reachable, key=lambda s: abs(sum(branch_span(tree_xs, s, length)) / 2 - target_x))
        else:
            slot = random.choice(reachable)
        # A thin branch can snap, so only make one where Kitty could climb past it without it
        is_last = i == len(gaps) - 1
        thin = (not is_last and random.random() < THIN_BRANCH_CHANCE
                and gap + gaps[i + 1] <= MAX_BRANCH_GAP)
        spec = new_spec(slot, y, thin, profile["stiffness"] + random.uniform(-0.03, 0.03), length)
        path.append(spec)
        prev_span = branch_span(tree_xs, slot, spec["length"])
        last_slot = slot
    path[-1]["final"] = True

    # Fillers: extra branches wherever there's room, at heights along the path
    placed = list(path)
    def has_room(candidate):
        span = branch_span(tree_xs, (candidate["tree"], candidate["direction"]), candidate["length"])
        for other in placed:
            other_span = branch_span(tree_xs, (other["tree"], other["direction"]), other["length"])
            if horizontal_gap(span, other_span) < 40 and abs(other["y"] - candidate["y"]) < 120:
                return False
        return True

    for spec in path:
        for _ in range(FILLERS_PER_PATH_BRANCH):
            fy = spec["y"] + random.randint(-70, 70)
            fy = max(path[-1]["y"] + 60, min(screen_height - 130, fy))
            filler = new_spec(random.choice(slots), fy, random.random() < THIN_BRANCH_CHANCE,
                              profile["stiffness"] + random.uniform(-0.03, 0.03))
            if has_room(filler):
                placed.append(filler)
    return placed

def setup_dog_spawn_candidates(platforms, screen_height):
    """Populate `dog_candidate_platforms` with platforms that are above 1/3 of the level height.
    This computes the vertical span from bottom (SCREEN_HEIGHT) to the topmost platform and
    selects platforms whose y is above the one-third threshold.
    """
    if not platforms:
        return []
    # Determine topmost platform y value
    ys = [p.rect.y for p in platforms if not getattr(p, 'is_ground', False)]
    if not ys:
        return []
    topmost = min(ys)
    level_span = screen_height - topmost
    if level_span <= 0:
        return []

    threshold = screen_height - (level_span * (1.0 / 3.0))
    
    # Candidate platforms: above the threshold, not ground, and strictly lower than the topmost platform
    candidates = [
        p for p in platforms 
        if (p.rect.y < threshold 
            and not getattr(p, 'is_ground', False) 
            and p.rect.y > topmost)
    ]
    
    # Sort candidates top-to-bottom and assign spawn index
    candidates.sort(key=lambda p: p.rect.y)
    for idx, p in enumerate(candidates):
        p.spawn_index = idx
        
    return candidates
