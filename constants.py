# ------------------ Constants -------------------
SCREEN_WIDTH, SCREEN_HEIGHT = 600, 1200
FPS = 60

# World Dimensions (Expanded)
WORLD_WIDTH = SCREEN_WIDTH * 2
WORLD_HEIGHT = int(SCREEN_HEIGHT * 1.5)

# Development Mode
DEV_MODE = True
SIDEBAR_WIDTH = 250

# Colors
BLACK = (0, 0, 0)
SKY_BLUE = (145, 220, 255)
WHITE = (255, 255, 255)
GRAY = (100, 100, 100)
UI_GRAY = (240, 240, 240)

# Physics Defaults
GRAVITY = 0.89
SUBSTEPS = 16
BRANCH_STIFFNESS = 0.32
BRANCH_DAMPING = 0.99
BRANCH_SEGMENTS = 13
SPRITE_MASS = 0.85
SPRITE_RESTITUTION = 0.27
TERMINAL_VEL_CAMERA_STOP = 800

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
