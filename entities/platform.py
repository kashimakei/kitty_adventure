import random
import math
import pygame
from constants import *
from entities.animation import SpriteFrames, Clip, Animator

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

# Built from the frames sliced out of the dog sheets in assets/source/ (see tools/slice_sprites.py)
DOG_CLIPS = {
    "walk":    Clip(["sniff_1", "sniff_2", "sniff_3", "sniff_4"], fps=5),  # nose down, sniffing along
    "scratch": Clip(["scratch_0", "scratch_1", "scratch_2", "scratch_3", "scratch_2", "scratch_3",
                     "scratch_4", "scratch_5"], fps=6, loop=False),
    "snarl":   Clip(["menace_0", "menace_4", "menace_5", "menace_4"], fps=6),
    "run":     Clip([f"run_a_{i}" for i in range(4)] + [f"run_b_{i}" for i in range(4)], fps=16),
    "bark":    Clip(["bark_0", "bark_1", "bark_2", "bark_1"], fps=6),
    "prep":    Clip(["prep_1", "prep_2", "prep_3"], fps=10, loop=False),
    "rise":    Clip(["air_0", "air_1"], fps=8, loop=False),
    "soar":    Clip(["air_2"], fps=1),
    "drop":    Clip(["air_3", "air_4"], fps=8, loop=False),
    "land":    Clip(["land_0", "land_1", "land_3", "land_4"], fps=12, loop=False),
    "crash":   Clip(["land_0", "land_2", "land_2", "land_3", "land_4"], fps=8, loop=False),
}

class Dog(pygame.sprite.Sprite):
    """Lives on the ground: sniffs about, scratches, snarls at and chases Kitty on its level,
    barks up at Kitty overhead, and leaps onto branches low enough to reach."""
    frames = None  # loaded on first spawn, shared by every dog

    def __init__(self, ground, branches, kitty, x, speed_scale=1.0):
        super().__init__()
        if Dog.frames is None:
            Dog.frames = SpriteFrames("assets/dog")
        self.animator = Animator(Dog.frames, DOG_CLIPS, "walk")
        self.ground = ground
        self.branches = branches
        self.kitty = kitty
        self.speed_scale = speed_scale
        self.x = float(x)
        self.y = float(ground.rect.top)
        self.support = ground  # what the dog is standing on; None while airborne
        self.velocity_x = self.velocity_y = 0.0
        self.facing = random.choice([-1, 1])
        self.state = "walk"
        self.state_time = 0.0
        self.next_scratch = random.uniform(*DOG_SCRATCH_EVERY)
        self.branch_time = 0.0
        self.branch_stay = random.uniform(*DOG_BRANCH_STAY)
        self.jump_target = None
        self.jump_x = 0.0
        self.crashed = False
        self.rect = pygame.Rect((0, 0), DOG_HITBOX)
        self.rect.midbottom = (self.x, self.y)
        self.image = self.animator.frame(self.facing)[0]

    def _set_state(self, state):
        if state != self.state:
            self.state = state
            self.state_time = 0.0

    def shift(self, dx, dy):
        self.x += dx
        self.y += dy
        self.rect.move_ip(dx, dy)

    def update(self, dt):
        self.state_time += dt
        if self.support is None:
            self._fly(dt)
        else:
            self._think(dt)
        self.rect.midbottom = (round(self.x), round(self.y))
        self._animate(dt)

    # ------------------ On the ground or a branch -------------------
    def _think(self, dt):
        support = self.support
        surface = support.surface_under([self.x])
        if surface is None or getattr(support, "broken", False):
            # The branch snapped, or the dog stepped off its end
            self._take_off(0.0, 0.0, crashed=getattr(support, "broken", False))
            return
        self.y = surface
        if support is not self.ground:
            support.apply_load(self.x, DOG_WEIGHT)

        if self.state == "prep":
            if self.animator.finished:
                self._launch()
            return
        if self.state in ("land", "crash") and not self.animator.finished:
            return

        kitty = self.kitty.rect
        dx = kitty.centerx - self.x
        toward = 1 if dx > 0 else -1
        same_level = abs(kitty.bottom - self.y) < 45
        above = self.y - 500 < kitty.bottom < self.y - 45 and abs(dx) < DOG_BARK_RANGE
        below = kitty.bottom > self.y + 45 and abs(dx) < DOG_SIGHT
        lo, hi = support.walk_range()

        if same_level and abs(dx) < DOG_SIGHT:
            self.facing = toward
            if self.state not in ("snarl", "run"):
                self._set_state("snarl")
            elif self.state == "snarl" and self.state_time >= DOG_SNARL_TIME:
                self._set_state("run")
            if self.state == "run" and not self._move(DOG_CHARGE_SPEED, dt, lo, hi, turn=False):
                self._set_state("snarl")  # cornered at the end of the branch
            return
        if above:
            self.facing = toward
            self._set_state("bark")
            target = self._reachable(self.kitty.standing_on or self.kitty.hanging_on)
            if target is not None and self.state_time >= DOG_BARK_BEFORE_JUMP:
                tlo, thi = target.walk_range()
                self._prepare_jump(target, max(tlo, min(thi, kitty.centerx)))
            return
        if below and support is not self.ground:
            self.facing = toward
            self._prepare_jump(self.ground, self.x + toward * 70)
            return

        # Nothing to chase: wander, scratch, explore
        if self.state == "scratch":
            if self.animator.finished:
                self._set_state("walk")
                self.next_scratch = random.uniform(*DOG_SCRATCH_EVERY)
            return
        self.next_scratch -= dt
        if self.next_scratch <= 0:
            self._set_state("scratch")
            return
        self._set_state("walk")
        self._move(DOG_WALK_SPEED, dt, lo, hi, turn=True)
        if support is self.ground:
            overhead = [b for b in self.branches if self._reachable(b) is b and b.surface_under([self.x]) is not None]
            if overhead and random.random() < DOG_EXPLORE_CHANCE * dt:
                blo, bhi = overhead[0].walk_range()
                self._prepare_jump(overhead[0], max(blo, min(bhi, self.x + self.facing * 30)))
        else:
            self.branch_time += dt
            if self.branch_time > self.branch_stay:
                self._prepare_jump(self.ground, self.x + self.facing * 70)

    def _move(self, speed, dt, lo, hi, turn):
        """Trot along; returns False when stopped at the end of the walkable span."""
        self.x += self.facing * speed * self.speed_scale * dt
        if lo <= self.x <= hi:
            return True
        self.x = max(lo, min(hi, self.x))
        if turn:
            self.facing = 1 if self.x <= lo else -1
        return False

    def _reachable(self, branch):
        """`branch` if it's an intact branch above the dog and low enough to leap onto, else None.
        Height is measured from the ground, so dogs can't climb the tree branch by branch."""
        if branch is None or branch is self.support or branch not in self.branches or branch.broken:
            return None
        lo, hi = branch.walk_range()
        top = branch.surface_under([max(lo, min(hi, self.x))])
        if top is None or top >= self.y or self.ground.rect.top - top > DOG_JUMP_HEIGHT:
            return None
        return branch

    # ------------------ Jumping and falling -------------------
    def _prepare_jump(self, target, x):
        self.jump_target = target
        self.jump_x = x
        self.facing = 1 if x >= self.x else -1
        self._set_state("prep")

    def _launch(self):
        """Leap on an arc that clears the target's surface and comes down on it at jump_x."""
        target_y = self.jump_target.surface_under([self.jump_x])
        if target_y is None:
            self._set_state("walk")
            return
        rise = self.y - target_y                # positive when jumping up
        apex = max(rise, 0) + 30
        vy = -math.sqrt(2 * DOG_GRAVITY * apex)
        flight = -vy / DOG_GRAVITY + math.sqrt(2 * (apex - rise) / DOG_GRAVITY)
        vx = max(-300, min(300, (self.jump_x - self.x) / flight))
        self._take_off(vx, vy)

    def _take_off(self, vx, vy, crashed=False):
        self.support = None
        self.velocity_x, self.velocity_y = vx, vy
        self.crashed = crashed
        if crashed:
            self.jump_target = None
        self._set_state("air")

    def _fly(self, dt):
        lo, hi = self.ground.walk_range()
        previous_y = self.y
        self.velocity_y += DOG_GRAVITY * dt
        self.x = max(lo, min(hi, self.x + self.velocity_x * dt))
        self.y += self.velocity_y * dt
        if self.velocity_y <= 0:
            return
        for candidate in (self.jump_target, self.ground):
            if candidate is None:
                continue
            top = candidate.surface_under([self.x])
            if top is not None and previous_y <= top + 4 and self.y >= top:
                self.support = candidate
                self.y = top
                self.velocity_x = self.velocity_y = 0.0
                self.branch_time = 0.0
                self.branch_stay = random.uniform(*DOG_BRANCH_STAY)
                if candidate is not self.ground:
                    candidate.kick(self.x, LANDING_KICK * 8)
                self._set_state("crash" if self.crashed else "land")
                self.crashed = False
                self.jump_target = None
                return

    # ------------------ Drawing -------------------
    def _animate(self, dt):
        anim = self.animator
        if self.state == "air":
            if self.velocity_y < -150:
                anim.play("rise")
            elif self.velocity_y > 150:
                anim.play("drop")
            else:
                anim.play("soar")
        else:
            anim.play(self.state)
        anim.update(dt)
        self.image = anim.frame(self.facing)[0]

    def draw(self, screen):
        if self.rect.right < -80 or self.rect.left > SCREEN_WIDTH + 80 or self.rect.top > SCREEN_HEIGHT + 80:
            return
        image, (ax, ay) = self.animator.frame(self.facing)
        screen.blit(image, (self.rect.centerx - ax, self.rect.bottom - ay))

# Built from the frames sliced out of assets/source/eagle_sheet.jpg (see tools/slice_sprites.py)
EAGLE_CLIPS = {
    "flap":    Clip(["flap_0", "flap_1", "flap_2", "flap_3", "flap_4"], fps=10),
    "glide":   Clip(["dive_0", "dive_1"], fps=4),   # wings set, lining up the swoop
    "plunge":  Clip(["dive_2", "dive_3"], fps=8),   # steep dive
    "talons":  Clip(["talon_0", "talon_1", "talon_2", "talon_3", "talon_4"], fps=12, loop=False),
    "recover": Clip(["dive_4", "flap_0", "flap_1"], fps=8, loop=False),
}

class Eagle(pygame.sprite.Sprite):
    frames = None  # loaded on first spawn, shared by every eagle

    def __init__(self, kitty, speed_scale=1.0):
        super().__init__()
        if Eagle.frames is None:
            Eagle.frames = SpriteFrames("assets/eagle")
        self.animator = Animator(Eagle.frames, EAGLE_CLIPS, "flap")
        self.kitty = kitty
        self.speed_scale = speed_scale

        # Determine side to swoop in from
        self.side = random.choice(['left', 'right'])
        self.facing = 1 if self.side == 'left' else -1
        start_y = random.randint(100, SCREEN_HEIGHT // 2)
        self.x = -60.0 if self.side == 'left' else SCREEN_WIDTH + 60.0
        self.y = float(start_y)
        self.velocity_x = random.randint(150, 250) * speed_scale * self.facing
        # Drift toward Kitty's height while crossing
        self.velocity_y = (kitty.rect.y - start_y) / (SCREEN_WIDTH / abs(self.velocity_x))
        self.velocity_y = max(-100, min(100, self.velocity_y))

        self.state = "cruise"
        self.state_time = 0.0
        self.swooped = False
        self.rect = pygame.Rect((0, 0), EAGLE_HITBOX)
        self.rect.center = (self.x, self.y)
        self.image = self.animator.frame(self.facing)[0]

    def _set_state(self, state):
        self.state = state
        self.state_time = 0.0

    def shift(self, dx, dy):
        self.x += dx
        self.y += dy
        self.rect.center = (self.x, self.y)

    def update(self, dt):
        self.state_time += dt
        kx, ky = self.kitty.rect.center
        ahead = (kx - self.x) * self.facing  # how far in front of the eagle Kitty is

        if self.state == "cruise":
            if not self.swooped and 0 < ahead < EAGLE_SWOOP_RANGE and ky > self.y - 40:
                self._set_state("windup")
                self.swooped = True
                self.velocity_y = 0
        elif self.state == "windup":
            # Wings set, gliding level: the tell that a dive is coming
            if self.state_time >= EAGLE_WINDUP:
                self._set_state("swoop")
                speed = EAGLE_SWOOP_SPEED * self.speed_scale
                angle = math.atan2(ky - self.y, kx - self.x)
                self.velocity_x, self.velocity_y = math.cos(angle) * speed, math.sin(angle) * speed
        elif self.state == "swoop":
            # Bend gently toward Kitty; quick feet can still slip out from under the talons
            speed = math.hypot(self.velocity_x, self.velocity_y)
            heading = math.atan2(self.velocity_y, self.velocity_x)
            turn = (math.atan2(ky - self.y, kx - self.x) - heading + math.pi) % (2 * math.pi) - math.pi
            heading += max(-EAGLE_HOMING * dt, min(EAGLE_HOMING * dt, turn))
            self.velocity_x, self.velocity_y = math.cos(heading) * speed, math.sin(heading) * speed
            # Never double back
            if self.velocity_x * self.facing < 60:
                self.velocity_x = 60 * self.facing
            if ahead < -30 or self.state_time > EAGLE_SWOOP_TIME:
                self._set_state("recover")
                self.velocity_x = self.facing * max(abs(self.velocity_x), 200 * self.speed_scale)
                self.velocity_y = -160
        elif self.state == "recover":
            self.velocity_y = min(-40, self.velocity_y + 150 * dt)  # pull up, then level off
            if self.state_time > 0.6:
                self._set_state("cruise")

        self.x += self.velocity_x * dt
        self.y += self.velocity_y * dt
        self.rect.center = (self.x, self.y)
        self._animate(dt, kx, ky)

        if ((self.facing > 0 and self.rect.left > SCREEN_WIDTH + 80) or
                (self.facing < 0 and self.rect.right < -80) or
                self.rect.top > SCREEN_HEIGHT + 150):
            self.kill()

    def _animate(self, dt, kx, ky):
        anim = self.animator
        if self.state == "windup":
            anim.play("glide")
        elif self.state == "swoop":
            if self.velocity_y > 1.3 * abs(self.velocity_x):
                anim.play("plunge")
            elif math.hypot(kx - self.x, ky - self.y) < 170:
                anim.play("talons")
            elif anim.name != "talons":
                anim.play("glide")
        elif self.state == "recover":
            anim.play("recover")
        elif anim.name != "recover" or anim.finished:
            anim.play("flap")
        anim.update(dt)
        self.image = anim.frame(self.facing)[0]

    def draw(self, screen):
        image, (ax, ay) = self.animator.frame(self.facing)
        head_x = self.rect.centerx + EAGLE_HEAD_OFFSET[0] * self.facing
        head_y = self.rect.centery + EAGLE_HEAD_OFFSET[1]
        screen.blit(image, (head_x - ax, head_y - ay))
