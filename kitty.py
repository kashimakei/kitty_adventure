#!/usr/bin/python3
import pygame
import sys
import random
import math
from pygame.locals import *
from pygame import mixer

# ------------------ Initialization -------------------------
pygame.init()
mixer.init()

# ------------------ Constants and Globals -------------------
SCREEN_WIDTH, SCREEN_HEIGHT = 600, 1200
FPS = 60

# Levels and difficulty
max_levels = 5
current_level = 1
current_difficulty = 1  # 1=Easy, 2=Medium, 3=Hard, etc.
level_complete = False

# Breeze/wind parameters
breeze_amplitude = 5.0    # Maximum horizontal sway (pixels)
breeze_speed = 1       # Speed of the sway (frequency)
wind_force_on_kitty = 0.1  # Constant horizontal push on kitty each frame
time_elapsed = 0  # Global time counter for wind sway calculations

# Colors
BLACK = (0, 0, 0)
SKY_BLUE = (145, 220, 255)
WHITE = (255, 255, 255)

# Font and congratulatory text
font = pygame.font.SysFont(None, 55)
congrats_text = font.render('Congratulations Kitty!', True, (0, 128, 0))

# Load sounds and music
hiss_sound = pygame.mixer.Sound('assets/hiss.wav')
meow_1 = pygame.mixer.Sound('assets/meow.wav')
meow_2 = pygame.mixer.Sound('assets/meow2.wav')
die_sound_duration = 2500  # milliseconds

mixer.music.load('assets/BGM.ogg')
mixer.music.play(-1)  # Loop music

# Setup display and clock
screen = pygame.display.set_mode((SCREEN_WIDTH, SCREEN_HEIGHT))
pygame.display.set_caption("Kitty Adventure")
clock = pygame.time.Clock()

# Camera variables for vertical scrolling
camera_offset = 0  
camera_follow_kitty = False  # flag to control when camera follows Kitty

# ------------------ Platform Generation -----------------------
def generate_platforms(max_platforms, screen_width, screen_height, level=1, difficulty=1):
    """
    Generate platforms that change as the level progresses.
    Early platforms are larger and closer together; later ones get smaller
    and are spaced farther apart. The parameters are adjusted by both the
    level and the difficulty.
    """
    platforms = []
    # Base parameters
    base_min_width = 100
    base_max_width = 200
    base_min_gap = 120
    base_max_gap = 150

    # Adjustments based on difficulty and level
    width_adjustment = difficulty * level * 5        # reduce platform width over progress
    gap_adjustment   = difficulty * level * 10         # increase gap over progress

    last_platform_y = screen_height  # Start from the bottom

    for i in range(max_platforms):
        progress_fraction = i / float(max_platforms)
        
        # Interpolate platform width range
        current_min_width = max(30, int(base_min_width - width_adjustment * progress_fraction))
        current_max_width = max(40, int(base_max_width - width_adjustment * progress_fraction))
        
        # Interpolate vertical gap (platforms get further apart)
        min_gap = base_min_gap + int(gap_adjustment * progress_fraction)
        max_gap = base_max_gap + int(gap_adjustment * progress_fraction)
        
        width = random.randint(current_min_width, current_max_width)
        vertical_gap = random.randint(min_gap, max_gap)
        
        # Randomly decide platform alignment: left or right
        if random.choice([True, False]):
            x_position = 0
        else:
            x_position = screen_width - width
        
        y_position = last_platform_y - vertical_gap
        last_platform_y = y_position
        
        platform = (x_position, y_position, width, 20)  # 20 is the platform thickness
        platforms.append(platform)
        
    final_platform = platforms[-1]  # The highest (last) platform
    return platforms, final_platform

# ------------------ Platform Class ----------------------------
class Platform(pygame.sprite.Sprite):
    def __init__(self, platform_data):
        super().__init__()
        x, y, width, height = platform_data
        self.surf = pygame.Surface((width, height))
        self.surf.fill(BLACK)
        self.rect = self.surf.get_rect(topleft=(x, y))
        # Save original x position for swaying
        self.original_x = x
        # Give each platform a random phase offset so they don't all sway identically
        self.offset_phase = random.random() * 2 * math.pi

# ------------------ Kitty Class -------------------------------
class Kitty(pygame.sprite.Sprite):
    def __init__(self):
        super().__init__()
        self.image = pygame.image.load('assets/kitty.png').convert_alpha()
        self.image = pygame.transform.scale(self.image, (100, 100))
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
        self.meow_sounds = [meow_1, meow_2]
        self.meow_index = 0

    def check_falling(self, platforms):
        # Temporarily move kitty down a pixel to check for a platform underneath
        self.rect.y += 1
        if not pygame.sprite.spritecollideany(self, platforms):
            if not self.falling:
                self.falling = True
                self.velocity = 0
        self.rect.y -= 1

    def update(self, pressed_keys, platforms):
        self.previous_rect = self.rect.copy()
        
        # Move left/right with arrow keys
        if pressed_keys[pygame.K_LEFT]:
            self.rect.x -= self.speed
        if pressed_keys[pygame.K_RIGHT]:
            self.rect.x += self.speed
        
        # Apply a small constant wind push
        self.rect.x += wind_force_on_kitty
        
        # Jump logic
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

        # Check if kitty should start falling
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

# ------------------ Ground Platform ---------------------------
def create_ground():
    return Platform((0, SCREEN_HEIGHT - 10, SCREEN_WIDTH, 10))

# ------------------ Level Transition --------------------------
def start_new_level():
    global platforms, final_platform_data, camera_offset, level_complete, current_level, kitty, all_sprites
    # Move to the next level (or reset if final level is complete)
    if current_level < max_levels:
        current_level += 1
    else:
        print("All levels complete! You Win!")
        pygame.time.delay(3000)
        current_level = 1  # Reset to level 1 (or exit the game)
    
    # Generate a new (shorter) set of platforms for the next level
    platform_data, temp_final_platform = generate_platforms(
        max_platforms=10, 
        screen_width=SCREEN_WIDTH,
        screen_height=SCREEN_HEIGHT,
        level=current_level,
        difficulty=current_difficulty
    )
    final_platform_data = (
        temp_final_platform[0],
        temp_final_platform[1] + camera_offset,
        temp_final_platform[2],
        temp_final_platform[3]
    )
    platforms.empty()
    for plat_data in platform_data:
        platforms.add(Platform(plat_data))
    # Always add the ground platform
    ground = create_ground()
    platforms.add(ground)
    
    camera_offset = 0
    kitty.rect.midbottom = (SCREEN_WIDTH // 2, SCREEN_HEIGHT - 50)
    level_complete = False

# ------------------ Sprite Groups Setup -----------------------
platforms = pygame.sprite.Group()

# For the initial level, we generate more platforms (to allow longer upward progress)
platform_data, final_platform_data = generate_platforms(
    max_platforms=31,
    screen_width=SCREEN_WIDTH,
    screen_height=SCREEN_HEIGHT,
    level=current_level,
    difficulty=current_difficulty
)
for plat_data in platform_data:
    platforms.add(Platform(plat_data))
# Add the ground
ground = create_ground()
platforms.add(ground)

kitty = Kitty()

# Group for drawing all sprites
all_sprites = pygame.sprite.Group()
all_sprites.add(kitty)
for platform in platforms:
    all_sprites.add(platform)

# ------------------ Main Game Loop ----------------------------
while True:
    dt = clock.tick(FPS) / 1000.0  # delta time in seconds
    time_elapsed += dt

    # Event handling
    for event in pygame.event.get():
        if event.type == QUIT:
            pygame.quit()
            sys.exit()
        elif event.type == KEYDOWN:
            if event.key == K_SPACE:
                kitty.do_jump()
        elif event.type == KEYUP:
            if event.key == K_SPACE:
                kitty.stop_jump()

    pressed_keys = pygame.key.get_pressed()
    kitty.update(pressed_keys, platforms)
    
    # Start following kitty with the camera if she's jumping and reaches half the screen height
    if kitty.jump and kitty.rect.top <= SCREEN_HEIGHT / 2:
        camera_follow_kitty = True
    if camera_follow_kitty and not kitty.falling:
        camera_movement = max(0, (SCREEN_HEIGHT / 2) - kitty.rect.top)
        camera_offset += camera_movement
        kitty.rect.y += camera_movement
        # Move all platforms downward to simulate upward camera movement
        for platform in platforms:
            platform.rect.y += camera_movement

    if kitty.falling:
        camera_follow_kitty = False

    # Game over: kitty falls below the screen
    if kitty.rect.top > SCREEN_HEIGHT:
        print("Game Over!")
        pygame.quit()
        sys.exit()

    # Apply breeze: each platform sways horizontally with a sine offset
    for platform in platforms:
        sway_offset = breeze_amplitude * math.sin(breeze_speed * time_elapsed + platform.offset_phase)
        platform.rect.x = platform.original_x + sway_offset

    # Check collisions for landing on platforms (only if falling)
    collisions = pygame.sprite.spritecollide(kitty, platforms, False)
    for platform in collisions:
        if kitty.velocity > 0:  # kitty is falling
            if kitty.previous_rect.bottom <= platform.rect.top and kitty.rect.bottom >= platform.rect.top:
                kitty.falling = False
                kitty.velocity = 0
                kitty.rect.bottom = platform.rect.top
                break

    # Check for level completion:
    # Create a temporary sprite for the final platform and check collision.
    test_platform = Platform((
        final_platform_data[0],
        final_platform_data[1] + camera_offset,
        final_platform_data[2],
        final_platform_data[3]
    ))
    if not level_complete and kitty.rect.colliderect(test_platform.rect):
        print("Level complete!")
        level_complete = True
        screen.blit(congrats_text, (100, SCREEN_HEIGHT // 2))
        pygame.display.flip()
        pygame.time.delay(2000)
        start_new_level()

    # Drawing
    screen.fill(SKY_BLUE)
    for entity in all_sprites:
        if isinstance(entity, Kitty):
            screen.blit(entity.image, entity.rect)
        else:
            # Only draw platforms that are visible (optional)
            if entity.rect.top > -camera_offset:
                screen.blit(entity.surf, entity.rect)
    
    pygame.display.flip()
