# Rust Tree Branch Engine - App Contract

This contract defines the interface and behaviors of the `rusty_branch` Python module, providing instructions for AI agents and developers implementing it into the Python game app.

## Overview

The `rusty_branch` module is a high-performance Python extension written in Rust (via PyO3). It generates procedural "tree branch" structures modeled as interconnected nodes (a SoftBody) and computes physics for natural bending and oscillation.

**Intended Usage:**
- The Python codebase uses `rusty_branch.generate_branch(...)` to spawn a branch struct.
- The Python game loop calls the `.update(dt, ...)` method precisely once per game tick.
- During rendering, the Python game iterates over the branch's `.nodes` to draw lines representing the branch.
- Collision detection happens either against the physical points in Python, or by passing collision objects into Rust (TBD based on developer setup).

## Installation

The module should be built into the active Python environment or local directory using `maturin`:
```bash
pip install maturin
maturin develop --release
```

Include it in Python scripts via:
```python
import rusty_branch
```

## API Specification

### Data Structures

#### `Vec2`
A simple 2D vector for coordinates.
* **Fields:**
  - `x` (float)
  - `y` (float)

#### `Node`
A point mass making up a segment of the branch.
* **Properties:**
  - `pos` (`Vec2`): Current position
  - `radius` (float): Thickness of the node for drawing/collision
  - `pinned` (bool): If true, the node cannot move (typically the base of the branch attached to the trunk).

#### `Constraint`
A rigid or spring-like linkage between nodes.
* **Properties:**
  - `node_a` (int): Index of the first node.
  - `node_b` (int): Index of the second node.

#### `Branch` (previously SoftBody)
The primary collection of nodes representing the physical object.
* **Properties:**
  - `nodes` (List[`Node`]): An ordered list of physics points.
  - `constraints` (List[`Constraint`]): The springs connecting the nodes.
* **Methods:**
  - `update_physics(dt: float)`: Steps the internal physics simulation by delta time `dt`.
  - `resolve_collision(x: float, y: float, radius: float)`: Optional physics interaction API (TBD based on implementation).

### Instantiation

```python
def generate_branch(start_x: float, start_y: float, length: float, base_depth: float, segments: int, stiffness: float, gnarl: float) -> Branch:
    """
    Generates a new procedurally curved branch.

    Args:
        start_x: The root x coordinate (e.g. at the tree trunk).
        start_y: The root y coordinate.
        length: Linear length of the branch in pixels.
        base_depth: Thickness magnitude at the start of the branch.
        segments: Number of structural joints. Higher is smoother but costs more CPU.
        stiffness: (0.0 to 1.0) Physics parameter for rigidity.
        gnarl: A multiplier altering the amplitude and frequency of the branch sine wave shape. 
               1.0 = normal, 2.0 = highly jagged, 0.5 = almost straight.
    """
```

## Example Integration

```python
import rusty_branch

class GameObjBranch:
    def __init__(self, x_pos, y_pos):
        # 1. Generate the physical struct 
        self.branch_struct = rusty_branch.generate_branch(
            start_x=x_pos, 
            start_y=y_pos, 
            length=600.0, 
            base_depth=40.0, 
            segments=20, 
            stiffness=0.95,
            gnarl=1.2 # slightly more random/curved than standard
        )

    def tick(self, dt):
        # 2. Advance Rust interior simulation
        self.branch_struct.update_physics(dt)

    def render(self, screen):
        # 3. Read positions directly from Rust memory representations
        # Top contour nodes
        nodes = self.branch_struct.nodes
        half = len(nodes) // 2
        for i in range(half - 1):
            n1 = nodes[i]
            n2 = nodes[i+1]
            pygame.draw.line(screen, (100, 50, 0), (n1.pos.x, n1.pos.y), (n2.pos.x, n2.pos.y), int(n1.radius))
```

## Guidelines for AI Agents
1. **Never** attempt to modify `rusty_branch.Branch.nodes` via generic Python assignment if iterating through references. Variables received from PyO3 properties behave as copies unless explicitly bound as reference endpoints; consult PyO3 specs if mutable python -> rust updates are necessary.
2. Ensure you initialize the Python Virtual Environment or verify PyO3 compile step is successful before running `import rusty_branch`.
3. Use the `gnarl` parameter as the primary "difficulty/strangeness" parameter when composing procedural trees. Wait for user specification before adjusting rendering width via `base_depth`.
