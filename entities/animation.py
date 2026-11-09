import json
import os
import pygame

class SpriteFrames:
    """Frames cut by tools/slice_sprites.py, loaded once in both facings.
    Each frame comes with an anchor: the pixel that sits on the character's hitbox midbottom."""

    def __init__(self, folder):
        with open(os.path.join(folder, "anchors.json")) as f:
            manifest = json.load(f)
        self.right = {}
        self.left = {}
        for names in manifest["animations"].values():
            for filename in names:
                name = filename[:-4]
                image = pygame.image.load(os.path.join(folder, filename)).convert_alpha()
                ax, ay = manifest["anchors"][filename]
                self.right[name] = (image, (ax, ay))
                # Mirrored frame, mirrored anchor
                self.left[name] = (pygame.transform.flip(image, True, False), (image.get_width() - 1 - ax, ay))

    def get(self, name, facing=1):
        return (self.right if facing >= 0 else self.left)[name]


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

    def frame(self, facing=1):
        clip = self.clips[self.name]
        index = int(self.time * clip.fps)
        index = index % len(clip.frames) if clip.loop else min(index, len(clip.frames) - 1)
        return self.frames.get(clip.frames[index], facing)
