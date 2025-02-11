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

# Level settings
max_levels = 5
current_level = 1
current_difficulty = 1  # 1 = Easy, 2 = Medium, 3 = Hard, etc.
level_complete = False

# Lives
lives = 3

time_elapsed = 0  # Used for the sine function in wind sway

# Colors
BLACK = (0, 0, 0)
SKY_BLUE = (145, 220, 255)
WHITE = (255, 255, 255)

# Fonts for messages and HUD
font_large = pygame.font.SysFont(None, 72)
font_medium = pygame.font.SysFont(None, 55)
font_small = pygame.font.SysFont(None, 36)

congrats_text = font_medium.render("Congratulations Kitty!", True, (0, 128, 0))
game_over_text = font_large.render("Game Over!", True, (255, 0, 0))
restart_text = font_small.render("Press R to Restart or Q to Quit", True, BLACK)

# Load sounds and music
hiss_sound = pygame.mixer.Sound('assets/hiss.wav')
meow_1 = pygame.mixer.Sound('assets/meow.wav')
meow_2 = pygame.mixer.Sound('assets/meow2.wav')
die_sound_duration = 2500  # milliseconds

mixer.music.load('assets/BGM.ogg')
mixer.music.play(-1)  # Loop music

# Miniature kitties for life counters
kitty_mini = pygame.image.load("assets/kitty.png")  # Load the main sprite
kitty_mini = pygame.transform.scale(kitty_mini, (40, 40))  # Scale it down for lives display

# Splash screen image
splash_image = pygame.image.load('assets/Kitty_splash.png')
splash_image = pygame.transform.scale(splash_image, (SCREEN_WIDTH, SCREEN_HEIGHT //2))

# Setup display and clock
screen = pygame.display.set_mode((SCREEN_WIDTH, SCREEN_HEIGHT))
pygame.display.set_caption("Kitty Adventure")
clock = pygame.time.Clock()

# Camera variables for vertical scrolling
camera_offset = 0  
camera_follow_kitty = False  # When True, the camera follows Kitty upward

# ------------------ Splash Screen -------------------------
def splash_screen():
    splash_running = True
    while splash_running:
        for event in pygame.event.get():
            if event.type == QUIT:
                pygame.quit()
                sys.exit()
            elif event.type == KEYDOWN or event.type == MOUSEBUTTONDOWN:
                splash_running = False
        screen.fill(SKY_BLUE)
        splash_rect = splash_image.get_rect(center=(SCREEN_WIDTH // 2, SCREEN_HEIGHT // 2))
        screen.blit(splash_image, splash_rect)
        pygame.display.flip()
        clock.tick(FPS)
# ------------------ Game Over Screen -------------------------
def game_over_screen():
    global lives, current_level
    over = True
    while over:
        for event in pygame.event.get():
            if event.type == QUIT:
                pygame.quit()
                sys.exit()
            elif event.type == KEYDOWN:
                if event.key == K_r:
                    # Restart the game: reset lives and level
                    lives = 3
                    current_level = 1
                    restart_current_level(regenerate=True)
                    over = False
                elif event.key == K_q:
                    pygame.quit()
                    sys.exit()

        screen.fill(SKY_BLUE)
        screen.blit(game_over_text, ((SCREEN_WIDTH - game_over_text.get_width()) // 2, SCREEN_HEIGHT // 3))
        screen.blit(restart_text, ((SCREEN_WIDTH - restart_text.get_width()) // 2, SCREEN_HEIGHT // 2))
        pygame.display.flip()
        clock.tick(FPS)

# ------------------ Breeze Generation -----------------------
def get_breeze_strength():
    """
    Calculates breeze strength as a slow oscillating sine wave.
    The value smoothly increases and decreases over time.
    """
    global time_elapsed
    base_strength = 1.5  # Base force of the wind
    variability = 3.0  # How much the wind fluctuates
    cycle_speed = 0.1  # Speed of oscillation 
    
    return base_strength + variability * math.sin(cycle_speed * time_elapsed)

# ------------------ Platform Generation -----------------------
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

# ------------------ Platform Class ----------------------------
class Platform(pygame.sprite.Sprite):
    def __init__(self, platform_data):
        super().__init__()
        x, y, width, height = platform_data
        self.surf = pygame.Surface((width, height))
        self.surf.fill(BLACK)
        self.rect = self.surf.get_rect(topleft=(x, y))
        self.original_x = x
        self.base_y = y  
        # We no longer use a random amplitude or undulation_speed.
        self.phase = random.uniform(0, 2 * math.pi)  # For vertical oscillation timing.
        self.offset_phase = random.uniform(0, 2 * math.pi)  # For horizontal sway offset.
        self.prev_rect = self.rect.copy()

    def update(self, breeze_strength):
        global time_elapsed

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

        # Apply wind push (breeze strength affects this)
        self.rect.x += breeze_strength * 0.2  # Scaled down to avoid excessive push

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

# ------------------ Ground Platform ---------------------------
def create_ground():
    ground = Platform((0, SCREEN_HEIGHT - 10, SCREEN_WIDTH, 10))
    ground.is_ground = True  # mark this platform as ground
    return ground


# ------------------ Restart Current Level ---------------------
def restart_current_level(regenerate=False):
    """
    Resets the current level without changing the current_level counter.
    If 'regenerate' is True, new platforms are generated.
    """
    global platforms, final_platform_data, camera_offset, level_complete, all_sprites, kitty

    camera_offset = 0
    kitty.rect.midbottom = (SCREEN_WIDTH // 2, SCREEN_HEIGHT - 50)
    kitty.velocity = 0
    kitty.falling = False
    kitty.jump = False
    level_complete = False

    # Determine how many platforms to generate.
    # For level 1 we generated more platforms; subsequent levels use fewer.
    max_plats = 11 if current_level == 1 else 10

    if regenerate:
        platforms.empty()
        all_sprites.empty()
        all_sprites.add(kitty)
        platform_data, temp_final_platform = generate_platforms(
            max_plats,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
            level=current_level,
            difficulty=current_difficulty
        )
        for plat_data in platform_data:
            new_platform = Platform(plat_data)
            platforms.add(new_platform)
            all_sprites.add(new_platform)
        # Always add the ground platform
        ground = create_ground()
        platforms.add(ground)
        all_sprites.add(ground)
        final_platform_data = (
            temp_final_platform[0],
            temp_final_platform[1],  # camera_offset is 0 now
            temp_final_platform[2],
            temp_final_platform[3]
        )

# ------------------ Level Transition --------------------------
def start_new_level():
    """
    Increments the level (or resets if final level is reached) and generates a new set of platforms.
    """
    global platforms, final_platform_data, camera_offset, level_complete, current_level, all_sprites, kitty
    if current_level < max_levels:
        current_level += 1
    else:
        # All levels complete—restart from level 1.
        print("All levels complete! You Win!")
        pygame.time.delay(3000)
        current_level = 1

    platforms.empty()
    all_sprites.empty()
    all_sprites.add(kitty)
    # For subsequent levels we generate fewer platforms.
    platform_data, temp_final_platform = generate_platforms(
        max_platforms=10,
        screen_width=SCREEN_WIDTH,
        screen_height=SCREEN_HEIGHT,
        level=current_level,
        difficulty=current_difficulty
    )
    for plat_data in platform_data:
        new_platform = Platform(plat_data)
        platforms.add(new_platform)
        all_sprites.add(new_platform)
    ground = create_ground()
    platforms.add(ground)
    all_sprites.add(ground)
    camera_offset = 0
    kitty.rect.midbottom = (SCREEN_WIDTH // 2, SCREEN_HEIGHT - 50)
    kitty.velocity = 0
    kitty.falling = False
    kitty.jump = False
    level_complete = False
    final_platform_data = (
        temp_final_platform[0],
        temp_final_platform[1],  # camera_offset is 0
        temp_final_platform[2],
        temp_final_platform[3]
    )

# ------------------ Sprite Groups Setup -----------------------
platforms = pygame.sprite.Group()
# For the initial level, generate a larger set of platforms.
platform_data, final_platform_data = generate_platforms(
    max_platforms=11,
    screen_width=SCREEN_WIDTH,
    screen_height=SCREEN_HEIGHT,
    level=current_level,
    difficulty=current_difficulty
)
for plat_data in platform_data:
    platforms.add(Platform(plat_data))
# Add the ground platform
ground = create_ground()
platforms.add(ground)

kitty = Kitty()

# Group for drawing all sprites
all_sprites = pygame.sprite.Group()
all_sprites.add(kitty)
for platform in platforms:
    all_sprites.add(platform)

# ------------------ HUD Drawing -------------------------
def draw_hud():
    # Draw a semi-transparent background behind HUD
    hud_background = pygame.Surface((SCREEN_WIDTH, 50), pygame.SRCALPHA)
    hud_background.fill((255, 255, 255, 180))  # White with 70% opacity
    screen.blit(hud_background, (0, 0))

    # Display Level Number on the Left
    level_hud = font_small.render(f"Level: {current_level}", True, BLACK)
    screen.blit(level_hud, (10, 10))

    # Display Miniature Kitty Sprites for Lives on the Right
    for i in range(lives):
        screen.blit(kitty_mini, (SCREEN_WIDTH - (i + 1) * 45, 5))  # Offset each life icon


# ------------------ Main Game Loop ----------------------------
def main_game():
    global camera_offset, camera_follow_kitty, time_elapsed, level_complete, lives
    running = True
    while running:
        dt = clock.tick(FPS) / 1000.0  # Delta time in seconds
        time_elapsed += dt
        
        # Get current breeze strength
        breeze_strength = get_breeze_strength()
        
        # --- Event Handling ---
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
        kitty.update(pressed_keys, platforms, breeze_strength)


        # Update platforms with the new breeze strength
        for platform in platforms:
            platform.update(breeze_strength)
        
        # --- Camera vertical scrolling ---
        if kitty.jump and kitty.rect.top <= SCREEN_HEIGHT / 2:
            camera_follow_kitty = True
        if camera_follow_kitty and not kitty.falling:
            camera_movement = max(0, (SCREEN_HEIGHT / 2) - kitty.rect.top)
            camera_offset += camera_movement
            kitty.rect.y += camera_movement
            for platform in platforms:
                platform.rect.y += camera_movement
                platform.base_y += camera_movement  # Update base_y to include camera movement.

        if kitty.falling:
            camera_follow_kitty = False

        # --- Life Lost if Kitty Falls ---
        if kitty.rect.top > SCREEN_HEIGHT:
            lives -= 1
            if lives > 0:
                pygame.time.delay(1000)
                restart_current_level(regenerate=True)
                continue  # Skip rest of loop this frame.
            else:
                game_over_screen()
                continue  # After game over, restart loop.

#        # --- Apply Breeze (Platform Sway) ---
#        for platform in platforms:
#            sway_offset = breeze_amplitude * math.sin(breeze_speed * time_elapsed + platform.offset_phase)
#            platform.rect.x = platform.original_x + sway_offset

        # --- Collision Detection for Landing on Platforms ---
        collisions = pygame.sprite.spritecollide(kitty, platforms, False)
        for platform in collisions:
            # Only process landing if Kitty is falling.
            if kitty.velocity >= 0 and kitty.previous_rect.bottom <= platform.rect.top and kitty.rect.bottom >= platform.rect.top:
                kitty.falling = False
                kitty.velocity = 0
                kitty.rect.bottom = platform.rect.top
                
                # Calculate how far the platform moved this frame.
                dx = platform.rect.x - platform.prev_rect.x
                dy = platform.rect.y - platform.prev_rect.y
                
                # Carry Kitty along with the platform.
                kitty.rect.x += dx
                kitty.rect.y += dy
                break


        # --- Level Complete Check ---
        # Build a test final platform based on final_platform_data (with camera_offset already applied via restart/new-level)
        test_platform = Platform((
            final_platform_data[0],
            final_platform_data[1] + camera_offset,  # camera_offset was reset when generating level
            final_platform_data[2],
            final_platform_data[3]
        ))
        # Check that Kitty is falling and her bottom is (within a tolerance) exactly at the top of the final platform.
        if (not level_complete and kitty.velocity >= 0 and kitty.previous_rect.bottom <= test_platform.rect.top and kitty.rect.bottom >= test_platform.rect.top):
            kitty.falling = False
            kitty.velocity = 0
            kitty.rect.bottom = platform.rect.top
            level_complete = True
            print("Level complete!")
            screen.blit(congrats_text, ((SCREEN_WIDTH - congrats_text.get_width()) // 2, SCREEN_HEIGHT // 2))
            pygame.display.flip()
            pygame.time.delay(2000)
            start_new_level()

        # --- Drawing ---
        screen.fill(SKY_BLUE)

        for entity in all_sprites:
            if isinstance(entity, Kitty):
                screen.blit(entity.image, entity.rect)
            else:
                # Only draw platforms that are visible
                if entity.rect.top > -camera_offset:
                    screen.blit(entity.surf, entity.rect)

        draw_hud()  # Always draw HUD last so platforms don’t obscure it

        pygame.display.flip()


# ------------------ Start the Game -------------------------
splash_screen()
main_game()
