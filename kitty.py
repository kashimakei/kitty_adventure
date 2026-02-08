
#!/usr/bin/python3
import pygame
import sys
import random
import math
from pygame import mixer
from pygame.locals import *

# Import modules
from constants import *
from sprites import Kitty, Platform, Dog, Leaf, Eagle
from level_utils import get_breeze_strength, generate_platforms, setup_dog_spawn_candidates

# ------------------ Initialization -------------------------
pygame.init()
mixer.init()

# ------------------ Globals & Asset Loading -------------------
# Setup display and clock
screen = pygame.display.set_mode((SCREEN_WIDTH, SCREEN_HEIGHT))
pygame.display.set_caption("Kitty Adventure")
clock = pygame.time.Clock()

# Fonts for messages and HUD
font_large = pygame.font.SysFont(None, 75)
font_medium = pygame.font.SysFont(None, 55)
font_small = pygame.font.SysFont(None, 35)

congrats_text = font_large.render("Congratulations Kitty!", True, (184, 134, 11))
game_over_text = font_large.render("Game Over!", True, (255, 0, 0))
restart_text = font_small.render("Press R to Restart or Q to Quit", True, BLACK)

# Load Images
branch_image = pygame.image.load('assets/branch.png')
# Pass branch image to Platform class if I hadn't already... 
# In sprites.py, Platform takes `image`. I need to pass `branch_image` when creating platforms.

background_image = pygame.image.load('assets/Background_lvl1.png')
background_image = pygame.transform.scale(background_image, (SCREEN_WIDTH, 3 * SCREEN_HEIGHT))

splash_image = pygame.image.load('assets/Kitty_splash.png')
splash_image = pygame.transform.scale(splash_image, (SCREEN_WIDTH, SCREEN_HEIGHT //2))

kitty_mini = pygame.image.load("assets/kitty.png")
kitty_mini = pygame.transform.scale(kitty_mini, (40, 40))

# Load Sounds
hiss_sound = pygame.mixer.Sound('assets/hiss.wav')
meow_1 = pygame.mixer.Sound('assets/meow.wav')
meow_2 = pygame.mixer.Sound('assets/meow2.wav')
meow_sounds = [meow_1, meow_2]

mixer.music.load('assets/BGM.ogg')
mixer.music.play(-1)  # Loop music

# Game State Variables
max_levels = 10
current_level = 5
current_difficulty = 2
level_complete = False
lives = 3
time_elapsed = 0

camera_offset = 0
camera_follow_kitty = False

# Sprite Groups
all_sprites = pygame.sprite.Group()
platforms = pygame.sprite.Group()
leaves = pygame.sprite.Group()
dogs = pygame.sprite.Group()
eagles = pygame.sprite.Group()

# Other State
final_platform_data = None
dog_candidate_platforms = []
next_dog_spawn_time = 0
next_eagle_spawn_time = 0

kitty = None # Will be initialized in setup/start

# ------------------ Helper Functions -------------------

def create_ground():
    ground = Platform((0, SCREEN_HEIGHT - 10, SCREEN_WIDTH, 10), branch_image)
    ground.is_ground = True
    # Overwrite image with simple rect if we want, or just stick with branch stretch
    # Actually the original code just used Platform with branch_image scaled.
    return ground

def initialize_leaves():
    leaves.empty()
    for _ in range(MAX_LEAVES):
        x = random.randint(0, SCREEN_WIDTH)
        y = random.randint(-SCREEN_HEIGHT, 0)
        leaf = Leaf(x, y)
        leaves.add(leaf)

def spawn_dog_for_difficulty(difficulty):
    global dog_candidate_platforms
    
    # Cap concurrent dogs
    max_concurrent = max(0, difficulty)
    if len(dogs) >= max_concurrent:
        return
    if not dog_candidate_platforms:
        return

    occupied_indices = [getattr(d.platform, "spawn_index", None) for d in dogs]

    def allowed(p):
        if any(getattr(d, "platform", None) is p for d in dogs):
            return False
        if p.rect.colliderect(kitty.rect):
            return False
        idx = getattr(p, "spawn_index", None)
        if idx is None:
            return True
        for occ in occupied_indices:
            if occ is None: continue
            if abs(idx - occ) <= 1:
                return False
        return True

    spawn_buffer = 50
    offscreen_candidates = [p for p in dog_candidate_platforms if p.rect.bottom < -spawn_buffer and allowed(p)]
    
    candidates = offscreen_candidates
    if not candidates:
        return

    platform = random.choice(candidates)
    dog = Dog(platform)
    platform = random.choice(candidates)
    dog = Dog(platform)
    dogs.add(dog)

def spawn_eagle_logic():
    global next_eagle_spawn_time
    
    # Cap concurrent eagles to 1 for now to avoid chaos
    if len(eagles) >= 1:
        return

    eagle = Eagle(kitty)
    eagles.add(eagle)
    
    # Schedule next spawn
    wait_time = random.randint(5000, 10000) # 5-10 seconds
    next_eagle_spawn_time = pygame.time.get_ticks() + wait_time

def restart_current_level(regenerate=False):
    global camera_offset, level_complete, final_platform_data, dog_candidate_platforms, next_dog_spawn_time, kitty, current_level, lives

    camera_offset = 0
    kitty.rect.midbottom = (SCREEN_WIDTH // 2, SCREEN_HEIGHT - 50)
    kitty.velocity = 0
    kitty.falling = False
    kitty.jump = False
    level_complete = False

    max_plats = 11 if current_level == 1 else 10

    if regenerate:
        platforms.empty()
        all_sprites.empty()
        all_sprites.add(kitty)
        all_sprites.add(kitty)
        dogs.empty()
        eagles.empty()
        
        plat_data_list, temp_final = generate_platforms(
            max_plats, SCREEN_WIDTH, SCREEN_HEIGHT,
            level=current_level, difficulty=current_difficulty
        )
        
        for i, p_data in enumerate(plat_data_list):
            new_platform = Platform(p_data, branch_image)
            if i == len(plat_data_list) - 1:
                new_platform.is_final = True
            platforms.add(new_platform)
            all_sprites.add(new_platform)
            
        ground = create_ground()
        platforms.add(ground)
        all_sprites.add(ground)
        
        final_platform_data = temp_final
        
        # Recompute candidates
        dog_candidate_platforms = setup_dog_spawn_candidates(platforms.sprites(), SCREEN_HEIGHT)

    # Reset spawn time
    base = {1: 8000, 2: 5000, 3: 3000}.get(current_difficulty, 5000)
    # Reset spawn time
    base = {1: 8000, 2: 5000, 3: 3000}.get(current_difficulty, 5000)
    now = pygame.time.get_ticks()
    next_dog_spawn_time = now + random.randint(int(base*0.5), int(base*1.5))
    next_eagle_spawn_time = now + random.randint(5000, 10000)

def start_new_level():
    global current_level
    if current_level < max_levels:
        current_level += 1
    else:
        print("All levels complete! You Win!")
        pygame.time.delay(3000)
        current_level = 1
    
    restart_current_level(regenerate=True)

# ------------------ Screens -------------------------
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

def draw_hud():
    hud_background = pygame.Surface((SCREEN_WIDTH, 50), pygame.SRCALPHA)
    hud_background.fill((255, 255, 255, 180))
    screen.blit(hud_background, (0, 0))
    level_hud = font_small.render(f"Level: {current_level}", True, BLACK)
    screen.blit(level_hud, (10, 10))
    for i in range(lives):
        screen.blit(kitty_mini, (SCREEN_WIDTH - (i + 1) * 45, 5))

# ------------------ Main Game Loop ----------------------------
def main_game():
    global camera_offset, camera_follow_kitty, time_elapsed, level_complete, lives, next_dog_spawn_time, next_eagle_spawn_time, kitty, dog_candidate_platforms
    
    # Init Kitty
    kitty = Kitty(meow_sounds)
    all_sprites.add(kitty)
    
    # Init Level 1
    restart_current_level(regenerate=True)
    initialize_leaves()
    
    running = True
    while running:
        dt = clock.tick(FPS) / 1000.0
        time_elapsed += dt
        
        breeze_strength = get_breeze_strength(time_elapsed)
        
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

        # Update platforms
        for platform in platforms:
            platform.update(breeze_strength, time_elapsed)
            
        leaves.update(breeze_strength, dt)
        
        # Camera
        if kitty.jump and kitty.rect.top <= SCREEN_HEIGHT / 2:
            camera_follow_kitty = True
        
        if camera_follow_kitty and not kitty.falling:
            camera_movement = max(0, (SCREEN_HEIGHT / 2) - kitty.rect.top) * 0.1
            camera_offset += camera_movement
            kitty.rect.y += camera_movement
            for platform in platforms:
                platform.rect.y += camera_movement
                platform.base_y += camera_movement
            for leaf in leaves:
                leaf.rect.y += camera_movement
                leaf.y += camera_movement
            for eagle in eagles:
                eagle.rect.y += camera_movement

        if kitty.falling:
            camera_follow_kitty = False

        # Life Lost
        if kitty.rect.top > SCREEN_HEIGHT:
            lives -= 1
            if lives > 0:
                pygame.time.delay(1000)
                restart_current_level(regenerate=True)
                continue
            else:
                game_over_screen()
                continue

        # Collision with Platforms
        collisions = pygame.sprite.spritecollide(kitty, platforms, False)
        for platform in collisions:
             if kitty.velocity >= 0 and kitty.previous_rect.bottom <= platform.rect.top and kitty.rect.bottom >= platform.rect.top:
                kitty.falling = False
                kitty.velocity = 0
                kitty.rect.bottom = platform.rect.top
                
                # Check level complete
                if getattr(platform, 'is_final', False) and not level_complete:
                    level_complete = True
                    print("Level complete!")
                    screen.blit(congrats_text, ((SCREEN_WIDTH - congrats_text.get_width()) // 2, SCREEN_HEIGHT // 2))
                    pygame.display.flip()
                    pygame.time.delay(2000)
                    start_new_level()
                    break

                dx = platform.rect.x - platform.prev_rect.x
                dy = platform.rect.y - platform.prev_rect.y
                kitty.rect.x += dx
                kitty.rect.y += dy
                break

        # Dogs
        for dog in list(dogs):
            dog.update(dt)
            
        # Eagles
        for eagle in list(eagles):
            eagle.update(dt)

        # Spawn dogs (Level > 3)
        if current_level > 3:
            now = pygame.time.get_ticks()
            if now >= next_dog_spawn_time:
                spawn_dog_for_difficulty(current_difficulty)
                base = {1: 8000, 2: 5000, 3: 3000}.get(current_difficulty, 5000)
                next_dog_spawn_time = now + random.randint(int(base * 0.5), int(base * 1.5))

        # Spawn eagles (Level >= 5)
        if current_level >= 5:
            now = pygame.time.get_ticks()
            if now >= next_eagle_spawn_time:
                spawn_eagle_logic()

        # Collision with dogs
        hit_dog = pygame.sprite.spritecollideany(kitty, dogs)
        if hit_dog:
            lives -= 1
            try:
                hiss_sound.play()
            except Exception:
                pass
            if lives > 0:
                pygame.time.delay(800)
                restart_current_level(regenerate=True)
                continue
            else:
                game_over_screen()
                continue

        # Collision with Eagles
        hit_eagle = pygame.sprite.spritecollideany(kitty, eagles)
        if hit_eagle:
            lives -= 1
            try:
                hiss_sound.play()
            except Exception:
                pass
                
            if lives > 0:
                pygame.time.delay(1000)
                restart_current_level(regenerate=True)
                continue
            else:
                game_over_screen()
                continue

        # Draw
        background_y = -1.5 * SCREEN_HEIGHT + (camera_offset % (3 * SCREEN_HEIGHT))
        screen.blit(background_image, (0, background_y))
        screen.blit(background_image, (0, background_y + 3 * SCREEN_HEIGHT))

        leaves.draw(screen)

        for entity in all_sprites:
            if isinstance(entity, Kitty):
                screen.blit(entity.image, entity.rect)
            else:
                if entity.rect.top > -camera_offset:
                    screen.blit(entity.image, entity.rect)

        for dog in dogs:
            if -100 < dog.rect.top < SCREEN_HEIGHT + 100:
                screen.blit(dog.image, dog.rect)

        for eagle in eagles:
            screen.blit(eagle.image, eagle.rect)

        draw_hud()
        pygame.display.flip()

if __name__ == "__main__":
    splash_screen()
    main_game()
