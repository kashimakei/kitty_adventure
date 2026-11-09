import branch_engine

class PhysicsSystem:
    def __init__(self, gravity=500.0, global_damping=0.99):
        self.gravity = gravity
        self.global_damping = global_damping

    def step_branches(self, branches, sub_steps, dt):
        for branch in branches:
            branch.update_physics(sub_steps, dt, self.gravity, self.global_damping)

    def step_balls(self, balls, dt, friction=0.99):
        gravity_vec = branch_engine.Vec2(0.0, self.gravity)
        for ball in balls:
            # apply gravity
            ball.ball_struct.acc = branch_engine.Vec2(ball.ball_struct.acc.x + gravity_vec.x, ball.ball_struct.acc.y + gravity_vec.y)
            ball.update(dt, friction)

    def resolve_collisions(self, balls, branches):
        # Ball vs Branch
        for ball in balls:
            for branch in branches:
                branch_engine.ball.resolve_ball_plank_collision(ball.ball_struct, branch.branch_struct)
        
        # Ball vs Ball
        for i in range(len(balls)):
            for j in range(i + 1, len(balls)):
                branch_engine.ball.resolve_ball_ball_collision(balls[i].ball_struct, balls[j].ball_struct)
