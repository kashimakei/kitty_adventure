"""
debug_ui.py  –  Pygame-native simulation-parameter panel.

Mirrors the "Simulation Controls" egui window from Rusty-Board/src/main.rs.

Usage
-----
    from systems.debug_ui import SimulationUI

    ui = SimulationUI(panel_x=screen_width - SimulationUI.PANEL_W - 10, panel_y=10)

    # in your event loop:
    for event in pygame.event.get():
        ui.handle_event(event)

    # in your update step:
    p = ui.get_values()
    # p["gravity"], p["balls_per_sec"], p["spawn_x"], etc.

    # in your draw step:
    ui.draw(screen)
"""

from __future__ import annotations
import pygame

# ---------------------------------------------------------------------------
# Palette – all colours used by the panel
# ---------------------------------------------------------------------------
_BG          = (30,  30,  32)
_BORDER      = (70,  70,  75)
_LABEL       = (200, 200, 205)
_TRACK       = (55,  55,  58)
_THUMB       = (160, 160, 168)
_THUMB_HOT   = (210, 210, 220)
_BTN         = (60,  60,  65)
_BTN_HOT     = (85,  85,  92)
_BTN_LABEL   = (220, 220, 225)
_CHECK_FILL  = (100, 180, 110)
_TITLE_BG    = (45,  45,  50)
_TITLE_TEXT  = (230, 230, 235)
_VALUE_TEXT  = (150, 200, 160)

_FONT_SIZE_SM = 13
_FONT_SIZE_MD = 14


def _mkfont(size: int) -> pygame.font.Font:
    return pygame.font.SysFont("monospace", size)


# ---------------------------------------------------------------------------
# Slider
# ---------------------------------------------------------------------------
class Slider:
    H  = 22   # total widget height
    TH = 10   # thumb half-width

    def __init__(
        self,
        label: str,
        min_val: float,
        max_val: float,
        value: float,
        fmt: str = ".2f",
    ) -> None:
        self.label   = label
        self.min_val = min_val
        self.max_val = max_val
        self.value   = float(value)
        self.fmt     = fmt
        self._dragging = False
        self.rect: pygame.Rect = pygame.Rect(0, 0, 1, 1)  # set by layout

    # allow live-updating the range (e.g. Spawn X depends on Board Length)
    def set_range(self, min_val: float, max_val: float) -> None:
        self.min_val = min_val
        self.max_val = max_val
        self.value   = max(min_val, min(max_val, self.value))

    def _value_to_x(self) -> float:
        ratio = (self.value - self.min_val) / max(self.max_val - self.min_val, 1e-6)
        return self.rect.x + self.TH + ratio * (self.rect.width - 2 * self.TH)

    def _x_to_value(self, px: float) -> float:
        usable = self.rect.width - 2 * self.TH
        ratio  = (px - self.rect.x - self.TH) / max(usable, 1e-6)
        ratio  = max(0.0, min(1.0, ratio))
        return self.min_val + ratio * (self.max_val - self.min_val)

    def handle_event(self, event: pygame.event.Event) -> bool:
        """Return True if the event was consumed."""
        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
            if self.rect.collidepoint(event.pos):
                self._dragging = True
                self.value = self._x_to_value(event.pos[0])
                return True
        elif event.type == pygame.MOUSEBUTTONUP and event.button == 1:
            if self._dragging:
                self._dragging = False
                return True
        elif event.type == pygame.MOUSEMOTION and self._dragging:
            self.value = self._x_to_value(event.pos[0])
            return True
        return False

    def draw(self, surface: pygame.Surface, font_sm: pygame.font.Font, font_md: pygame.font.Font) -> None:
        r = self.rect
        # Track background
        track_rect = pygame.Rect(r.x + self.TH, r.centery - 3, r.width - 2 * self.TH, 6)
        pygame.draw.rect(surface, _TRACK, track_rect, border_radius=3)

        # Thumb
        tx = int(self._value_to_x())
        thumb_rect = pygame.Rect(tx - self.TH, r.centery - self.TH // 2, self.TH * 2, self.TH)
        hot = self._dragging or thumb_rect.collidepoint(pygame.mouse.get_pos())
        pygame.draw.rect(surface, _THUMB_HOT if hot else _THUMB, thumb_rect, border_radius=3)

        # Label  (left)
        lbl_surf = font_sm.render(self.label, True, _LABEL)
        surface.blit(lbl_surf, (r.x, r.y - lbl_surf.get_height() - 1))

        # Value  (right of label)
        val_str  = format(self.value, self.fmt)
        val_surf = font_sm.render(val_str, True, _VALUE_TEXT)
        surface.blit(val_surf, (r.right - val_surf.get_width(), r.y - val_surf.get_height() - 1))


# ---------------------------------------------------------------------------
# Checkbox
# ---------------------------------------------------------------------------
class Checkbox:
    SIZE = 14

    def __init__(self, label: str, checked: bool = False) -> None:
        self.label   = label
        self.checked = checked
        self.rect: pygame.Rect = pygame.Rect(0, 0, 1, 1)

    def handle_event(self, event: pygame.event.Event) -> bool:
        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
            if self.rect.collidepoint(event.pos):
                self.checked = not self.checked
                return True
        return False

    def draw(self, surface: pygame.Surface, font_sm: pygame.font.Font, font_md: pygame.font.Font) -> None:
        box = pygame.Rect(self.rect.x, self.rect.centery - self.SIZE // 2, self.SIZE, self.SIZE)
        pygame.draw.rect(surface, _TRACK, box, border_radius=2)
        if self.checked:
            inner = box.inflate(-4, -4)
            pygame.draw.rect(surface, _CHECK_FILL, inner, border_radius=1)
        pygame.draw.rect(surface, _THUMB, box, 1, border_radius=2)

        lbl_surf = font_sm.render(self.label, True, _LABEL)
        surface.blit(lbl_surf, (box.right + 6, self.rect.centery - lbl_surf.get_height() // 2))


# ---------------------------------------------------------------------------
# Button
# ---------------------------------------------------------------------------
class Button:
    H = 22

    def __init__(self, label: str) -> None:
        self.label    = label
        self.pressed  = False   # True for exactly one frame after click
        self._is_hot  = False
        self.rect: pygame.Rect = pygame.Rect(0, 0, 1, 1)

    def handle_event(self, event: pygame.event.Event) -> bool:
        self.pressed = False
        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
            if self.rect.collidepoint(event.pos):
                self.pressed = True
                return True
        return False

    def draw(self, surface: pygame.Surface, font_sm: pygame.font.Font, font_md: pygame.font.Font) -> None:
        self._is_hot = self.rect.collidepoint(pygame.mouse.get_pos())
        pygame.draw.rect(surface, _BTN_HOT if self._is_hot else _BTN, self.rect, border_radius=4)
        pygame.draw.rect(surface, _BORDER, self.rect, 1, border_radius=4)

        lbl_surf = font_md.render(self.label, True, _BTN_LABEL)
        lx = self.rect.centerx - lbl_surf.get_width()  // 2
        ly = self.rect.centery - lbl_surf.get_height() // 2
        surface.blit(lbl_surf, (lx, ly))


# ---------------------------------------------------------------------------
# SimulationUI  –  the full side panel
# ---------------------------------------------------------------------------

# Default physics values matching Rusty-Board/src/main.rs constants
_DEFAULTS = dict(
    balls_per_sec    = 1.5,
    spawn_x          = 350.0,
    board_length     = 700.0,
    gravity          = 800.0,
    gloopification   = 0.001,   # 1.0 - FRICTION  (FRICTION = 0.999)
    density          = 0.05,
    flubberization   = 0.85,
    stiffness        = 0.95,
    ball_size        = 15.0,
    wobble_threshold = 800.0,
    random_size      = True,
)

class SimulationUI:
    """
    A collapsible side-panel with sliders, checkboxes, and buttons that
    replicates the macroquad egui "Simulation Controls" window.
    """

    PANEL_W   = 310
    PADDING   = 10
    ROW_H     = 40    # space allocated per slider row (label + track)
    BTN_H     = 28
    CB_H      = 24
    TITLE_H   = 28

    def __init__(self, panel_x: int = 10, panel_y: int = 10) -> None:
        self._px = panel_x
        self._py = panel_y
        self._collapsed = False

        # ---- widgets (order matches Rust UI top→bottom) ----
        self._sliders: dict[str, Slider] = {
            "balls_per_sec":    Slider("Balls/Sec",          0.2,  3.5,    _DEFAULTS["balls_per_sec"],    ".2f"),
            "spawn_x":          Slider("Spawn X",            0.0,  700.0,  _DEFAULTS["spawn_x"],          ".0f"),
            "board_length":     Slider("Board Length",       100.0,1500.0, _DEFAULTS["board_length"],     ".0f"),
            "gravity":          Slider("Gravity",            0.0,  2000.0, _DEFAULTS["gravity"],          ".0f"),
            "gloopification":   Slider("Gloopification",     0.0,  0.05,   _DEFAULTS["gloopification"],   ".4f"),
            "density":          Slider("Density",            0.001,0.05,   _DEFAULTS["density"],          ".4f"),
            "flubberization":   Slider("Flubberization",     0.1,  2.0,    _DEFAULTS["flubberization"],   ".2f"),
            "stiffness":        Slider("Stiffness",          0.1,  1.0,    _DEFAULTS["stiffness"],        ".3f"),
            "ball_size":        Slider("Ball Size",          5.0,  50.0,   _DEFAULTS["ball_size"],        ".0f"),
            "wobble_threshold": Slider("Wobble Threshold",   100.0,5000.0, _DEFAULTS["wobble_threshold"], ".0f"),
        }
        self._random_size_cb = Checkbox("Random Size", checked=_DEFAULTS["random_size"])
        self._clear_btn      = Button("Clear Balls")
        self._reset_btn      = Button("Reset Everything")

        self._font_sm  = _mkfont(_FONT_SIZE_SM)
        self._font_md  = _mkfont(_FONT_SIZE_MD)
        self._panel_rect: pygame.Rect = pygame.Rect(0, 0, 1, 1)
        self._title_rect: pygame.Rect = pygame.Rect(0, 0, 1, 1)

        # callbacks – set these to react to button presses if you prefer
        # callbacks can also just be polled via get_values() / was_pressed()
        self._do_layout()

    # ------------------------------------------------------------------
    # Layout helpers
    # ------------------------------------------------------------------

    def _do_layout(self) -> None:
        """Recompute widget rects from current panel position."""
        px, py = self._px, self._py
        w = self.PANEL_W
        p = self.PADDING
        inner_w = w - p * 2

        # Title bar
        self._title_rect = pygame.Rect(px, py, w, self.TITLE_H)
        y = py + self.TITLE_H + p

        if self._collapsed:
            self._panel_rect = pygame.Rect(px, py, w, self.TITLE_H)
            return

        for key, slider in self._sliders.items():
            # Each row: label row above, track row =  ROW_H total
            slider.rect = pygame.Rect(px + p, y + 16, inner_w, Slider.H)
            y += self.ROW_H

        # Checkbox
        self._random_size_cb.rect = pygame.Rect(px + p, y, inner_w, self.CB_H)
        y += self.CB_H + 4

        # Buttons side by side
        bw = (inner_w - 6) // 2
        self._clear_btn.rect = pygame.Rect(px + p,          y, bw, self.BTN_H)
        self._reset_btn.rect = pygame.Rect(px + p + bw + 6, y, bw, self.BTN_H)
        y += self.BTN_H + p

        self._panel_rect = pygame.Rect(px, py, w, y - py)

    # ------------------------------------------------------------------
    # Dynamic range update (Spawn X max = current board_length + 200 as offset)
    # ------------------------------------------------------------------
    def _update_dynamic_ranges(self) -> None:
        bl = self._sliders["board_length"].value
        board_start = 200.0           # matches Rust: Vec2::new(200.0, …)
        board_end   = board_start + bl
        self._sliders["spawn_x"].set_range(board_start, board_end)

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def handle_event(self, event: pygame.event.Event) -> bool:
        """
        Pass every pygame event here before your own game logic.
        Returns True if the event was consumed by the panel so you can
        skip passing it downstream.
        """
        # Toggle collapse on title bar click
        if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
            if self._title_rect.collidepoint(event.pos):
                self._collapsed = not self._collapsed
                self._do_layout()
                return True

        if self._collapsed:
            return False

        consumed = False
        for slider in self._sliders.values():
            if slider.handle_event(event):
                consumed = True
                break

        if not consumed:
            if self._random_size_cb.handle_event(event):
                consumed = True
            elif self._clear_btn.handle_event(event):
                consumed = True
            elif self._reset_btn.handle_event(event):
                consumed = True

        # Keep Spawn X range in sync whenever Board Length changes
        self._update_dynamic_ranges()
        return consumed

    def was_clear_pressed(self) -> bool:
        """True for exactly one frame after "Clear Balls" is clicked."""
        return self._clear_btn.pressed

    def was_reset_pressed(self) -> bool:
        """True for exactly one frame after "Reset Everything" is clicked."""
        return self._reset_btn.pressed

    def reset_to_defaults(self) -> None:
        """Restore all sliders to their default values."""
        for key, slider in self._sliders.items():
            slider.value = _DEFAULTS[key]
        self._random_size_cb.checked = _DEFAULTS["random_size"]

    def get_values(self) -> dict:
        """
        Return a snapshot of every tunable parameter.

        Keys match the Rust variable names from main.rs for easy cross-reference.

        {
            "balls_per_sec":    float   # spawn rate (balls / second)
            "spawn_x":          float   # x-position balls spawn above
            "board_length":     float   # branch linear length in pixels
            "gravity":          float   # downward acceleration (px/s²)
            "gloopification":   float   # friction loss per tick (0..0.05)
            "density":          float   # ball density (mass = π·r²·density)
            "flubberization":   float   # restitution coefficient
            "stiffness":        float   # branch constraint stiffness (0..1)
            "ball_size":        float   # radius when random_size is False
            "wobble_threshold": float   # mean curvature radius that triggers wobble
            "random_size":      bool    # randomise ball radius on spawn
        }
        """
        return {
            "balls_per_sec":    self._sliders["balls_per_sec"].value,
            "spawn_x":          self._sliders["spawn_x"].value,
            "board_length":     self._sliders["board_length"].value,
            "gravity":          self._sliders["gravity"].value,
            "gloopification":   self._sliders["gloopification"].value,
            "density":          self._sliders["density"].value,
            "flubberization":   self._sliders["flubberization"].value,
            "stiffness":        self._sliders["stiffness"].value,
            "ball_size":        self._sliders["ball_size"].value,
            "wobble_threshold": self._sliders["wobble_threshold"].value,
            "random_size":      self._random_size_cb.checked,
        }

    def draw(self, surface: pygame.Surface) -> None:
        """Render the panel onto *surface*. Call after drawing your scene."""
        # Panel background + border
        pygame.draw.rect(surface, _BG, self._panel_rect, border_radius=6)
        pygame.draw.rect(surface, _BORDER, self._panel_rect, 1, border_radius=6)

        # Title bar
        pygame.draw.rect(surface, _TITLE_BG, self._title_rect,
                         border_radius=(6 if self._collapsed else 0))
        arrow  = "▶" if self._collapsed else "▼"
        title  = f"{arrow}  Simulation Controls"
        t_surf = self._font_md.render(title, True, _TITLE_TEXT)
        surface.blit(t_surf, (
            self._title_rect.x + self.PADDING,
            self._title_rect.centery - t_surf.get_height() // 2,
        ))

        if self._collapsed:
            return

        for slider in self._sliders.values():
            slider.draw(surface, self._font_sm, self._font_md)
        self._random_size_cb.draw(surface, self._font_sm, self._font_md)
        self._clear_btn.draw(surface, self._font_sm, self._font_md)
        self._reset_btn.draw(surface, self._font_sm, self._font_md)
