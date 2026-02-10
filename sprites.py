import random
import math
import pygame
from pygame.math import Vector2
from constants import *

class VerletNode:
    def __init__(self, pos, is_fixed=False):
        self.pos = Vector2(pos)
        self.prev_pos = Vector2(pos)
        self.acc = Vector2(0, 0)
        self.is_fixed = is_fixed

    def update(self, dt, gravity, damping):
        if self.is_fixed:
            return
        
        vel = (self.pos - self.prev_pos) * damping
        self.prev_pos = Vector2(self.pos)
        self.pos += vel + self.acc * (dt * dt)
        self.acc = Vector2(0, 0)

    def apply_force(self, force):
        if not self.is_fixed:
            self.acc += force

class TrussBranch(pygame.sprite.Sprite):
    def __init__(self, anchor_pos, length, num_segments, base_thickness, tip_thickness, angle_deg=0):
        super().__init__()
        self.anchor_pos = Vector2(anchor_pos)
        self.length = length
        self.num_segments = num_segments
        self.base_thickness = base_thickness
        self.tip_thickness = tip_thickness
        self.angle = math.radians(angle_deg)
        
        self.nodes = []
        self.constraints = [] # List of (node_a, node_b, target_dist)
        
        self._setup_structure()
        
        # Sprite requirements
        self.image = pygame.Surface((WORLD_WIDTH, WORLD_HEIGHT), pygame.SRCALPHA) # Large enough to cover world for simplicity
        self.rect = self.image.get_rect()

    def _setup_structure(self):
        segment_len = self.length / self.num_segments
        dir_vec = Vector2(math.cos(self.angle), math.sin(self.angle))
        perp_vec = Vector2(-dir_vec.y, dir_vec.x)
        
        top_rail = []
        bottom_rail = []
        
        for i in range(self.num_segments + 1):
            t = i / self.num_segments
            thickness = self.base_thickness * (1 - t) + self.tip_thickness * t
            
            p_base = self.anchor_pos + dir_vec * (i * segment_len)
            p_top = p_base - perp_vec * (thickness / 2)
            p_bottom = p_base + perp_vec * (thickness / 2)
            
            is_fixed = (i == 0)
            node_top = VerletNode(p_top, is_fixed)
            node_bottom = VerletNode(p_bottom, is_fixed)
            
            self.nodes.extend([node_top, node_bottom])
            top_rail.append(node_top)
            bottom_rail.append(node_bottom)
            
            # Cross brace at this segment (vertical)
            self.constraints.append((node_top, node_bottom, thickness))
            
            # Longitudinal constraints
            if i > 0:
                # Top rail
                d_top = (top_rail[i].pos - top_rail[i-1].pos).length()
                self.constraints.append((top_rail[i], top_rail[i-1], d_top))
                # Bottom rail
                d_bot = (bottom_rail[i].pos - bottom_rail[i-1].pos).length()
                self.constraints.append((bottom_rail[i], bottom_rail[i-1], d_bot))
                # Diagonal braces
                d_diag1 = (top_rail[i].pos - bottom_rail[i-1].pos).length()
                self.constraints.append((top_rail[i], bottom_rail[i-1], d_diag1))
                d_diag2 = (bottom_rail[i].pos - top_rail[i-1].pos).length()
                self.constraints.append((bottom_rail[i], top_rail[i-1], d_diag2))
        
        self.top_rail = top_rail
        self.bottom_rail = bottom_rail

    def update(self, dt, gravity, damping, substeps, stiffness):
        # Scale dt for substeps
        sdt = dt / substeps
        
        for _ in range(substeps):
            for node in self.nodes:
                node.apply_force(Vector2(0, gravity * 100)) # Scale gravity for simulation
                node.update(sdt, gravity, damping)
            
            # Solve constraints
            # Higher stiffness = more iterations or higher correction factor
            iterations = int(stiffness * 10) # Simple mapping
            for _ in range(max(1, iterations)):
                for a, b, dist in self.constraints:
                    delta = b.pos - a.pos
                    curr_dist = delta.length()
                    if curr_dist == 0: continue
                    diff = (curr_dist - dist) / curr_dist
                    correction = delta * 0.5 * diff * stiffness # Scale correction by stiffness too
                    
                    if not a.is_fixed: a.pos += correction
                    if not b.is_fixed: b.pos -= correction

    def draw(self, surface, camera_offset_x, camera_offset_y):
        # Draw the "skin" of the branch (rendered as a thick solid branch)
        # Use top rail nodes to define the main branch shape
        points = []
        for node in self.top_rail:
            points.append((int(node.pos.x - camera_offset_x), int(node.pos.y - camera_offset_y)))
        
        # Draw a thick line for the main branch
        if len(points) > 1:
            # We draw a series of thick segments to simulate the tapering branch
            for i in range(len(points) - 1):
                # Calculate thickness at this segment
                t_ratio = i / (len(points) - 1)
                thickness = int(self.base_thickness * (1 - t_ratio) + self.tip_thickness * t_ratio)
                color = (139, 69, 19) # Brown
                pygame.draw.line(surface, color, points[i], points[i+1], max(1, thickness))
        
        # Fill rail surfaces for visual "thickness" if needed, but manual lines suffice for now

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
        self.rect = self.image.get_rect(center=(self.x, self.y))
        
        # Reset if leaf goes off screen
        # Screen space decorative leaves
        if (self.rect.right < -50 or self.rect.left > SCREEN_WIDTH + 50 or
            self.rect.top > SCREEN_HEIGHT + 50):
            self.reset()

    def reset(self):
        # Reset leaf to top of screen
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
    def __init__(self, meow_sounds, mass, restitution):
        super().__init__()
        img = pygame.image.load('assets/kitty.png').convert_alpha()
        self.original_image = pygame.transform.scale(img, (100, 100))
        self.flipped_image = pygame.transform.flip(self.original_image, True, False)
        self.image = self.original_image
        self.rect = self.image.get_rect(center=(SCREEN_WIDTH // 2, SCREEN_HEIGHT - 50))
        
        # New Vector-based physics
        self.pos = Vector2(self.rect.center)
        self.vel = Vector2(0, 0)
        self.mass = mass
        self.restitution = restitution
        
        self.jump = False
        self.falling = False
        self.jump_start_time = None
        self.max_jump_duration = 0.5  # seconds
        self.meow_sounds = meow_sounds
        self.meow_index = 0

    def update(self, pressed_keys, branches, platforms, breeze_strength, dt, gravity):
        # Apply horizontal input
        move_speed = 300 # pixels per second
        if pressed_keys[pygame.K_LEFT]:
            self.vel.x = -move_speed
            self.image = self.flipped_image
        elif pressed_keys[pygame.K_RIGHT]:
            self.vel.x = move_speed
            self.image = self.original_image
        else:
            self.vel.x *= 0.8 # Friction/Damping
            if abs(self.vel.x) < 1: self.vel.x = 0

        # Apply gravity
        self.vel.y += gravity * 1000 * dt
        
        # Apply wind push
        if self.jump or self.falling:
            self.vel.x += breeze_strength * 10 * dt

        # Update position
        self.pos += self.vel * dt
        
        # Collision with Truss Branches
        self.handle_branch_collisions(branches)
        # Collision with Static Platforms (like Ground)
        self.handle_platform_collisions(platforms)

        # Keep Kitty inside world bounds (horizontal)
        if self.pos.x < self.rect.width / 2:
            self.pos.x = self.rect.width / 2
            self.vel.x = 0
        if self.pos.x > WORLD_WIDTH - self.rect.width / 2:
            self.pos.x = WORLD_WIDTH - self.rect.width / 2
            self.vel.x = 0

        # Update rect position
        self.rect.center = (int(self.pos.x), int(self.pos.y))

    def handle_branch_collisions(self, branches):
        for branch in branches:
            # Check against top rail segments
            for i in range(len(branch.top_rail) - 1):
                node1 = branch.top_rail[i]
                node2 = branch.top_rail[i+1]
                p1 = node1.pos
                p2 = node2.pos
                
                # Collision check using point-to-line segment distance
                ab = p2 - p1
                ap = self.pos - p1
                
                # Projection
                t = ap.dot(ab) / ab.length_squared() if ab.length_squared() > 0 else 0
                t_clamped = max(0.0, min(1.0, t))
                nearest = p1 + (ab * t_clamped)
                
                dist_vec = self.pos - nearest
                dist = dist_vec.length()
                
                radius = self.rect.height / 2 * 0.8 # Effective radius for collision
                
                if dist < radius:
                    # We are colliding. 
                    # Only resolve if we are moving TOWARDS the segment (or falling onto it)
                    normal = dist_vec.normalize() if dist > 0 else Vector2(0, -1)
                    
                    if self.vel.dot(normal) < 0 or dist < radius: # Resolve even if resting
                        # Correct position
                        overlap = radius - dist
                        
                        # Apply reaction force/displacement to branch nodes
                        # We distribute the displacement based on where we hit the segment (t)
                        # And we scale it by a "mass factor"
                        mass_factor = self.mass * 0.5 # Adjustable weight impact
                        
                        if not node1.is_fixed:
                            node1.pos -= normal * overlap * mass_factor * (1 - t_clamped)
                        if not node2.is_fixed:
                            node2.pos -= normal * overlap * mass_factor * t_clamped
                            
                        # Move Kitty (less than before if branch moved)
                        self.pos += normal * overlap * (1.0 - mass_factor)
                        
                        # Reflect velocity
                        self.vel = self.vel.reflect(normal) * self.restitution
                        self.falling = False # Stop falling state if we hit a branch
                        self.jump = False

    def handle_platform_collisions(self, platforms):
        for p in platforms:
            if self.rect.colliderect(p.rect):
                # Check if falling onto platform
                if self.vel.y > 0 and self.pos.y < p.rect.bottom:
                    self.pos.y = p.rect.top - self.rect.height / 2 + 1
                    self.vel.y = 0
                    self.falling = False

    def do_jump(self):
        # We need a ground check or branch contact check to jump
        # For now, let's allow jumping if moving upwards or if we just hit a branch
        if not self.falling:
            self.vel.y = -600 # Initial jump velocity
            self.falling = True
            self.meow_sounds[self.meow_index].play()
            self.meow_index = (self.meow_index + 1) % len(self.meow_sounds)

    def stop_jump(self):
        if self.vel.y < -300:
            self.vel.y = -300 # Cap jump

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
