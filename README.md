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
assets/      # images and sounds; source/ holds the sprite sheets, kitty/ the cut frames
Engines/     # Rust soft-body branch physics (PyO3 module `branch_engine`)
entities/    # Kitty, trees, bendy branches, dogs, eagles, leaves
systems/     # camera, physics helpers, debug UI
tools/       # asset pipeline (sprite sheet slicer)
main.py      # main platformer starring a cat
platformer   # simplified vertical jumping example
```

Branch feel (how much they bend, wobble and kick back) is tuned in the
"Trees & Bendy Branches" section of `constants.py`. Pale, thin branches hold
Kitty or a dog, but snap under both. Level difficulty (spacing, bendiness, wind
and enemies) ramps up in `difficulty_profile()` in `level_utils.py`.

Kitty's, the dogs' and the eagles' animations come from the painted sprite
sheets in `assets/source/`. `python tools/slice_sprites.py` keys out their grey
background, cuts each pose into `assets/kitty/`, `assets/dog/` and
`assets/eagle/`, scales each character consistently across its sheets and records the point each frame is pinned by (feet, Kitty's
gripping paw, or the eagle's head) in `anchors.json`. For enemies it also writes
a damage mask per frame (`*_hit.png`): only a dog's paws and jaws or an eagle's
beak and talons hurt, and only when they touch Kitty's body (see `KITTY_BODY`
in `entities/kitty.py`), not the tail. Re-run it after editing a
sheet; add `--index <dir>` for numbered previews of every pose it finds. Which
frames make up each move and when they play is set in `KITTY_CLIPS` /
`Kitty.animate()` (`entities/kitty.py`), and `DOG_CLIPS` / `Dog` and
`EAGLE_CLIPS` / `Eagle` (`entities/platform.py`).

Controls: arrows to walk, Space to jump. Fall just short of a branch and Kitty
catches it by the paws: Left/Right shimmy along, Space or Up climbs on, Down
lets go. Eagles glide with their wings set for a moment before they dive, so
that's the cue to get moving. Dogs roam the ground: they snarl before they
charge, bark at Kitty overhead, and leap onto any branch low enough to reach.

Levels are `WORLD_SCREENS` (3) screens wide: Kitty starts in the middle and the
camera pans sideways as Kitty nears the edge of the screen. The bar in the HUD
shows the whole level, the visible slice, Kitty, and the gold goal branch.

Feel free to explore the code to tweak the physics, add new levels, or extend the gameplay with new features.
