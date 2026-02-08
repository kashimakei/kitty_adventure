
import random
import math
from constants import *

def get_breeze_strength(time_elapsed):
    """
    Calculates breeze strength as a slow oscillating sine wave.
    The value smoothly increases and decreases over time.
    """
    base_strength = 1.5  # Base force of the wind
    variability = 5.0  # How much the wind fluctuates
    cycle_speed = 0.5  # Speed of oscillation 
    
    return base_strength + variability * math.sin(cycle_speed * time_elapsed)

def generate_platforms(max_platforms, screen_width, screen_height, level=1, difficulty=1):
    """
    Generates platforms that start large and close together, and become smaller and further apart.
    """
    platforms_data = []
    # Base parameters
    base_min_width = 100
    base_max_width = 200
    base_min_gap = 120
    base_max_gap = 150

    # Adjustments based on level and difficulty:
    width_adjustment = difficulty * level * 5        # Shrinks platform widths as level increases
    gap_adjustment   = difficulty * level * 10         # Increases vertical gap as level increases

    last_platform_y = screen_height  # Start from the bottom

    for i in range(max_platforms):
        progress_fraction = i / float(max_platforms)
        current_min_width = max(30, int(base_min_width - width_adjustment * progress_fraction))
        current_max_width = max(40, int(base_max_width - width_adjustment * progress_fraction))
        # Interpolate the vertical gap: platforms get further apart toward the top
        min_gap = base_min_gap + int(gap_adjustment * progress_fraction)
        max_gap = base_max_gap + int(gap_adjustment * progress_fraction)
        width = random.randint(current_min_width, current_max_width)
        vertical_gap = random.randint(min_gap, max_gap)
        # Randomly align the platform to the left or right edge
        if random.choice([True, False]):
            x_position = 0
        else:
            x_position = screen_width - width
        y_position = last_platform_y - vertical_gap
        last_platform_y = y_position
        platform_tuple = (x_position, y_position, width, 20)  # 20 is the platform thickness
        platforms_data.append(platform_tuple)
    final_platform_data = platforms_data[-1]  # The last generated platform is the final one
    return platforms_data, final_platform_data

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
