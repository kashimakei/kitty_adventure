import functools
import json
import os
import pygame

@functools.lru_cache(maxsize=None)
def solid_mask(size):
    """A filled mask of `size`, for testing a rect against a sprite's damage mask."""
    return pygame.mask.Mask(size, fill=True)


class SpriteFrames:
    """Frames cut by tools/slice_sprites.py, loaded once in both facings.
    Each frame comes with an anchor: the pixel that sits on the character's hitbox midbottom.
    Enemy frames may also have a damage mask: the pixels (paws, jaws, beak, talons) that hurt."""

    def __init__(self, folder):
        with open(os.path.join(folder, "anchors.json")) as f:
            manifest = json.load(f)
        self.right = {}
        self.left = {}
        self.right_hits = {}
        self.left_hits = {}
        hits = manifest.get("hits", {})
        for names in manifest["animations"].values():
            for filename in names:
                name = filename[:-4]
                image = pygame.image.load(os.path.join(folder, filename)).convert_alpha()
                ax, ay = manifest["anchors"][filename]
                self.right[name] = (image, (ax, ay))
                # Mirrored frame, mirrored anchor
                self.left[name] = (pygame.transform.flip(image, True, False), (image.get_width() - 1 - ax, ay))
                if filename in hits:
                    hit = pygame.image.load(os.path.join(folder, hits[filename])).convert_alpha()
                    self.right_hits[name] = pygame.mask.from_surface(hit, 127)
                    self.left_hits[name] = pygame.mask.from_surface(pygame.transform.flip(hit, True, False), 127)

    def get(self, name, facing=1):
        return (self.right if facing >= 0 else self.left)[name]

    def hit_mask(self, name, facing=1):
        """The frame's damage mask, or None if this character can't hurt anything."""
        return (self.right_hits if facing >= 0 else self.left_hits).get(name)


class Clip:
    """A named run of frames. Looping clips repeat; others hold their last frame when done.
    `pin` says which point of the hitbox the frames' anchors sit on: "feet" (midbottom) for
    standing and flying poses, "grip" (midtop) for hanging ones."""

    def __init__(self, frames, fps, loop=True, pin="feet"):
        self.frames = frames
        self.fps = fps
        self.loop = loop
        self.pin = pin

    @property
    def duration(self):
        return len(self.frames) / self.fps


class Animator:
    """Plays one clip at a time and reports the frame to draw."""

    def __init__(self, frames, clips, start):
        self.frames = frames
        self.clips = clips
        self.name = start
        self.time = 0.0

    def play(self, name, restart=False):
        if name != self.name or restart:
            self.name = name
            self.time = 0.0

    def update(self, dt):
        self.time += dt

    @property
    def clip(self):
        return self.clips[self.name]

    @property
    def finished(self):
        clip = self.clips[self.name]
        return not clip.loop and self.time >= clip.duration

    @property
    def frame_name(self):
        clip = self.clips[self.name]
        index = int(self.time * clip.fps)
        index = index % len(clip.frames) if clip.loop else min(index, len(clip.frames) - 1)
        return clip.frames[index]

    def frame(self, facing=1):
        return self.frames.get(self.frame_name, facing)

    def strikes(self, topleft, facing, rect):
        """True if the current frame, drawn at `topleft`, has damaging pixels inside `rect`."""
        mask = self.frames.hit_mask(self.frame_name, facing)
        if mask is None:
            return False
        offset = (rect.x - round(topleft[0]), rect.y - round(topleft[1]))
        return mask.overlap(solid_mask(rect.size), offset) is not None
