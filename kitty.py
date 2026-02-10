
#!/usr/bin/python3
import pygame
import sys
import random
import math
from pygame import mixer
from pygame.math import Vector2
from pygame.locals import *

# Import modules
from constants import *
from sprites import Kitty, Platform, Dog, Leaf, Eagle, TrussBranch
from level_utils import get_breeze_strength, generate_platforms, setup_dog_spawn_candidates

# ------------------ Initialization -------------------------
pygame.init()
mixer.init()

# ------------------ Globals & Asset Loading -------------------
# Physics Globals (modifiable via Sliders in Dev Mode)
g_gravity = GRAVITY
g_substeps = SUBSTEPS
g_branch_stiffness = BRANCH_STIFFNESS
g_branch_damping = BRANCH_DAMPING
g_sprite_mass = SPRITE_MASS
g_sprite_restitution = SPRITE_RESTITUTION
g_branch_segments = BRANCH_SEGMENTS

# Setup display and clock
window_width = SCREEN_WIDTH + (SIDEBAR_WIDTH if DEV_MODE else 0)
screen = pygame.display.set_mode((window_width, SCREEN_HEIGHT))
pygame.display.set_caption("Kitty Adventure - Dev Mode" if DEV_MODE else "Kitty Adventure")
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
# Scale background to cover the entire WORLD dimensions
background_image = pygame.transform.scale(background_image, (WORLD_WIDTH, WORLD_HEIGHT))

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

camera_pos = Vector2(0, WORLD_HEIGHT - SCREEN_HEIGHT)
# Removed camera_offset and camera_follow_kitty in favor of camera_pos

# Sprite Groups
all_sprites = pygame.sprite.Group()
platforms = pygame.sprite.Group()
branches = pygame.sprite.Group()
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

# ------------------ UI Components -------------------
class Slider:
    def __init__(self, x, y, w, h, label, min_val, max_val, initial_val, is_int=False):
        self.rect = pygame.Rect(x, y, w, h)
        self.label = label
        self.min_val = min_val
        self.max_val = max_val
        self.val = initial_val
        self.is_int = is_int
        self.grabbed = False
        self.handle_rect = pygame.Rect(x, y, 10, h)
        self.update_handle()

    def update_handle(self):
        pos = (self.val - self.min_val) / (self.max_val - self.min_val)
        self.handle_rect.centerx = self.rect.x + pos * self.rect.width

    def draw(self, surface):
        # Draw label and value
        font = pygame.font.SysFont(None, 24)
        val_str = f"{int(self.val)}" if self.is_int else f"{self.val:.2f}"
        txt = font.render(f"{self.label}: {val_str}", True, BLACK)
        surface.blit(txt, (self.rect.x, self.rect.y - 20))
        
        # Draw track
        pygame.draw.rect(surface, GRAY, self.rect)
        # Draw handle
        pygame.draw.rect(surface, BLACK, self.handle_rect)

    def handle_event(self, event):
        if event.type == pygame.MOUSEBUTTONDOWN:
            if self.handle_rect.collidepoint(event.pos):
                self.grabbed = True
        elif event.type == pygame.MOUSEBUTTONUP:
            self.grabbed = False
        elif event.type == pygame.MOUSEMOTION:
            if self.grabbed:
                x = max(self.rect.left, min(event.pos[0], self.rect.right))
                pos = (x - self.rect.left) / self.rect.width
                self.val = self.min_val + pos * (self.max_val - self.min_val)
                if self.is_int:
                    self.val = round(self.val)
                self.update_handle()
                return True
        return False

# Initialize Sliders
sliders = []
if DEV_MODE:
    sx = SCREEN_WIDTH + 20
    sw = SIDEBAR_WIDTH - 40
    sliders = [
        Slider(sx, 60, sw, 10, "Gravity", 0.0, 2.0, g_gravity),
        Slider(sx, 120, sw, 10, "Substeps", 1, 20, g_substeps, is_int=True),
        Slider(sx, 180, sw, 10, "Branch Stiffness", 0.1, 1.0, g_branch_stiffness),
        Slider(sx, 240, sw, 10, "Branch Damping", 0.9, 1.0, g_branch_damping),
        Slider(sx, 300, sw, 10, "Sprite Mass", 0.1, 5.0, g_sprite_mass),
        Slider(sx, 360, sw, 10, "Sprite Restitution", 0.0, 1.5, g_sprite_restitution),
        Slider(sx, 420, sw, 10, "Branch Segments", 2, 20, g_branch_segments, is_int=True),
    ]

def draw_sidebar():
    if not DEV_MODE: return
    sidebar_rect = pygame.Rect(SCREEN_WIDTH, 0, SIDEBAR_WIDTH, SCREEN_HEIGHT)
    pygame.draw.rect(screen, UI_GRAY, sidebar_rect)
    pygame.draw.line(screen, BLACK, (SCREEN_WIDTH, 0), (SCREEN_WIDTH, SCREEN_HEIGHT), 2)
    
    title = font_small.render("Dev Tuning", True, BLACK)
    screen.blit(title, (SCREEN_WIDTH + 50, 20))
    
    for slider in sliders:
        slider.draw(screen)

def update_physics_globals():
    global g_gravity, g_substeps, g_branch_stiffness, g_branch_damping, g_sprite_mass, g_sprite_restitution, g_branch_segments
    g_gravity = sliders[0].val
    g_substeps = int(sliders[1].val)
    g_branch_stiffness = sliders[2].val
    g_branch_damping = sliders[3].val
    g_sprite_mass = sliders[4].val
    g_sprite_restitution = sliders[5].val
    
    old_segments = g_branch_segments
    g_branch_segments = int(sliders[6].val)
    
    if kitty:
        kitty.mass = g_sprite_mass
        kitty.restitution = g_sprite_restitution
    
    # If segments changed, we might want to regenerate the dev level
    if g_branch_segments != old_segments and DEV_MODE:
        setup_dev_level()

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
        y = random.randint(-SCREEN_HEIGHT, SCREEN_HEIGHT)
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

def setup_dev_level():
    global kitty
    branches.empty()
    platforms.empty()
    all_sprites.empty()
    
    # Re-init Kitty in the middle of the expanded world
    if kitty:
        all_sprites.add(kitty)
        kitty.pos = Vector2(WORLD_WIDTH // 2, WORLD_HEIGHT - 100)
        kitty.vel = Vector2(0, 0)
    
    # Create wider ground
    ground_width = WORLD_WIDTH
    ground = Platform((0, WORLD_HEIGHT - 20, ground_width, 20), branch_image)
    ground.is_ground = True
    platforms.add(ground)
    all_sprites.add(ground)
    
    # Create two tree trunks rising from the ground
    tree1_x = WORLD_WIDTH * 0.3
    tree2_x = WORLD_WIDTH * 0.7
    
    # Create trunks (visual only platforms)
    trunk1 = Platform((tree1_x - 20, 0, 40, WORLD_HEIGHT), branch_image)
    trunk2 = Platform((tree2_x - 20, 0, 40, WORLD_HEIGHT), branch_image)
    platforms.add(trunk1, trunk2)
    all_sprites.add(trunk1, trunk2)
    
    # Add branches to trees up to the top of the expanded world
    # Adjust spacing for better reachability (150-200 pixels)
    for y in range(400, WORLD_HEIGHT - 300, 250):
        # Tree 1 branches (pointing right)
        b1 = TrussBranch((tree1_x, y), 200, g_branch_segments, 40, 10, angle_deg=0)
        branches.add(b1)
        # Tree 2 branches (pointing left)
        b2 = TrussBranch((tree2_x, y + 125), 200, g_branch_segments, 40, 10, angle_deg=180)
        branches.add(b2)

def restart_current_level(regenerate=False):
    global camera_pos, level_complete, final_platform_data, dog_candidate_platforms, next_dog_spawn_time, kitty, current_level, lives

    camera_pos = Vector2(0, WORLD_HEIGHT - SCREEN_HEIGHT)
    if kitty:
        kitty.pos = Vector2(WORLD_WIDTH // 2, WORLD_HEIGHT - 100)
        kitty.vel = Vector2(0, 0)
        kitty.falling = False
        kitty.jump = False
    level_complete = False

    if DEV_MODE:
        setup_dev_level()
        return

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
    kitty = Kitty(meow_sounds, g_sprite_mass, g_sprite_restitution)
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
            
            if DEV_MODE:
                for slider in sliders:
                    if slider.handle_event(event):
                        update_physics_globals()

        pressed_keys = pygame.key.get_pressed()
        kitty.update(pressed_keys, branches, platforms, breeze_strength, dt, g_gravity)

        # Update branches
        for branch in branches:
            branch.update(dt, g_gravity, g_branch_damping, g_substeps, g_branch_stiffness)

        # Update platforms
        for platform in platforms:
            platform.update(breeze_strength, time_elapsed)
            
        leaves.update(breeze_strength, dt)
        
        # Camera System
        target_cam_x = kitty.pos.x - SCREEN_WIDTH // 2
        target_cam_y = kitty.pos.y - SCREEN_HEIGHT // 2
        
        # Smoothly move camera
        camera_pos.x += (target_cam_x - camera_pos.x) * 0.1
        
        # Vertical tracking with terminal velocity check
        if kitty.vel.y < TERMINAL_VEL_CAMERA_STOP:
            camera_pos.y += (target_cam_y - camera_pos.y) * 0.1
        
        # Clamp camera to world bounds
        camera_pos.x = max(0, min(camera_pos.x, WORLD_WIDTH - SCREEN_WIDTH))
        camera_pos.y = max(0, min(camera_pos.y, WORLD_HEIGHT - SCREEN_HEIGHT))

        # Life Lost
        if kitty.pos.y > WORLD_HEIGHT + 100:
            lives -= 1
            if lives > 0:
                pygame.time.delay(1000)
                restart_current_level(regenerate=True)
                continue
            else:
                game_over_screen()
                continue

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
        # Draw background with scrolling
        screen.blit(background_image, (-camera_pos.x, -camera_pos.y))

        # Leaves are independent of camera (screen-space)
        for leaf in leaves:
             screen.blit(leaf.image, leaf.rect)

        for entity in all_sprites:
            if isinstance(entity, Kitty):
                # Kitty is already moved in world space, we blit at screen space
                screen.blit(entity.image, (entity.pos.x - camera_pos.x - entity.rect.width//2, 
                                           entity.pos.y - camera_pos.y - entity.rect.height//2))
            else:
                screen.blit(entity.image, (entity.rect.x - camera_pos.x, entity.rect.y - camera_pos.y))

        for dog in dogs:
            screen.blit(dog.image, (dog.rect.x - camera_pos.x, dog.rect.y - camera_pos.y))

        for eagle in eagles:
            screen.blit(eagle.image, (eagle.rect.x - camera_pos.x, eagle.rect.y - camera_pos.y))

        # Draw Branches
        for branch in branches:
            branch.draw(screen, camera_pos.x, camera_pos.y)

        draw_hud()
        draw_sidebar()
        pygame.display.flip()

if __name__ == "__main__":
    splash_screen()
    main_game()
