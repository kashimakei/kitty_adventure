import random
import pygame
from constants import *
from entities.branch import BranchWrapper

class Tree:
    """A trunk that branches grow out of. It has no collision: Kitty passes straight through it."""

    def __init__(self, x, top_y, bottom_y, width=TRUNK_WIDTH):
        self.x = x
        self.width = width
        self.top_y = top_y
        self.bottom_y = bottom_y
        self.branches = []
        self.trunk_image = self._make_trunk(int(bottom_y - top_y))
        self.canopy_image = self._make_canopy()

    def grow_branch(self, y, direction, length, stiffness, gnarl, thickness, strength=0.0):
        """`strength` is the total weight a branch holds before snapping; 0 never snaps."""
        # Never fewer than 8 segments, so even short branches bend smoothly
        segments = max(8, round(length / BRANCH_SEGMENT_LEN))
        branch = BranchWrapper(self.x, y, length, thickness, segments, stiffness, gnarl, direction,
                               seed=random.getrandbits(32))
        branch.trunk_half_width = self.width / 2
        if strength > 0:
            # Breaks right where it leaves the bark
            branch.make_breakable(strength, self.width / 2)
        branch.settle()
        self.branches.append(branch)
        return branch

    def shift(self, dx, dy):
        self.x += dx
        self.top_y += dy
        self.bottom_y += dy

    def _make_trunk(self, height):
        surf = pygame.Surface((self.width, height))
        surf.fill(TRUNK_COLOR)
        # Vertical bark furrows
        for _ in range(height // 12):
            x = random.randint(3, self.width - 4)
            y = random.randint(0, height)
            length = random.randint(20, 70)
            color = random.choice([BARK_DARK, BARK_COLOR])
            pygame.draw.line(surf, color, (x, y), (x + random.randint(-2, 2), y + length), random.randint(1, 3))
        # Shaded edges give the trunk some roundness
        pygame.draw.rect(surf, BARK_DARK, (0, 0, 5, height))
        pygame.draw.rect(surf, BARK_DARK, (self.width - 5, 0, 5, height))
        return surf

    def _make_canopy(self):
        w, h = 220, 160
        surf = pygame.Surface((w, h), pygame.SRCALPHA)
        for _ in range(28):
            r = random.randint(22, 40)
            cx = random.randint(r, w - r)
            cy = random.randint(r, h - r)
            pygame.draw.circle(surf, random.choice(FALL_COLORS), (cx, cy), r)
        return surf

    def draw(self, screen):
        if not -self.width < self.x < SCREEN_WIDTH + self.width + 110:
            return
        top = max(self.top_y, -10)
        bottom = min(self.bottom_y, SCREEN_HEIGHT + 10)
        if bottom > top:
            area = pygame.Rect(0, int(top - self.top_y), self.width, int(bottom - top))
            screen.blit(self.trunk_image, (self.x - self.width // 2, int(top)), area)
        canopy_rect = self.canopy_image.get_rect(midbottom=(self.x, int(self.top_y) + 60))
        if canopy_rect.bottom > 0:
            screen.blit(self.canopy_image, canopy_rect)
