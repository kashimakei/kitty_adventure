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
        old_center = self.rect.center
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
        
        # Optional attribute for final platform, set by level generator
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

    def check_falling(self, platforms):
        # Move Kitty 1 pixel down temporarily to check for a platform underneath
        self.rect.y += 1
        if not pygame.sprite.spritecollideany(self, platforms):
            if not self.falling:
                self.falling = True
                self.velocity = 0
        self.rect.y -= 1

    def update(self, pressed_keys, platforms, breeze_strength):
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

        # Apply wind push only while Kitty is jumping (breeze strength affects this)
        if self.jump:
            self.rect.x += breeze_strength * 0.3  # Scaled down to avoid excessive push

        # Keep Kitty inside screen bounds
        if self.rect.left < 0:
            self.rect.left = 0
        if self.rect.right > SCREEN_WIDTH:
            self.rect.right = SCREEN_WIDTH

        # Jumping and gravity logic remains the same
            # Jump/fall logic
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
            self.check_falling(platforms)

    def do_jump(self):
        if not self.jump and not self.falling:
            self.jump = True
            self.jump_start_time = pygame.time.get_ticks()
            self.meow_sounds[self.meow_index].play()
            self.meow_index = (self.meow_index + 1) % len(self.meow_sounds)

    def stop_jump(self):
        if self.jump:
            self.velocity = self.gravity
            self.falling = True
            self.jump = False

class Dog(pygame.sprite.Sprite):
    def __init__(self, platform):
        super().__init__()
        try:
            img = pygame.image.load('assets/Fierce_Dog.png').convert_alpha()
            self.original_image = pygame.transform.smoothscale(img, (100, 60))
        except Exception:
            self.original_image = pygame.Surface((100, 60), pygame.SRCALPHA)
            self.original_image.fill((120, 20, 20))
        self.image = self.original_image
        self.platform = platform
        self.offset_x = max(10, min(platform.rect.width - 20, platform.rect.width // 2))
        self.rect = self.image.get_rect()
        self.rect.midbottom = (platform.rect.left + self.offset_x, platform.rect.top)
        self.speed = random.uniform(20, 45) 
        self.direction = random.choice([-1, 1])

        if self.direction < 0:
            self.image = pygame.transform.flip(self.original_image, True, False)
        else:
            self.image = self.original_image

    def update(self, dt):
        self.offset_x += self.direction * self.speed * dt
        if self.offset_x < 10:
            self.offset_x = 10
            self.direction *= -1
        if self.offset_x > self.platform.rect.width - 10:
            self.offset_x = self.platform.rect.width - 10
            self.direction *= -1

        self.rect.midbottom = (self.platform.rect.left + int(self.offset_x), self.platform.rect.top)

        if self.direction < 0:
            self.image = pygame.transform.flip(self.original_image, True, False)
        else:
            self.image = self.original_image

class Eagle(pygame.sprite.Sprite):
    def __init__(self, kitty):
        super().__init__()
        try:
            img = pygame.image.load('assets/Flying_Eagle.png').convert_alpha()
            # Scale accordingly
            self.original_image = pygame.transform.smoothscale(img, (120, 80))
        except Exception:
            self.original_image = pygame.Surface((120, 80), pygame.SRCALPHA)
            self.original_image.fill((100, 100, 0)) # Placeholder color

        # Determine side to swoop in from (left or right)
        self.side = random.choice(['left', 'right'])
        
        # Set starting position based on side
        start_y = random.randint(100, SCREEN_HEIGHT // 2) # Start from upper half
        if self.side == 'left':
            self.rect = self.original_image.get_rect(midright=(0, start_y))
            self.velocity_x = random.randint(150, 250)
            self.image = self.original_image 
        else:
            self.rect = self.original_image.get_rect(midleft=(SCREEN_WIDTH, start_y))
            self.velocity_x = random.randint(-250, -150)
            # Flip image to face left
            self.image = pygame.transform.flip(self.original_image, True, False)

        # Target kitty's current position roughly? NO, just swoop across
        # Actually, let's target kitty slightly
        self.target_y = kitty.rect.y
        self.velocity_y = (self.target_y - start_y) / (SCREEN_WIDTH / abs(self.velocity_x)) 
        
        # Cap vertical velocity so it doesn't dive too steep
        self.velocity_y = max(-100, min(100, self.velocity_y))
        
        # Ensure the sprite is facing the direction of travel
        if self.velocity_x < 0:
            self.image = self.original_image
        else:
            self.image = pygame.transform.flip(self.original_image, True, False)

    def update(self, dt):
        self.rect.x += self.velocity_x * dt
        self.rect.y += self.velocity_y * dt

        # Kill if off screen
        if (self.velocity_x > 0 and self.rect.left > SCREEN_WIDTH) or \
           (self.velocity_x < 0 and self.rect.right < 0):
            self.kill()
