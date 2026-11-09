
import pygame

# ------------------ Constants -------------------
SCREEN_WIDTH, SCREEN_HEIGHT = 600, 1200
FPS = 60

# The level is several screens wide; Kitty starts in the middle one and the camera scrolls sideways
WORLD_SCREENS = 3
WORLD_WIDTH = SCREEN_WIDTH * WORLD_SCREENS
CAMERA_DEADZONE = 0.35   # Kitty roams the middle of the screen freely before the camera pans
BACKGROUND_PARALLAX = 0.3  # the backdrop pans this fraction as fast as the trees

# Colors
BLACK = (0, 0, 0)
SKY_BLUE = (145, 220, 255)
WHITE = (255, 255, 255)

# Fall Colors for Leaves
FALL_COLORS = [
    (139, 37, 0),   # Dark Red
    (205, 55, 0),   # Burnt Orange
    (238, 118, 0),  # Golden Orange
    (205, 133, 63), # Peru Brown
    (160, 82, 45),  # Sienna
    (218, 165, 32), # Golden Rod
    (184, 134, 11), # Dark Golden Rod
    (139, 69, 19),  # Saddle Brown
]

MAX_LEAVES = 50

# ------------------ Trees & Bendy Branches -------------------
TRUNK_WIDTH = 50
TRUNK_COLOR = (92, 58, 32)
BARK_COLOR = (120, 74, 38)
BARK_DARK = (70, 42, 20)
BARK_LIGHT = (160, 108, 62)

BRANCH_LENGTH = (170, 220)        # natural spread; branches no longer shrink as levels go up
BRANCH_STURDY_DEPTH = (30, 38)    # thickness where the branch leaves the trunk
BRANCH_THIN_DEPTH = (22, 26)
THIN_BRANCH_CHANCE = 0.3
THIN_BRANCH_STRENGTH = (1.15, 1.3)  # x KITTY_WEIGHT: holds Kitty or a dog, snaps under both
MAX_BRANCH_GAP = 360              # tallest climb between branches Kitty can still make
BRANCH_SEGMENT_LEN = 18.0         # pixels per physics segment
BRANCH_SUB_STEPS = 8
BRANCH_GRAVITY = 800.0
BRANCH_DAMPING = 0.995       # per sub-step velocity retention; lower = less wobble
BRANCH_WIND = 120.0          # how much the breeze bobs the branches

# Forces the critters put on the branches (px/s^2 spread over the nodes under them)
KITTY_WEIGHT = 38000.0
DOG_WEIGHT = 18000.0
LANDING_KICK = 0.06          # fraction of landing speed passed into the branch
JUMP_KICK = 1.2              # downward push on the branch when kitty leaps off
KITTY_FOOT_REACH = 30        # half-width of kitty's paws, measured from rect.centerx
LANDING_TOLERANCE = 12       # px a springing branch may rise past kitty's feet and still catch the kitty
BARK_YOUNG = (164, 120, 70)  # paler wood marks thin branches that can snap
BRANCH_TREMBLE = 6000.0      # shaking force on a branch close to breaking
MAX_FRAME_DT = 1 / 30        # physics never steps more than this, even after a pause
TREE_SPACING = 270           # rough distance between trunks; adjusted so trees fill the world evenly
MAX_BRANCH_HOP = 150         # widest sideways gap between one path branch and the next
FILLERS_PER_PATH_BRANCH = 2  # attempts at an extra branch near each path branch

# Hanging & climbing: Kitty catches a branch by the paws when a jump falls just short
GRAB_REACH = 22             # px below the top of Kitty's hitbox where the raised paws are
GRIP_DEPTH = 4              # px the gripping paw wraps over the top of the branch
HANG_SHIMMY = 2             # px/frame Kitty can shuffle along while hanging
CLIMB_FRAMES = 33           # frames (at 60fps) to haul up onto the branch
REGRAB_DELAY = 0.4          # seconds after letting go before the same branch can be caught again

# Eagles cruise across, swoop once at Kitty if Kitty is ahead and below, then pull away
EAGLE_HITBOX = (100, 72)
EAGLE_HEAD_OFFSET = (42, -15)  # where the head sits relative to the hitbox centre, facing right
EAGLE_SWOOP_RANGE = 280        # px ahead at which an eagle commits to a swoop
EAGLE_SWOOP_SPEED = 340        # px/s, scaled by the level's eagle speed
EAGLE_WINDUP = 0.35            # seconds an eagle glides, wings set, before diving: the player's cue
EAGLE_HOMING = 0.6             # rad/s an eagle can bend its swoop; low enough to dodge
EAGLE_SWOOP_TIME = 1.4         # seconds before a swoop gives up

# Dogs live on the ground and can leap onto low branches
DOG_HITBOX = (80, 64)
DOG_WALK_SPEED = 45          # px/s sniffing along, scaled by the level's dog speed
DOG_CHARGE_SPEED = 190       # px/s galloping at Kitty
DOG_GRAVITY = 1800           # px/s^2
DOG_JUMP_HEIGHT = 170        # tallest leap, feet to branch top
DOG_SIGHT = 260              # px; Kitty this close on the same level gets snarled at, then chased
DOG_SNARL_TIME = 0.5         # seconds of snarling before the charge: the player's cue
DOG_BARK_RANGE = 160         # px sideways; Kitty above and within this gets barked at
DOG_BARK_BEFORE_JUMP = 0.8   # seconds of barking before leaping up at Kitty's branch
DOG_SCRATCH_EVERY = (5.0, 10.0)
DOG_EXPLORE_CHANCE = 0.15    # per second, while under a low branch, of hopping up to look around
DOG_BRANCH_STAY = (4.0, 8.0)  # seconds a wandering dog stays up a branch before hopping down
