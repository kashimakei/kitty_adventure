# Kitty Adventure

This repository provides two small games built with [Pygame](https://www.pygame.org/). The main game, `kitty.py`, is a side-scrolling platformer starring a jumping cat. The `platformer` script contains a simpler vertical jumping demo. Game art and sound effects are stored in the `assets/` directory.

## Requirements

- Python 3.7+
- Pygame

Install dependencies using `pip`:

```bash
pip install -r requirements.txt
```

## Running the games

Run the main game:

```bash
python kitty.py
```

Run the smaller demo:

```bash
python platformer
```

Both scripts expect the `assets/` directory to be present in the repository root. Launching `kitty.py` will display a splash screen and then begin the first level.

## Project structure

```
assets/      # images and sounds used by the games
kitty.py     # main platformer starring a cat
platformer   # simplified vertical jumping example
```

Feel free to explore the code to tweak the physics, add new levels, or extend the gameplay with new features.
