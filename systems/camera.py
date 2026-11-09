from constants import SCREEN_WIDTH, SCREEN_HEIGHT, WORLD_WIDTH, CAMERA_DEADZONE

class CameraSystem:
    def __init__(self):
        self.offset = 0
        self.follow_kitty = False
        # How far the left edge of the screen is from the left edge of the world
        self.start_x = (WORLD_WIDTH - SCREEN_WIDTH) // 2
        self.offset_x = self.start_x

    def world_bounds(self):
        """The world's left and right edges in screen coordinates."""
        left = -self.offset_x
        return (left, left + WORLD_WIDTH)

    def update(self, kitty, world, leaves, eagles):
        """`world` is anything with a shift(dx, dy) method: ground, trees, branches, dogs."""
        if kitty.jump and kitty.rect.top <= SCREEN_HEIGHT / 2:
            self.follow_kitty = True

        dy = 0
        if self.follow_kitty and not kitty.falling:
            dy = max(0, (SCREEN_HEIGHT / 2) - kitty.rect.top) * 0.1
            self.offset += dy

        if kitty.falling:
            self.follow_kitty = False

        # Pan sideways once Kitty leaves the middle of the screen, never past the world's edges.
        # Whole pixels only, so Kitty's int rect and the float-positioned branches stay in step.
        dx = 0
        margin = SCREEN_WIDTH * CAMERA_DEADZONE
        if kitty.rect.centerx < margin:
            dx = int(margin - kitty.rect.centerx)
        elif kitty.rect.centerx > SCREEN_WIDTH - margin:
            dx = -int(kitty.rect.centerx - (SCREEN_WIDTH - margin))
        dx = max(-(WORLD_WIDTH - SCREEN_WIDTH - self.offset_x), min(self.offset_x, dx))
        self.offset_x -= dx

        if dx == 0 and dy == 0:
            return
        kitty.rect.x += dx
        kitty.rect.y += dy
        for obj in world:
            obj.shift(dx, dy)
        for leaf in leaves:
            leaf.rect.move_ip(dx, dy)
            leaf.x += dx
            leaf.y += dy
        for eagle in eagles:
            eagle.rect.move_ip(dx, dy)

    def reset(self):
        self.offset = 0
        self.follow_kitty = False
        self.offset_x = self.start_x
