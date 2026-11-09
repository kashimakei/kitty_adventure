import pygame
from constants import *

class Kitty(pygame.sprite.Sprite):
    def __init__(self, meow_sounds):
        super().__init__()
        img = pygame.image.load('assets/kitty.png').convert_alpha()
        self.original_image = pygame.transform.scale(img, (100, 100))
        self.flipped_image = pygame.transform.flip(self.original_image, True, False)
        self.image = self.original_image
        self.rect = self.image.get_rect(center=(SCREEN_WIDTH // 2, SCREEN_HEIGHT - 50))
        self.speed = 5
        self.jump = False
        self.falling = False
        self.velocity = 0
        self.jump_start_time = None
        self.max_jump_duration = 0.5  # seconds
        self.gravity = 0.5
        self.upward_acceleration = -1
        self.previous_rect = self.rect.copy()
        self.meow_sounds = meow_sounds
        self.meow_index = 0
        self.standing_on = None  # the branch (or ground) under Kitty's paws
        self.wind_carry = 0.0

    def foot_xs(self):
        return (self.rect.centerx - KITTY_FOOT_REACH, self.rect.centerx, self.rect.centerx + KITTY_FOOT_REACH)

    def check_falling(self):
        # Walked off the tip, or back past the base through the trunk
        if self.standing_on is None or self.standing_on.surface_under(self.foot_xs()) is None:
            self.standing_on = None
            if not self.falling:
                self.falling = True
                self.velocity = 0

    def resolve_supports(self, supports):
        """Ride the support Kitty is on, or catch one while falling.
        Returns the support Kitty just landed on, if any."""
        if self.standing_on is not None:
            y = self.standing_on.surface_under(self.foot_xs())
            if y is not None:
                self.rect.bottom = y
                # Press down through whichever paw is on the branch, centre first
                feet = sorted(self.foot_xs(), key=lambda x: abs(x - self.rect.centerx))
                for x in feet:
                    if self.standing_on.surface_under([x]) is not None:
                        self.standing_on.apply_load(x, KITTY_WEIGHT)
                        break
            return None

        # Branches are one-way: Kitty jumps up through them and only lands coming down
        if self.jump or self.velocity < 0:
            return None
        best, best_y = None, None
        for support in supports:
            y = support.surface_under(self.foot_xs())
            if y is None:
                continue
            if self.previous_rect.bottom <= y + LANDING_TOLERANCE and self.rect.bottom >= y:
                if best_y is None or y < best_y:
                    best, best_y = support, y
        if best is None:
            return None

        best.kick(self.rect.centerx, self.velocity * LANDING_KICK)
        self.rect.bottom = best_y
        self.falling = False
        self.velocity = 0
        self.standing_on = best
        return best

    def update(self, pressed_keys, breeze_strength, bounds=(0, SCREEN_WIDTH)):
        """`bounds` are the world's left and right edges in screen coordinates."""
        self.previous_rect = self.rect.copy()

        # Move left/right with arrow keys
        if pressed_keys[pygame.K_LEFT]:
            self.rect.x -= self.speed
        if pressed_keys[pygame.K_RIGHT]:
            self.rect.x += self.speed

        # Flip sprite based on horizontal movement direction
        dx = self.rect.x - self.previous_rect.x
        if dx > 0:
            # moving right
            self.image = self.original_image
        elif dx < 0:
            # moving left
            self.image = self.flipped_image

        # The wind pushes Kitty around while airborne (breeze strength affects this).
        # Fractions of a pixel are carried over so gentle breezes still cause drift.
        if self.jump or self.falling:
            self.wind_carry += breeze_strength * 0.3  # Scaled down to avoid excessive push
            drift = int(self.wind_carry)
            self.rect.x += drift
            self.wind_carry -= drift

        # Keep Kitty inside the world
        if self.rect.left < bounds[0]:
            self.rect.left = bounds[0]
        if self.rect.right > bounds[1]:
            self.rect.right = bounds[1]

        # Jumping and gravity logic
        if self.jump:
            jump_time = (pygame.time.get_ticks() - self.jump_start_time) / 1000.0
            if jump_time < self.max_jump_duration:
                self.velocity += self.upward_acceleration
            else:
                self.falling = True
                self.jump = False
                self.velocity = 0
        if self.falling:
            self.velocity += self.gravity
        self.rect.y += self.velocity
        if not self.jump:
            self.check_falling()

    def do_jump(self):
        if not self.jump and not self.falling:
            if self.standing_on is not None:
                self.standing_on.kick(self.rect.centerx, JUMP_KICK)
                self.standing_on = None
            self.jump = True
            self.jump_start_time = pygame.time.get_ticks()
            self.meow_sounds[self.meow_index].play()
            self.meow_index = (self.meow_index + 1) % len(self.meow_sounds)

    def stop_jump(self):
        if self.jump:
            self.velocity = self.gravity
            self.falling = True
            self.jump = False
