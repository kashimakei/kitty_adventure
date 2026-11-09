# Kitty Adventure

This repository provides two small games built with [Pygame](https://www.pygame.org/). The main game, `main.py`, is a vertical platformer starring a jumping cat who climbs trees by hopping between bendy branches. The branches are soft bodies simulated by the Rust `branch_engine` in `Engines/` (from the Rusty-Board project): they sag and wobble under Kitty and the dogs, while the trunks are scenery Kitty can pass straight through. The `platformer` script contains a simpler vertical jumping demo. Game art and sound effects are stored in the `assets/` directory.

## Requirements

- Python 3.7+
- Pygame

- A Rust toolchain (for the branch physics engine)

Install dependencies and build the engine into your virtualenv:

```bash
pip install -r requirements.txt
cd Engines && maturin develop --release && cd ..
```

Rebuild with `maturin develop --release` after changing anything in `Engines/src/`.

## Running the games

Run the main game:

```bash
python main.py
```

Or the branch physics sandbox (click to drop balls):

```bash
python main.py --test
```

Run the smaller demo:

```bash
python platformer
```

Both scripts expect the `assets/` directory to be present in the repository root. Launching `main.py` will display a splash screen and then begin the first level.

## Project structure

```
assets/      # images and sounds used by the games
Engines/     # Rust soft-body branch physics (PyO3 module `branch_engine`)
entities/    # Kitty, trees, bendy branches, dogs, eagles, leaves
systems/     # camera, physics helpers, debug UI
main.py      # main platformer starring a cat
platformer   # simplified vertical jumping example
```

Branch feel (how much they bend, wobble and kick back) is tuned in the
"Trees & Bendy Branches" section of `constants.py`. Pale, thin branches hold
Kitty or a dog, but snap under both. Level difficulty (spacing, bendiness, wind
and enemies) ramps up in `difficulty_profile()` in `level_utils.py`.

Levels are `WORLD_SCREENS` (3) screens wide: Kitty starts in the middle and the
camera pans sideways as Kitty nears the edge of the screen. The bar in the HUD
shows the whole level, the visible slice, Kitty, and the gold goal branch.

Feel free to explore the code to tweak the physics, add new levels, or extend the gameplay with new features.
