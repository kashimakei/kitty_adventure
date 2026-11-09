import pygame
import branch_engine

class BallWrapper:
    def __init__(self, x_pos, y_pos, radius=15.0, density=1.0, restitution=0.8):
        pos_vec = branch_engine.Vec2(x_pos, y_pos)
        self.ball_struct = branch_engine.Ball(pos_vec, radius, density, restitution)
        # Give it an initial downward nudge just in case
        self.ball_struct.acc = branch_engine.Vec2(0.0, 500.0)

    def update(self, dt, friction=0.99):
        self.ball_struct.update(dt, friction)

    def render(self, screen, camera_offset=0):
        pos = self.ball_struct.pos
        rad = int(self.ball_struct.radius)
        # Draw outline
        pygame.draw.circle(screen, (255, 255, 255), (int(pos.x), int(pos.y - camera_offset)), rad + 2)
        # Draw inside
        pygame.draw.circle(screen, (200, 50, 50), (int(pos.x), int(pos.y - camera_offset)), rad)
