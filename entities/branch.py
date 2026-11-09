import math
import random
import pygame
import branch_engine
from constants import *

class BranchWrapper:
    def __init__(self, x_pos, y_pos, length=600.0, base_depth=40.0, segments=20, stiffness=0.8, gnarl=1.0, direction=1.0, seed=0):
        start_vec = branch_engine.Vec2(x_pos, y_pos)
        self.branch_struct = branch_engine.Branch(
            start_vec, 
            length, 
            base_depth, 
            segments, 
            stiffness,
            gnarl,
            direction,
            seed
        )
        self.direction = 1 if direction >= 0 else -1
        self.is_final = False
        self.is_ground = False
        self.just_snapped = False
        # How far the base is buried inside its trunk; dogs stay out of that part.
        self.trunk_half_width = 0
        self.wind_phase = random.uniform(0, 2 * math.pi)
        self.tuft = [
            (random.uniform(-14, 14), random.uniform(-16, 6), random.randint(6, 11), random.choice(FALL_COLORS))
            for _ in range(7)
        ]
        self._refresh_rect()

    def update_physics(self, sub_steps, dt, gravity, global_damping):
        self.branch_struct.update_physics(sub_steps, dt, gravity, global_damping)

    # ------------------ Game-facing API -------------------
    # Branches and the ground share this interface so Kitty and Dogs can stand on either.

    def make_breakable(self, strength, snap_distance):
        """Snap once the total weight on the branch stays above `strength` for a moment."""
        self.branch_struct.strength = strength
        self.branch_struct.snap_distance = snap_distance

    @property
    def breakable(self):
        return self.branch_struct.strength > 0

    @property
    def broken(self):
        return self.branch_struct.broken_at is not None

    @property
    def strain(self):
        """0 when unloaded, 1 at the point of snapping."""
        if not self.breakable:
            return 0.0
        return self.branch_struct.stress / self.branch_struct.strength

    def settle(self, frames=120):
        """Let the branch sag under its own weight, then freeze it there so levels start still."""
        for _ in range(frames):
            self.update_physics(BRANCH_SUB_STEPS, 1 / FPS, BRANCH_GRAVITY, BRANCH_DAMPING)
        self.branch_struct.rest()
        self._refresh_rect()

    def step(self, dt, breeze_strength, time_elapsed):
        was_broken = self.broken
        gust = breeze_strength * BRANCH_WIND * math.sin(time_elapsed * 2.0 + self.wind_phase)
        # An overloaded branch trembles before it gives way
        gust += random.uniform(-1, 1) * BRANCH_TREMBLE * max(0.0, self.strain - 0.5)
        self.branch_struct.apply_force(0.0, gust)
        self.update_physics(BRANCH_SUB_STEPS, dt, BRANCH_GRAVITY, BRANCH_DAMPING)
        self.just_snapped = self.broken and not was_broken
        self._refresh_rect()

    def surface_under(self, xs):
        """Highest point of the branch's top surface under any of `xs`, or None if off the branch."""
        ys = [y for y in (self.branch_struct.surface_y(x) for x in xs) if y is not None]
        return min(ys) if ys else None

    def apply_load(self, x, weight):
        self.branch_struct.apply_load(x, weight)

    def kick(self, x, speed):
        self.branch_struct.apply_impulse(x, speed)

    def shift(self, dx, dy):
        self.branch_struct.translate(dx, dy)
        self.rect.move_ip(dx, dy)

    def walk_range(self):
        """x-extent a dog can patrol: from the trunk's bark to just short of the tip."""
        top = self.branch_struct.top_contour()
        base_x, tip_x = top[0][0], top[-1][0]
        a = base_x + self.direction * (self.trunk_half_width + 10)
        b = tip_x - self.direction * 10
        return (min(a, b), max(a, b))

    def _refresh_rect(self):
        min_x, min_y, max_x, max_y = self.branch_struct.bounds()
        self.rect = pygame.Rect(int(min_x), int(min_y), int(max_x - min_x) + 1, int(max_y - min_y) + 1)

    def draw(self, screen):
        if (self.rect.bottom < -40 or self.rect.top > SCREEN_HEIGHT + 40
                or self.rect.right < -40 or self.rect.left > SCREEN_WIDTH + 40):
            return
        # Young, breakable wood is paler so players can read the risk
        fill = BARK_YOUNG if self.breakable else BARK_COLOR
        for outline in self.branch_struct.outlines():
            top = outline[:len(outline) // 2]
            pygame.draw.polygon(screen, fill, outline)
            pygame.draw.lines(screen, BARK_DARK, True, outline, 2)
            # Sunlit ridge along the top so the walkable surface reads clearly
            pygame.draw.lines(screen, BARK_LIGHT, False, [(x, y + 2) for x, y in top], 2)
        if self.strain > 0.5 and not self.broken:
            self._draw_cracks(screen)
        top = self.branch_struct.top_contour()
        tip_x, tip_y = top[-1]
        for dx, dy, r, color in self.tuft:
            pygame.draw.circle(screen, color, (int(tip_x + dx * self.direction), int(tip_y + dy)), r)

    def _draw_cracks(self, screen):
        """Jagged splits where the branch will snap, growing as it nears breaking."""
        seg = max(1, int(self.branch_struct.snap_distance / self.branch_struct.segment_len))
        nodes = self.branch_struct.nodes
        top = nodes[seg + 1].pos
        bottom = nodes[seg + 1 + self.branch_struct.segments + 1].pos
        depth = bottom.y - top.y
        reach = min(1.0, (self.strain - 0.5) * 2)
        for side in (0.1, 0.55):
            x, y = top.x, top.y + depth * side
            points = [(x, y)]
            for k in range(3):
                x += self.direction * 4
                y += (5 if k % 2 == 0 else -3) * reach
                points.append((x, y))
            pygame.draw.lines(screen, BARK_DARK, False, points, 2)

    def render(self, screen, camera_offset=0):
        nodes = self.branch_struct.nodes
        half = len(nodes) // 2

        # Draw top contour
        for i in range(half - 1):
            n1 = nodes[i]
            n2 = nodes[i+1]
            pygame.draw.line(screen, (139, 69, 19), (n1.pos.x, n1.pos.y - camera_offset), (n2.pos.x, n2.pos.y - camera_offset), max(2, int(n1.radius)))

        # Draw bottom contour
        for i in range(half, len(nodes) - 1):
            n1 = nodes[i]
            n2 = nodes[i+1]
            pygame.draw.line(screen, (100, 50, 0), (n1.pos.x, n1.pos.y - camera_offset), (n2.pos.x, n2.pos.y - camera_offset), max(2, int(n1.radius)))

        # Draw cross connect lines to make it look like a solid branch
        for i in range(half):
            n1 = nodes[i]
            n2 = nodes[i + half]
            pygame.draw.line(screen, (80, 40, 0), (n1.pos.x, n1.pos.y - camera_offset), (n2.pos.x, n2.pos.y - camera_offset), 2)
