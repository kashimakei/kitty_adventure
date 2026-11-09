import random
import pygame
from constants import *
from entities.animation import SpriteFrames, Clip, Animator

# Kitty's moves, built from the frames sliced out of assets/source/ (see tools/slice_sprites.py)
KITTY_CLIPS = {
    "sit":   Clip(["sit_0"], fps=1),
    "walk":  Clip(["walk_0", "walk_1", "walk_2", "walk_3"], fps=10),
    "run_start": Clip(["run_start_0"], fps=8, loop=False),
    "run":   Clip(["run_0", "run_1", "run_2"], fps=12),
    # Crouch, push off, tuck: the crouch flashes by so the jump still leaves instantly
    "rise":  Clip(["jump_prep_0", "jump_prep_1", "mid_jump_0"], fps=14, loop=False),
    "apex":  Clip(["mid_jump_1"], fps=1),
    "fall":  Clip(["mid_jump_2"], fps=1),
    "land":  Clip(["land_0", "land_1", "land_2"], fps=14, loop=False),
    "panic": Clip(["dangle_0", "dangle_1"], fps=6),
    "groom": Clip(["clean_0", "clean_1", "clean_2", "clean_1", "clean_2", "groom_1", "groom_1", "sit_alert_0"],
                  fps=5, loop=False),
    "wave":  Clip(["wave_0", "wave_0", "wave_0", "sit_0"], fps=3, loop=False),
    "sleep": Clip(["sleep_0", "sleep_1"], fps=1.2),
    # Catching a branch: reach, then swing from one paw
    "grab":  Clip(["hang_0"], fps=5, loop=False, pin="grip"),
    "hang":  Clip(["hang_1", "hang_2", "hang_3", "hang_2"], fps=4, pin="grip"),
    "let_go": Clip(["let_go_0", "let_go_1"], fps=6, loop=False),
    "climb": Clip(["climb_0", "climb_1", "climb_2", "climb_3", "climb_4"], fps=5 * 60 / CLIMB_FRAMES, loop=False),
    "climb_done": Clip(["climb_5"], fps=3, loop=False),
}
KITTY_HITBOX = (100, 100)
RUN_AFTER = 0.6        # seconds of walking before Kitty breaks into a run
SLEEP_AFTER = 15.0     # seconds sitting still before Kitty curls up
PANIC_SPEED = 20       # px/frame; falling faster than this means a long drop
FIDGET_DELAY = (3.0, 7.0)  # seconds between grooming or waving while sitting

class Kitty(pygame.sprite.Sprite):
    def __init__(self, meow_sounds):
        super().__init__()
        self.animator = Animator(SpriteFrames("assets/kitty"), KITTY_CLIPS, "sit")
        self.facing = 1
        self.walk_dir = 0
        self.move_time = 0.0
        self.idle_time = 0.0
        self.next_fidget = random.uniform(*FIDGET_DELAY)
        self.just_landed = False
        self.panicking = False  # set when the branch underfoot snaps
        self.hanging_on = None  # branch Kitty is dangling from by the paws
        self.climbing_on = None  # branch Kitty is hauling up onto
        self.climb_frame = 0
        self.just_grabbed = False
        self.just_climbed = False
        self.released_from = None  # branch just let go of, briefly not catchable
        self.released_time = 0.0
        self.image = self.animator.frame()[0]
        self.rect = pygame.Rect((0, 0), KITTY_HITBOX)
        self.rect.center = (SCREEN_WIDTH // 2, SCREEN_HEIGHT - 50)
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
        self.standing_on = None  # the branch (or ground) under Kitty's paws
        self.wind_carry = 0.0

    def respawn(self, midbottom, ground):
        """Back to the start, standing on `ground`, with no jump, grip or climb in progress."""
        self.rect.midbottom = midbottom
        self.previous_rect = self.rect.copy()
        self.velocity = 0
        self.jump = self.falling = self.panicking = False
        self.standing_on = ground
        self.hanging_on = self.climbing_on = self.released_from = None

    def foot_xs(self):
        return (self.rect.centerx - KITTY_FOOT_REACH, self.rect.centerx, self.rect.centerx + KITTY_FOOT_REACH)

    def check_falling(self):
        # Walked off the tip, or back past the base through the trunk, or the branch snapped
        if self.standing_on is None or self.standing_on.surface_under(self.foot_xs()) is None:
            if getattr(self.standing_on, "broken", False):
                self.panicking = True
            self.standing_on = None
            if not self.falling:
                self.falling = True
                self.velocity = 0

    def resolve_supports(self, supports):
        """Ride the support Kitty is on, hang from or climb one, or catch one while falling.
        Returns the support Kitty just landed or climbed onto, if any."""
        if self.climbing_on is not None:
            return self._climb_step()
        if self.hanging_on is not None:
            y = self.hanging_on.surface_under([self.rect.centerx])
            if y is None:
                # Shuffled off the end, or the branch snapped
                self.let_go(panic=getattr(self.hanging_on, "broken", False))
            else:
                self.rect.top = y - GRIP_DEPTH
                self.hanging_on.apply_load(self.rect.centerx, KITTY_WEIGHT)
            return None
        if self.standing_on is not None:
            y = self.standing_on.surface_under(self.foot_xs())
            if y is not None:
                self.rect.bottom = y
                # Press down through whichever paw is on the branch, centre first
                feet = sorted(self.foot_xs(), key=lambda x: abs(x - self.rect.centerx))
                for x in feet:
                    if self.standing_on.surface_under([x]) is not None:
                        self.standing_on.apply_load(x, KITTY_WEIGHT)
                        break
            return None

        # Branches are one-way: Kitty jumps up through them and only lands coming down
        if self.jump or self.velocity < 0:
            return None
        best, best_y = None, None
        for support in supports:
            y = support.surface_under(self.foot_xs())
            if y is None:
                continue
            if self.previous_rect.bottom <= y + LANDING_TOLERANCE and self.rect.bottom >= y:
                if best_y is None or y < best_y:
                    best, best_y = support, y
        if best is None:
            self._try_grab(supports)
            return None

        best.kick(self.rect.centerx, self.velocity * LANDING_KICK)
        self.rect.bottom = best_y
        self.falling = False
        self.velocity = 0
        self.standing_on = best
        self.just_landed = True
        self.panicking = False
        return best

    def _try_grab(self, supports):
        """Falling just short of a branch: catch it if the raised paws cross its top."""
        if self.jump or self.velocity < 0:
            return
        x = self.rect.centerx
        for support in supports:
            if getattr(support, "is_ground", False) or getattr(support, "broken", False):
                continue
            if support is self.released_from:
                continue
            y = support.surface_under([x])
            if y is None:
                continue
            if self.previous_rect.top + GRAB_REACH <= y <= self.rect.top + GRAB_REACH:
                self.hanging_on = support
                self.falling = False
                self.velocity = 0
                self.panicking = False
                self.just_grabbed = True
                self.rect.top = y - GRIP_DEPTH
                support.kick(x, LANDING_KICK * 6)
                return

    def _climb_step(self):
        """Haul up from hanging to standing, easing the hitbox up past the branch."""
        branch = self.climbing_on
        y = branch.surface_under([self.rect.centerx])
        if y is None:
            self.climbing_on = None
            self.let_go(panic=getattr(branch, "broken", False))
            return None
        self.climb_frame += 1
        progress = min(1.0, self.climb_frame / CLIMB_FRAMES)
        eased = 1 - (1 - progress) ** 2
        hang_bottom = y - GRIP_DEPTH + self.rect.height
        self.rect.bottom = round(hang_bottom + (y - hang_bottom) * eased)
        branch.apply_load(self.rect.centerx, KITTY_WEIGHT)
        if progress < 1.0:
            return None
        self.climbing_on = None
        self.standing_on = branch
        self.just_climbed = True
        return branch

    def climb_up(self):
        if self.hanging_on is not None:
            self.climbing_on = self.hanging_on
            self.hanging_on = None
            self.climb_frame = 0

    def let_go(self, panic=False):
        if self.hanging_on is None and self.climbing_on is None:
            return
        self.released_from = self.hanging_on or self.climbing_on
        self.released_time = REGRAB_DELAY
        self.hanging_on = self.climbing_on = None
        self.falling = True
        self.velocity = 0
        self.panicking = panic

    def update(self, pressed_keys, breeze_strength, bounds=(0, SCREEN_WIDTH)):
        """`bounds` are the world's left and right edges in screen coordinates."""
        self.previous_rect = self.rect.copy()

        # Move left/right with arrow keys
        self.walk_dir = pressed_keys[pygame.K_RIGHT] - pressed_keys[pygame.K_LEFT]
        if self.walk_dir:
            self.facing = self.walk_dir
        if self.released_from is not None:
            self.released_time -= 1 / FPS
            if self.released_time <= 0:
                self.released_from = None
        if self.climbing_on is not None:
            return
        if self.hanging_on is not None:
            # Shuffle paw over paw along the branch
            self.rect.x += self.walk_dir * HANG_SHIMMY
            self.rect.clamp_ip(pygame.Rect(bounds[0], self.rect.y, bounds[1] - bounds[0], self.rect.height))
            return
        self.rect.x += self.walk_dir * self.speed

        # The wind pushes Kitty around while airborne (breeze strength affects this).
        # Fractions of a pixel are carried over so gentle breezes still cause drift.
        if self.jump or self.falling:
            self.wind_carry += breeze_strength * 0.3  # Scaled down to avoid excessive push
            drift = int(self.wind_carry)
            self.rect.x += drift
            self.wind_carry -= drift

        # Keep Kitty inside the world
        if self.rect.left < bounds[0]:
            self.rect.left = bounds[0]
        if self.rect.right > bounds[1]:
            self.rect.right = bounds[1]

        # Jumping and gravity logic
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
            self.check_falling()

    def do_jump(self):
        if self.hanging_on is not None:
            self.climb_up()
            return
        if self.climbing_on is not None:
            return
        if not self.jump and not self.falling:
            if self.standing_on is not None:
                self.standing_on.kick(self.rect.centerx, JUMP_KICK)
                self.standing_on = None
            self.jump = True
            self.jump_start_time = pygame.time.get_ticks()
            self.meow_sounds[self.meow_index].play()
            self.meow_index = (self.meow_index + 1) % len(self.meow_sounds)

    def stop_jump(self):
        if self.jump:
            self.velocity = self.gravity
            self.falling = True
            self.jump = False

    def animate(self, dt):
        """Pick the clip that matches what Kitty is doing, then advance it."""
        anim = self.animator
        if self.climbing_on is not None:
            self.move_time = self.idle_time = 0.0
            anim.play("climb")
        elif self.hanging_on is not None:
            self.move_time = self.idle_time = 0.0
            if self.just_grabbed:
                anim.play("grab", restart=True)
            elif anim.name != "grab" or anim.finished:
                anim.play("hang")
        elif self.standing_on is None:
            self.move_time = self.idle_time = 0.0
            if self.released_from is not None and not self.panicking:
                anim.play("let_go")
            elif self.panicking or self.velocity > PANIC_SPEED:
                anim.play("panic")
            elif self.jump:
                anim.play("rise")
            elif self.velocity < 3:
                anim.play("apex")
            else:
                anim.play("fall")
        elif self.walk_dir:
            self.idle_time = 0.0
            self.move_time += dt
            if self.move_time < RUN_AFTER:
                anim.play("walk")
            elif anim.name == "walk":
                anim.play("run_start")
            elif anim.name != "run_start" or anim.finished:
                anim.play("run")
        else:
            self.move_time = 0.0
            if self.just_landed:
                anim.play("land", restart=True)
            elif self.just_climbed:
                anim.play("climb_done", restart=True)
            if anim.name in ("land", "climb_done", "groom", "wave") and not anim.finished:
                pass  # let the squash or the fidget play out
            elif anim.name == "sleep" or self.idle_time >= SLEEP_AFTER:
                anim.play("sleep")
            elif self.idle_time >= self.next_fidget:
                anim.play(random.choice(["groom", "wave"]))
                self.next_fidget = self.idle_time + anim.clips[anim.name].duration + random.uniform(*FIDGET_DELAY)
            else:
                anim.play("sit")
            self.idle_time += dt
        if self.idle_time == 0.0:
            self.next_fidget = random.uniform(*FIDGET_DELAY)
        self.just_landed = self.just_grabbed = self.just_climbed = False
        anim.update(dt)
        self.image = anim.frame(self.facing)[0]

    def draw(self, screen):
        image, (ax, ay) = self.animator.frame(self.facing)
        pin_y = self.rect.top if self.animator.clip.pin == "grip" else self.rect.bottom
        screen.blit(image, (self.rect.centerx - ax, pin_y - ay))
