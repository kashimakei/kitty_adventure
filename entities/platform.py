import random
import math
import pygame
from constants import *

class Leaf(pygame.sprite.Sprite):
    def __init__(self, x, y):
        super().__init__()
        # Create a leaf surface
        self.size = random.randint(5, 15)
        self.original_image = pygame.Surface((self.size, self.size), pygame.SRCALPHA)
        
        # Draw a leaf shape
        color = random.choice(FALL_COLORS)
        points = [
            (self.size//2, 0),  # top
            (self.size, self.size//2),  # right
            (self.size//2, self.size),  # bottom
            (0, self.size//2),  # left
        ]
        pygame.draw.polygon(self.original_image, color, points)
        
        self.image = self.original_image
        self.rect = self.image.get_rect(center=(x, y))
        
        # Movement variables
        self.x = float(x)
        self.y = float(y)
        self.angle = random.uniform(0, 360)
        self.rotation_speed = random.uniform(-2, 2)
        self.fall_speed = random.uniform(1, 2)
        self.horizontal_speed = 0
        self.oscillation_phase = random.uniform(0, 2 * math.pi)
        self.oscillation_speed = random.uniform(1, 3)

    def update(self, breeze_strength, dt):
        # Update position based on breeze and natural falling
        self.horizontal_speed = breeze_strength * 1.5
        
        # Add oscillating motion
        oscillation = math.sin(self.oscillation_phase) * 0.5
        self.oscillation_phase += self.oscillation_speed * dt
        
        # Update position
        self.x += (self.horizontal_speed + oscillation) * dt * 10
        self.y += self.fall_speed * dt * 60
        
        # Rotate leaf
        self.angle += (self.rotation_speed + breeze_strength * 0.5) * dt * 60
        self.image = pygame.transform.rotate(self.original_image, self.angle)
        
        # Update rect position
        self.rect = self.image.get_rect()
        self.rect.center = (self.x, self.y)
        
        # Reset if leaf goes off screen
        if (self.rect.right < 0 or self.rect.left > SCREEN_WIDTH or
            self.rect.top > SCREEN_HEIGHT):
            self.reset()

    def reset(self):
        # Reset leaf to top of screen at random x position
        self.x = random.randint(0, SCREEN_WIDTH)
        self.y = -50
        self.rect.center = (self.x, self.y)
        self.angle = random.uniform(0, 360)
        self.rotation_speed = random.uniform(-2, 2)
        self.fall_speed = random.uniform(1, 2)

class Platform(pygame.sprite.Sprite):
    def __init__(self, platform_data, image):
        super().__init__()
        x, y, width, height = platform_data
        self.image = pygame.transform.scale(image, (width, height))
        self.rect = self.image.get_rect(topleft=(x, y))
        self.original_x = x
        self.base_y = y  
        self.phase = random.uniform(0, 2 * math.pi)  # For vertical oscillation timing.
        self.offset_phase = random.uniform(0, 2 * math.pi)  # For horizontal sway offset.
        self.prev_rect = self.rect.copy()
        self.is_final = False

    def update(self, breeze_strength, time_elapsed):
        # Save previous position for potential use in carrying Kitty.
        self.prev_rect = self.rect.copy()

        # Horizontal sway: Use breeze_strength and time_elapsed.
        sway_offset = breeze_strength * math.sin(time_elapsed + self.offset_phase)
        self.rect.x = self.original_x + sway_offset

        # Vertical undulation: Skip if this is the ground platform.
        if not getattr(self, "is_ground", False):
            self.phase += 0.05  # Fixed increment for undulation timing.
            undulation_offset = breeze_strength * math.sin(self.phase)
            self.rect.y = self.base_y + undulation_offset

    # Same support interface as BranchWrapper, so Kitty can stand on the ground too.
    def surface_under(self, xs):
        if any(self.rect.left <= x <= self.rect.right for x in xs):
            return self.rect.top
        return None

    def apply_load(self, x, weight):
        pass

    def kick(self, x, speed):
        pass

    def shift(self, dx, dy):
        self.rect.move_ip(dx, dy)
        self.original_x += dx
        self.base_y += dy

    def walk_range(self):
        return (self.rect.left + 10, self.rect.right - 10)

class Dog(pygame.sprite.Sprite):
    def __init__(self, platform, speed_scale=1.0):
        super().__init__()
        try:
            img = pygame.image.load('assets/Fierce_Dog.png').convert_alpha()
            self.original_image = pygame.transform.smoothscale(img, (100, 60))
        except Exception:
            self.original_image = pygame.Surface((100, 60), pygame.SRCALPHA)
            self.original_image.fill((120, 20, 20))
        self.image = self.original_image
        self.platform = platform
        lo, hi = platform.walk_range()
        self.x = (lo + hi) / 2
        self.rect = self.image.get_rect()
        self.speed = random.uniform(20, 45) * speed_scale
        self.direction = random.choice([-1, 1])
        self.fall_speed = None  # set once the branch snaps out from under the dog
        self._stand()

    def _stand(self):
        y = self.platform.surface_under([self.x])
        if y is None:
            y = self.platform.rect.top
        self.rect.midbottom = (int(self.x), int(y))

    def shift(self, dx, dy):
        # A standing dog re-plants itself on its branch each frame; only its x needs carrying along
        self.x += dx
        if self.fall_speed is not None:
            self.rect.move_ip(dx, dy)

    def update(self, dt):
        if self.fall_speed is None and getattr(self.platform, "broken", False):
            self.fall_speed = 0.0
            self.image = pygame.transform.flip(self.image, False, True)
        if self.fall_speed is not None:
            self.fall_speed += 1400 * dt
            self.rect.y += self.fall_speed * dt
            if self.rect.top > SCREEN_HEIGHT:
                self.kill()
            return

        lo, hi = self.platform.walk_range()
        self.x += self.direction * self.speed * dt
        if self.x < lo:
            self.x = lo
            self.direction = 1
        if self.x > hi:
            self.x = hi
            self.direction = -1

        self._stand()
        self.platform.apply_load(self.x, DOG_WEIGHT)

        if self.direction < 0:
            self.image = pygame.transform.flip(self.original_image, True, False)
        else:
            self.image = self.original_image

class Eagle(pygame.sprite.Sprite):
    def __init__(self, kitty, speed_scale=1.0):
        super().__init__()
        try:
            img = pygame.image.load('assets/Flying_Eagle.png').convert_alpha()
            self.original_image = pygame.transform.smoothscale(img, (120, 80))
        except Exception:
            self.original_image = pygame.Surface((120, 80), pygame.SRCALPHA)
            self.original_image.fill((100, 100, 0)) # Placeholder color

        # Determine side to swoop in from
        self.side = random.choice(['left', 'right'])
        
        start_y = random.randint(100, SCREEN_HEIGHT // 2)
        if self.side == 'left':
            self.rect = self.original_image.get_rect(midright=(0, start_y))
            self.velocity_x = random.randint(150, 250) * speed_scale
            self.image = self.original_image 
        else:
            self.rect = self.original_image.get_rect(midleft=(SCREEN_WIDTH, start_y))
            self.velocity_x = random.randint(-250, -150) * speed_scale
            self.image = pygame.transform.flip(self.original_image, True, False)

        self.target_y = kitty.rect.y
        self.velocity_y = (self.target_y - start_y) / (SCREEN_WIDTH / abs(self.velocity_x)) 
        self.velocity_y = max(-100, min(100, self.velocity_y))
        
        if self.velocity_x < 0:
            self.image = self.original_image
        else:
            self.image = pygame.transform.flip(self.original_image, True, False)

    def update(self, dt):
        self.rect.x += self.velocity_x * dt
        self.rect.y += self.velocity_y * dt

        if (self.velocity_x > 0 and self.rect.left > SCREEN_WIDTH) or \
           (self.velocity_x < 0 and self.rect.right < 0):
            self.kill()
