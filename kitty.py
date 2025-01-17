#!/usr/bin/python3
import pygame
import sys
import random
import math
from pygame.locals import *
from pygame import mixer

def generate_platforms(max_platforms, screen_width, screen_height):
    platforms = []
    max_jump_height = 150  # Maximum vertical distance Kitty can jump
    min_vertical_gap = 120  # Minimum vertical distance between platforms
    max_vertical_gap = max_jump_height - 20  # Keeping it within jump reach
    platform_width_range = (100, 200)  # Platform width variability
    platform_height = 20  # Consistent platform thickness
    
    last_platform_y = screen_height  # Starting from the bottom
    
    for _ in range(max_platforms):
        width = random.randint(*platform_width_range)
        vertical_gap = random.randint(min_vertical_gap, max_vertical_gap)
        
        # Decide whether the platform extends from left or right
        if random.choice([True, False]):
            # Extends from the left
            x_position = 0
        else:
            # Extends from the right, ensure it's fully visible
            x_position = screen_width - width
        
        y_position = last_platform_y - vertical_gap  # Move up for the next platform
        last_platform_y = y_position  # Update for next iteration
        
        platform = (x_position, y_position, width, platform_height)
        platforms.append(platform)
 
    final_platform = platforms[100]  # This assumes the last generated platform is the highest
    return platforms, final_platform
 

# Initialize Pygame
pygame.init()
# Constants
SCREEN_WIDTH, SCREEN_HEIGHT = 600, 1200
FPS = 60

platform_data, final_platform_data = generate_platforms(101, SCREEN_WIDTH, SCREEN_HEIGHT)  # Generate platform data

# Colors
BLACK = (0, 0, 0)
SKY_BLUE = (145, 220, 255) 
WHITE = (255, 255, 255)

font = pygame.font.SysFont(None, 55)
congrats_text = font.render('Congratulations Kitty!', True, (0, 128, 0))  # Green text

# Load sound effects
hiss_sound = pygame.mixer.Sound('assets/hiss.wav')
meow_1 = pygame.mixer.Sound('assets/meow.wav')
meow_2 = pygame.mixer.Sound('assets/meow2.wav')
die_sound_duration = 2500  # Duration in milliseconds
mixer.init()
mixer.music.load('assets/BGM.ogg')
mixer.music.play()
# Setup the screen
screen = pygame.display.set_mode((SCREEN_WIDTH, SCREEN_HEIGHT))
pygame.display.set_caption("Kitty Adventure")
clock = pygame.time.Clock()
camera_offset = 0  # Initialize the camera offset
camera_follow_kitty = False  # A flag to determine when the camera should follow Kitty

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
        self.max_jump_duration = 0.5  # Maximum duration of the jump in seconds
        self.gravity = 0.5
        self.upward_acceleration = -1  # Acceleration while the jump key is held
        self.previous_rect = self.rect.copy()
        self.meow_sounds =[meow_1,meow_2]
        self.meow_index = 0

    def check_falling(self, platforms):
        # Temporarily increase the rectangle to check for platforms below
        self.rect.y += 1
        if not pygame.sprite.spritecollideany(self, platforms):
            # If there are no platform collisions, Kitty is in the air and should fall
            if not self.falling:
                self.falling = True
                self.velocity = 0
        self.rect.y -= 1  # Reset the rectangle position
        print("falling")

    def update(self, pressed_keys):
        self.previous_rect = self.rect.copy()
        # Movement controls
        if pressed_keys[pygame.K_LEFT]:
            self.rect.move_ip(-self.speed, 0)
        if pressed_keys[pygame.K_RIGHT]:
            self.rect.move_ip(self.speed, 0)

        # Update velocity based on jumping and falling
        if self.jump:
            jump_time = (pygame.time.get_ticks() - self.jump_start_time) / 1000.0  # Convert to seconds
            if jump_time < self.max_jump_duration:
                # Jumping, move up
                self.velocity += self.upward_acceleration
            else:
                # Maximum jump reached, start falling
                self.falling = True
                self.jump = False
                self.velocity = 0

        if self.falling:
            # Falling, move down
            self.velocity += self.gravity
#            self.velocity = min(self.velocity, self.max_jump_velocity)

        # Apply the updated velocity
        self.rect.move_ip(0, self.velocity)

#        # If Kitty is falling and has reached the ground, stop falling
#        if self.falling and self.rect.bottom >= SCREEN_HEIGHT - 10:
#            self.jump = False
#            self.falling = False
#            self.rect.bottom = SCREEN_HEIGHT - 10
#            self.velocity = 0  # Reset velocity

        # Check if Kitty is on a platform or should start falling
        if not self.jump:
            self.check_falling(platforms)


    def do_jump(self):
        if not self.jump and not self.falling:
            self.jump = True
            self.jump_start_time = pygame.time.get_ticks()  # Start time of the jump
#            self.velocity += self.upward_acceleration
            # Play sound
            self.meow_sounds[self.meow_index].play()
            self.meow_index = (self.meow_index +1) % len(self.meow_sounds)
            

    def stop_jump(self):
        if self.jump:
            # If we're moving up, stop the upward movement and start falling
            self.velocity = self.gravity
            self.falling = True
            self.jump = False


class Platform(pygame.sprite.Sprite):
    def __init__(self, platform_data):
        super().__init__()
        x, y, width, height = platform_data
        # Load branch image and scale it
        self.image = pygame.image.load('assets/branch.png').convert_alpha()
        self.image = pygame.transform.scale(self.image, (width, height))
        self.rect = self.image.get_rect(topleft=(x, y))
        
        # Create collision rect (slightly smaller than visual for better gameplay)
        self.collision_rect = pygame.Rect(x + 10, y + 5, width - 20, height - 5)
        
        # Animation properties
        self.angle = 0
        self.amplitude = 5  # Max sway distance
        self.speed = 0.05  # Sway speed
        self.original_x = x
        self.original_y = y
        
    def update(self):
        # Swaying motion using sine wave
        self.angle += self.speed
        sway = self.amplitude * math.sin(self.angle)
        self.rect.x = self.original_x + sway
        self.rect.y = self.original_y + abs(sway * 0.2)  # Slight vertical movement
        
        # Update collision rect position
        self.collision_rect.x = self.rect.x + 10
        self.collision_rect.y = self.rect.y + 5

# Create platform sprites based on the generated data
platforms = pygame.sprite.Group()
for plat_data in platform_data:
    platforms.add(Platform(plat_data))

# Create Ground platform and add it to the platforms group
    ground = Platform((0, SCREEN_HEIGHT - 10, SCREEN_WIDTH, 10))
    platforms.add(ground)

def start_new_level():
    global platforms, final_platform_data, camera_offset, level_complete
    # Generate new platforms for the next level
    platform_data, temp_final_platform = generate_platforms(10, SCREEN_WIDTH, SCREEN_HEIGHT)
    final_platform_data = (temp_final_platform[0], temp_final_platform[1] + camera_offset, temp_final_platform[2], temp_final_platform[3])
    platforms.empty()  # Clear old platforms
    for plat_data in platform_data:
        platforms.add(Platform(plat_data))
    camera_offset = 0  # Reset camera offset for the new level
    kitty.rect.midbottom = (SCREEN_WIDTH // 2, SCREEN_HEIGHT - 50)  # Reset Kitty's position
    level_complete = False  # Reset level complete flag

level_complete = False
# Game loop
kitty = Kitty()
#platforms = pygame.sprite.Group()

# Add the kitty and platforms to a group
all_sprites = pygame.sprite.Group()
all_sprites.add(kitty)
for platform in platforms:  # Add each platform individually to the all_sprites group
    all_sprites.add(platform)

while True:
    for event in pygame.event.get():
        if event.type == pygame.QUIT:
            pygame.quit()
            sys.exit()
        elif event.type == pygame.KEYDOWN:
            if event.key == pygame.K_SPACE:
                kitty.do_jump()
        elif event.type == pygame.KEYUP:
            if event.key == pygame.K_SPACE:
                kitty.stop_jump()  # Call stop_jump when the space key is released

    pressed_keys = pygame.key.get_pressed()
    kitty.update(pressed_keys)
    platforms.update()  # Update platform animations

    if kitty.jump and kitty.rect.top <= SCREEN_HEIGHT / 2:
        # Start moving the camera with Kitty only when she's jumping and reaches half the screen height
        camera_follow_kitty = True
    
    if camera_follow_kitty and not kitty.falling:
        # Calculate how much to move the camera. We only move it when Kitty is going up.
        camera_movement = max(0, (SCREEN_HEIGHT / 2) - kitty.rect.top)
        camera_offset += camera_movement
        # Adjust Kitty's position relative to the camera
        kitty.rect.y += camera_movement  
        
        # Move all platforms down to simulate camera movement up
        for platform in platforms:
            platform.rect.y += camera_movement

    # Reset the camera follow flag when Kitty is falling
    if kitty.falling:
        camera_follow_kitty = False

    # End the game if Kitty falls below the screen
    if kitty.rect.top > SCREEN_HEIGHT:
        print("Game Over!")  # Later on, this is where you would handle lives and reset the level
        break

    # Drawing the scene with the camera offset
    screen.fill(SKY_BLUE)
    for entity in all_sprites:
        # Check if the entity is an instance of Kitty and blit the image attribute
        if isinstance(entity, Kitty):
            screen.blit(entity.image, entity.rect)
        else:
            # For other entities (like platforms), use image
            if entity.rect.top > -camera_offset:  # Don't draw platforms that have moved above the screen
                screen.blit(entity.image, entity.rect)


    # Check for collisions between Kitty and platforms
    collisions = pygame.sprite.spritecollide(kitty, platforms, False)
    for platform in collisions:
        if kitty.velocity > 0:  # Kitty is moving down
            if kitty.previous_rect.bottom <= platform.rect.top and kitty.rect.bottom >= platform.rect.top:
                # Landing on platform
                kitty.falling = False
                kitty.velocity = 0
                kitty.rect.bottom = platform.rect.top  # Now kitty will land on the platform
                break  # Exit the loop once we have processed landing on a platform

    # Check for level completion
    if not level_complete and kitty.rect.colliderect(Platform((final_platform_data[0], final_platform_data[1] + camera_offset, final_platform_data[2], final_platform_data[3])).rect):
        print("Level complete")
        level_complete = True
        screen.blit(congrats_text, (100, SCREEN_HEIGHT // 2))  # Display congratulations message
        pygame.display.flip()
        pygame.time.delay(2000)  # Give time for the player to read the message

        start_new_level()  # Call this to reset the game for a new level

    pygame.display.flip()
    clock.tick(FPS)

