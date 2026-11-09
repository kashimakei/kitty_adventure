# `debug_ui.SimulationUI` – Integration Contract

This document describes how `kitty_adventure/main.py`'s `test_mode()` should
instantiate and drive `SimulationUI` from `systems/debug_ui.py`.

---

## 1. Import & instantiation

```python
from systems.debug_ui import SimulationUI

PANEL_W = SimulationUI.PANEL_W
ui = SimulationUI(
    panel_x = SCREEN_WIDTH - PANEL_W - 10,
    panel_y = 10,
)
```

Place the panel on the right-hand side so it never overlaps the branch.  
The panel is **collapsible** — clicking the title bar toggles it.

---

## 2. Event handling

Pass **every** pygame event to `ui.handle_event(event)` **before** your own
game logic.  It returns `True` when the event was consumed (e.g. mouse drag on
a slider), letting you skip downstream processing:

```python
for event in pygame.event.get():
    if event.type == pygame.QUIT:
        running = False
        continue
    consumed = ui.handle_event(event)
    if not consumed:
        # your own game events (click-to-spawn ball, etc.)
        if event.type == pygame.MOUSEBUTTONDOWN:
            mx, my = pygame.mouse.get_pos()
            balls.append(BallWrapper(mx, my))
```

---

## 3. Reading parameters each frame

Call `ui.get_values()` once per frame and cache the result:

```python
p = ui.get_values()
```

| Key                | Type   | Units / Notes                                  |
|--------------------|--------|------------------------------------------------|
| `balls_per_sec`    | float  | Spawn rate (0.2 – 3.5)                         |
| `spawn_x`          | float  | X-coord above which balls drop; auto-clamped   |
| `board_length`     | float  | Linear length of branch in pixels              |
| `gravity`          | float  | Downward acceleration (px / s²)                |
| `gloopification`   | float  | Friction loss `(0 – 0.05)` → `friction = 1 - val` |
| `density`          | float  | Ball density; `mass = π·r²·density`            |
| `flubberization`   | float  | Restitution coefficient (0.1 – 2.0)            |
| `stiffness`        | float  | Branch constraint stiffness (0.1 – 1.0)        |
| `ball_size`        | float  | Radius when `random_size` is False             |
| `wobble_threshold` | float  | Mean curvature radius threshold for wobble SFX |
| `random_size`      | bool   | Randomise ball radius on each spawn            |

---

## 4. Acting on buttons

Query the one-frame-true accessors **after** calling `handle_event`:

```python
if ui.was_clear_pressed():
    balls.clear()

if ui.was_reset_pressed():
    balls.clear()
    ui.reset_to_defaults()          # restores all sliders
    p = ui.get_values()             # refresh snapshot
    # Rebuild the branch with default length/stiffness:
    branches = [BranchWrapper(
        x_pos    = 0,
        y_pos    = 300,
        length   = p["board_length"],
        segments = max(2, int(p["board_length"] / 30)),
        stiffness= p["stiffness"],
        gnarl    = 1.0,
    )]
```

---

## 5. Driving the physics system

```python
# PhysicsSystem reads gravity & damping at construction time,
# so rebuild it when those sliders change – or expose setters.
physics = PhysicsSystem(
    gravity        = p["gravity"],
    global_damping = 1.0 - p["gloopification"],
)

physics.step_balls(balls, dt, friction=1.0 - p["gloopification"])
physics.step_branches(branches, sub_steps=4, dt=dt)
physics.resolve_collisions(balls, branches)
```

> **Note:** `PhysicsSystem` currently takes `gravity` and `global_damping`
> only at init. Consider adding `physics.gravity` and `physics.global_damping`
> as live-writable properties so the sliders take effect immediately without
> recreating the object each frame.

---

## 6. Spawning balls from slider values

```python
import random, math

spawn_timer   += dt * p["balls_per_sec"]
while spawn_timer >= 1.0:
    spawn_timer -= 1.0
    radius = random.uniform(10.0, 30.0) if p["random_size"] else p["ball_size"]
    balls.append(BallWrapper(
        p["spawn_x"] + random.uniform(-20, 20),
        -50,
        radius   = radius,
        density  = p["density"],
        restitution = p["flubberization"],
    ))
```

---

## 7. Drawing

Always draw the panel **last** so it sits on top of the scene:

```python
screen.fill(SKY_BLUE)

for b in branches:
    b.render(screen, camera_offset=0)
for b in balls:
    b.render(screen, camera_offset=0)

ui.draw(screen)          # ← always last
pygame.display.flip()
```

---

## 8. Board-length / stiffness rebuild

When the user moves the **Board Length** or **Stiffness** sliders, the branch
must be rebuilt.  Track the previous values and compare each frame:

```python
prev_len      = p["board_length"]
prev_stiff    = p["stiffness"]

# in the frame loop:
p = ui.get_values()
if abs(p["board_length"] - prev_len) > 1.0 or abs(p["stiffness"] - prev_stiff) > 0.001:
    branches = [BranchWrapper(
        x_pos    = 0,
        y_pos    = 300,
        length   = p["board_length"],
        segments = max(2, int(p["board_length"] / 30)),
        stiffness= p["stiffness"],
        gnarl    = 1.0,
    )]
    prev_len   = p["board_length"]
    prev_stiff = p["stiffness"]
```
