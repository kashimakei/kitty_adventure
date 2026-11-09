
#!/usr/bin/python3
import pygame
import sys
import random
import math
from pygame import mixer
from pygame.locals import *

# Import modules
import argparse
from constants import *
from entities.kitty import Kitty
from entities.platform import Platform, Dog, Leaf, Eagle
from entities.tree import Tree
from systems.camera import CameraSystem
from level_utils import get_breeze_strength, difficulty_profile, generate_branches, tree_positions

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
# convert()/convert_alpha() match the display's pixel format once, instead of on every blit
branch_image = pygame.image.load('assets/branch.png').convert_alpha()

background_image = pygame.image.load('assets/Background_lvl1.png').convert()
# Wide enough to pan slowly behind the trees for the whole width of the world
background_width = int(SCREEN_WIDTH + (WORLD_WIDTH - SCREEN_WIDTH) * BACKGROUND_PARALLAX)
background_image = pygame.transform.scale(background_image, (background_width, 3 * SCREEN_HEIGHT))

splash_image = pygame.image.load('assets/Kitty_splash.png')
splash_image = pygame.transform.scale(splash_image, (SCREEN_WIDTH, SCREEN_HEIGHT //2))

kitty_mini = pygame.image.load("assets/kitty/sit_0.png").convert_alpha()
kitty_mini = pygame.transform.smoothscale_by(kitty_mini, 40 / kitty_mini.get_height())

# Load Sounds
hiss_sound = pygame.mixer.Sound('assets/hiss.wav')
crunch_sound = pygame.mixer.Sound('assets/crunch.wav')  # a branch snapping
meow_1 = pygame.mixer.Sound('assets/meow.wav')
meow_2 = pygame.mixer.Sound('assets/meow2.wav')
meow_sounds = [meow_1, meow_2]

mixer.music.load('assets/BGM.ogg')
mixer.music.play(-1)  # Loop music

# Game State Variables
max_levels = 10
current_level = 9
current_difficulty = 2
level_complete = False
lives = 3
time_elapsed = 0

camera_sys = CameraSystem()

# Sprite Groups
platforms = pygame.sprite.Group()  # just the ground now; branches live on the trees
leaves = pygame.sprite.Group()
dogs = pygame.sprite.Group()
eagles = pygame.sprite.Group()

# Trees and the bendy branches growing out of them
trees = []
branches = []

# Other State
profile = difficulty_profile(current_level, current_difficulty)
next_dog_spawn_time = 0
next_eagle_spawn_time = 0

kitty = None # Will be initialized in setup/start

# ------------------ Helper Functions -------------------

def create_ground():
    left, _ = camera_sys.world_bounds()
    ground = Platform((left, SCREEN_HEIGHT - 10, WORLD_WIDTH, 10), branch_image)
    ground.is_ground = True
    return ground

def grow_trees(branch_layout):
    """Plant the trees tall enough to hold every branch, then grow the branches on them."""
    top_y = min(b["y"] for b in branch_layout) - 150
    left, _ = camera_sys.world_bounds()
    new_trees = [Tree(left + x, top_y, SCREEN_HEIGHT) for x in tree_positions(WORLD_WIDTH)]
    new_branches = []
    for spec in branch_layout:
        branch = new_trees[spec["tree"]].grow_branch(
            spec["y"], spec["direction"], spec["length"], spec["stiffness"], spec["gnarl"],
            spec["thickness"], spec["strength"]
        )
        branch.is_final = spec["final"]
        new_branches.append(branch)
    return new_trees, new_branches

def supports():
    """Everything Kitty can stand on."""
    return branches + platforms.sprites()

def world_objects():
    """Everything the camera scrolls."""
    return trees + branches + platforms.sprites() + dogs.sprites()

def initialize_leaves():
    leaves.empty()
    for _ in range(MAX_LEAVES):
        x = random.randint(0, SCREEN_WIDTH)
        y = random.randint(-SCREEN_HEIGHT, 0)
        leaf = Leaf(x, y)
        leaves.add(leaf)

def spawn_dog():
    """A dog trots in somewhere on the ground that's off-screen and well away from Kitty."""
    if len(dogs) >= profile["max_dogs"]:
        return
    ground = next(p for p in platforms if p.is_ground)
    left, right = ground.walk_range()
    spots = [x for x in (random.uniform(left, right) for _ in range(12))
             if (x < -60 or x > SCREEN_WIDTH + 60) and abs(x - kitty.rect.centerx) > 300]
    if spots:
        dogs.add(Dog(ground, branches, kitty, spots[0], profile["dog_speed"]))

def spawn_eagle_logic():
    global next_eagle_spawn_time
    
    # Cap concurrent eagles to avoid chaos; later levels allow a second
    if len(eagles) >= profile["max_eagles"]:
        return

    eagle = Eagle(kitty, profile["eagle_speed"])
    eagles.add(eagle)
    
    # Schedule next spawn, sooner on later levels
    wait_time = random.randint(*profile["eagle_interval_ms"])
    next_eagle_spawn_time = pygame.time.get_ticks() + wait_time

def restart_current_level(regenerate=False):
    global level_complete, next_dog_spawn_time, next_eagle_spawn_time, kitty, current_level, lives, trees, branches, profile

    camera_sys.reset()
    level_complete = False

    max_plats = 11 if current_level == 1 else 10
    profile = difficulty_profile(current_level, current_difficulty)

    if regenerate:
        platforms.empty()
        dogs.empty()
        eagles.empty()
        
        branch_layout = generate_branches(
            max_plats, WORLD_WIDTH, SCREEN_HEIGHT,
            level=current_level, difficulty=current_difficulty
        )
        trees, branches = grow_trees(branch_layout)
            
        ground = create_ground()
        platforms.add(ground)
        
    ground = next(p for p in platforms if p.is_ground)
    kitty.respawn((SCREEN_WIDTH // 2, ground.rect.top), ground)

    # Reset spawn time
    base = profile["dog_interval_ms"]
    now = pygame.time.get_ticks()
    next_dog_spawn_time = now + random.randint(int(base*0.5), int(base*1.5))
    next_eagle_spawn_time = now + random.randint(*profile["eagle_interval_ms"])

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
    draw_minimap()

def draw_minimap():
    """Strip across the HUD: the whole world's width, what's on screen, Kitty, and the goal branch."""
    bar = pygame.Rect(150, 20, 290, 12)
    scale = bar.width / WORLD_WIDTH
    to_map = lambda screen_x: bar.left + (screen_x + camera_sys.offset_x) * scale
    pygame.draw.rect(screen, (90, 70, 50), bar, border_radius=4)
    view = pygame.Rect(to_map(0), bar.top, SCREEN_WIDTH * scale, bar.height)
    pygame.draw.rect(screen, (200, 230, 255), view, border_radius=4)
    for branch in branches:
        if branch.is_final:
            tip_x = branch.branch_struct.top_contour()[-1][0]
            pygame.draw.circle(screen, (218, 165, 32), (int(to_map(tip_x)), bar.centery), 7)
    pygame.draw.circle(screen, (230, 90, 40), (int(to_map(kitty.rect.centerx)), bar.centery), 5)

# ------------------ Main Game Loop ----------------------------
def main_game():
    global time_elapsed, level_complete, lives, next_dog_spawn_time, next_eagle_spawn_time, kitty
    
    # Init Kitty
    kitty = Kitty(meow_sounds)
    
    # Init Level 1
    restart_current_level(regenerate=True)
    initialize_leaves()
    
    running = True
    while running:
        # Clamp so the frame after a pause (level complete, lost life) can't fling the branches
        dt = min(clock.tick(FPS) / 1000.0, MAX_FRAME_DT)
        time_elapsed += dt
        
        breeze_strength = get_breeze_strength(time_elapsed, profile["wind"])
        
        for event in pygame.event.get():
            if event.type == QUIT:
                pygame.quit()
                sys.exit()
            elif event.type == KEYDOWN:
                if event.key == K_SPACE:
                    kitty.do_jump()  # also hauls Kitty up when hanging from a branch
                elif event.key == K_UP:
                    kitty.climb_up()
                elif event.key == K_DOWN:
                    kitty.let_go()
            elif event.type == KEYUP:
                if event.key == K_SPACE:
                    kitty.stop_jump()

        pressed_keys = pygame.key.get_pressed()
        kitty.update(pressed_keys, breeze_strength, camera_sys.world_bounds())

        # Kitty rides (and weighs down) the branch underfoot, or lands on one
        landed_on = kitty.resolve_supports(supports())
        kitty.animate(dt)

        # Update platforms
        for platform in platforms:
            platform.update(breeze_strength, time_elapsed)
        for branch in branches:
            branch.step(dt, breeze_strength, time_elapsed)
            if branch.just_snapped:
                crunch_sound.play()
            
        leaves.update(breeze_strength, dt)
        
        # Camera
        camera_sys.update(kitty, world_objects(), leaves, eagles)

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

        # Check level complete
        if landed_on is not None and landed_on.is_final and not level_complete:
            level_complete = True
            print("Level complete!")
            screen.blit(congrats_text, ((SCREEN_WIDTH - congrats_text.get_width()) // 2, SCREEN_HEIGHT // 2))
            pygame.display.flip()
            pygame.time.delay(2000)
            start_new_level()
            continue

        # Dogs
        for dog in list(dogs):
            dog.update(dt)
            
        # Eagles
        for eagle in list(eagles):
            eagle.update(dt)

        # Spawn dogs
        if current_level >= profile["dog_start_level"]:
            now = pygame.time.get_ticks()
            if now >= next_dog_spawn_time:
                spawn_dog()
                base = profile["dog_interval_ms"]
                next_dog_spawn_time = now + random.randint(int(base * 0.5), int(base * 1.5))

        # Spawn eagles
        if current_level >= profile["eagle_start_level"]:
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
        background_x = -camera_sys.offset_x * BACKGROUND_PARALLAX
        background_y = -1.5 * SCREEN_HEIGHT + (camera_sys.offset % (3 * SCREEN_HEIGHT))
        screen.blit(background_image, (background_x, background_y))
        screen.blit(background_image, (background_x, background_y + 3 * SCREEN_HEIGHT))

        # Branches first so their roots disappear into the trunks
        for branch in branches:
            branch.draw(screen)
        for tree in trees:
            tree.draw(screen)

        leaves.draw(screen)

        for platform in platforms:
            if platform.rect.top < SCREEN_HEIGHT:
                screen.blit(platform.image, platform.rect)

        for dog in dogs:
            dog.draw(screen)

        kitty.draw(screen)

        for eagle in eagles:
            eagle.draw(screen)

        draw_hud()
        pygame.display.flip()


# ------------------ Test Mode ----------------------------
def test_mode():
    pygame.init()
    screen = pygame.display.set_mode((SCREEN_WIDTH, SCREEN_HEIGHT))
    pygame.display.set_caption("Branch Physics Test Mode")
    clock = pygame.time.Clock()

    from entities.branch import BranchWrapper
    from entities.ball import BallWrapper
    from systems.physics_system import PhysicsSystem
    
    physics = PhysicsSystem(gravity=500.0, global_damping=0.99)
    branches = []
    
    branches.append(BranchWrapper(x_pos=0, y_pos=200, length=250, segments=12, stiffness=0.6, gnarl=0.5))
    branches.append(BranchWrapper(x_pos=SCREEN_WIDTH - 200, y_pos=400, length=250, segments=12, stiffness=0.9, gnarl=1.5))
    
    balls = []
    
    running = True
    time_elapsed = 0.0
    last_spawn_time = 0.0
    
    while running:
        dt = clock.tick(60) / 1000.0
        time_elapsed += dt
        
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.MOUSEBUTTONDOWN:
                mx, my = pygame.mouse.get_pos()
                balls.append(BallWrapper(mx, my))

        if time_elapsed - last_spawn_time > 2.0:
            import random
            balls.append(BallWrapper(random.randint(50, SCREEN_WIDTH - 50), -50))
            last_spawn_time = time_elapsed
            
        balls = [b for b in balls if b.ball_struct.pos.y < SCREEN_HEIGHT + 200]
        
        physics.step_balls(balls, dt)
        physics.step_branches(branches, sub_steps=4, dt=dt)
        physics.resolve_collisions(balls, branches)
        
        screen.fill(SKY_BLUE)
        for b in branches:
            b.render(screen, 0)
        for b in balls:
            b.render(screen, 0)
            
        for i in range(lives):
            pass # Keep it clean
            
        test_hud = pygame.font.SysFont(None, 35).render("Test Mode: Click to drop balls", True, BLACK)
        screen.blit(test_hud, (10, 10))
            
        pygame.display.flip()

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Kitty Adventure")
    parser.add_argument("--test", action="store_true", help="Launch test mode")
    args = parser.parse_args()

    if args.test:
        test_mode()
    else:
        splash_screen()
        main_game()
