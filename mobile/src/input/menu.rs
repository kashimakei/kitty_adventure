//! Menu gestures as pure logic, worked out alongside the play gestures in [`super::touch`] from
//! the same events: a tap (down and up close together) picks a button or a level, a drag scrolls
//! the map and a fling keeps it gliding. Only one finger counts. Keys move a focus between
//! buttons and press it, for desktop and web.

use macroquad::math::{Rect, Vec2};
use macroquad::miniquad::KeyCode;

use super::touch::Phase;

/// A finger that strays further than this between down and up is dragging, not tapping.
pub const SLOP_MM: f32 = 4.0;
/// Sane bounds for it in playfield units, whatever density the platform reports.
const SLOP_RANGE: (f32, f32) = (12.0, 40.0);
/// A fling's speed is measured over the end of the drag, this long (s).
const FLING_WINDOW: f64 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuThresholds {
    pub slop: f32,
}

impl MenuThresholds {
    /// For a screen showing `units_per_mm` playfield units per millimetre; nonsense lands on the
    /// bounds.
    pub fn new(units_per_mm: f32) -> Self {
        Self { slop: (SLOP_MM * units_per_mm).max(SLOP_RANGE.0).min(SLOP_RANGE.1) }
    }
}

/// A tap: where the finger went down and where it came up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tap {
    pub down: Vec2,
    pub up: Vec2,
}

/// A key's meaning in a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Activate,
    Back,
    PageUp,
    PageDown,
}

/// What the player did to the menus since the last frame, plus what they're pointing at now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MenuInput {
    pub tap: Option<Tap>,
    /// How far a drag moved down this frame (playfield units; up is negative).
    pub drag_dy: f32,
    /// A drag let go while moving: its speed down (units per second).
    pub fling: Option<f32>,
    /// Where the finger (or held mouse) is now, while it might still be a tap.
    pub held: Option<Vec2>,
    /// Where the mouse is with no button down (desktop and web).
    pub hover: Option<Vec2>,
    /// Mouse wheel turned this frame (positive away from the player).
    pub wheel: f32,
    pub nav: Option<Nav>,
}

impl MenuInput {
    /// Tapped inside `r`: both where the finger went down and where it came up.
    pub fn tapped(&self, r: Rect) -> bool {
        self.tap.is_some_and(|tap| r.contains(tap.down) && r.contains(tap.up))
    }

    /// A finger held on `r`, or the mouse over it.
    pub fn pointing_at(&self, r: Rect) -> bool {
        self.held.or(self.hover).is_some_and(|p| r.contains(p))
    }
}

struct Finger {
    id: u64,
    down: Vec2,
    at: Vec2,
    dragging: bool,
    /// Recent positions while dragging, oldest first, for the fling.
    trail: Vec<(f64, Vec2)>,
}

/// Turns finger events into [`MenuInput`].
pub struct MenuRecogniser {
    pub thresholds: MenuThresholds,
    finger: Option<Finger>,
    /// Fingers ignored until lifted: a second finger, or one already down when the screen
    /// changed (so a finger still on the screen when hearts run out can't press Retry).
    inert: Vec<u64>,
    out: MenuInput,
}

impl MenuRecogniser {
    pub fn new(thresholds: MenuThresholds) -> Self {
        Self { thresholds, finger: None, inert: Vec::new(), out: MenuInput::default() }
    }

    /// One finger event, in the order they happened, in playfield coordinates; `time` in seconds.
    pub fn event(&mut self, id: u64, phase: Phase, pos: Vec2, time: f64) {
        match phase {
            Phase::Down => {
                if self.finger.as_ref().is_some_and(|f| f.id == id) {
                    // Its lift went missing: start again
                    self.finger = None;
                }
                if self.finger.is_some() {
                    self.inert.push(id);
                    return;
                }
                self.inert.retain(|&i| i != id);
                self.finger = Some(Finger { id, down: pos, at: pos, dragging: false, trail: vec![(time, pos)] });
            }
            Phase::Move => {
                let slop = self.thresholds.slop;
                let Some(finger) = self.finger.as_mut().filter(|f| f.id == id) else { return };
                if !finger.dragging && finger.down.distance(pos) > slop {
                    // Catch up with everywhere it went since it landed
                    finger.dragging = true;
                    self.out.drag_dy += pos.y - finger.down.y;
                } else if finger.dragging {
                    self.out.drag_dy += pos.y - finger.at.y;
                }
                finger.at = pos;
                finger.trail.push((time, pos));
                let since = time - FLING_WINDOW;
                // Keep the window's worth, plus the last one before it
                let before = finger.trail.iter().rposition(|&(t, _)| t < since).unwrap_or(0);
                finger.trail.drain(..before);
            }
            Phase::Up | Phase::Cancel => {
                if let Some(k) = self.inert.iter().position(|&i| i == id) {
                    self.inert.swap_remove(k);
                    return;
                }
                let Some(finger) = self.finger.take_if(|f| f.id == id) else { return };
                if phase == Phase::Cancel {
                    return;
                }
                if !finger.dragging && finger.down.distance(pos) <= self.thresholds.slop {
                    self.out.tap = Some(Tap { down: finger.down, up: pos });
                } else {
                    let caught_up = if finger.dragging { finger.at } else { finger.down };
                    self.out.drag_dy += pos.y - caught_up.y;
                    // Speed over the end of the drag, from the oldest sample in the window that
                    // came in an earlier frame (samples in one frame share a time)
                    let since = time - FLING_WINDOW;
                    if let Some(&(t0, p0)) = finger.trail.iter().find(|&&(t, _)| t >= since && t < time)
                        .or_else(|| finger.trail.iter().rev().find(|&&(t, _)| t < time)) {
                        self.out.fling = Some((pos.y - p0.y) / (time - t0) as f32);
                    }
                }
            }
        }
    }

    /// The screen changed: a finger already down is ignored until lifted.
    pub fn ignore_current(&mut self) {
        if let Some(finger) = self.finger.take() {
            self.inert.push(finger.id);
        }
        self.out = MenuInput::default();
    }

    /// The app was put away: every finger is gone.
    pub fn release_all(&mut self) {
        self.finger = None;
        self.inert.clear();
    }

    /// What happened since the last take, and where the finger is now.
    pub fn take(&mut self) -> MenuInput {
        let held = self.finger.as_ref().filter(|f| !f.dragging).map(|f| f.at);
        MenuInput { held, ..std::mem::take(&mut self.out) }
    }
}

/// Keys in menus: arrows (or WASD) and Tab move the focus, Enter or Space press it, Escape or
/// Back go back, Page Up and Down scroll. Held keys repeat.
#[derive(Default)]
pub struct MenuKeys {
    nav: Option<Nav>,
}

impl MenuKeys {
    pub fn event(&mut self, code: KeyCode, down: bool, _repeat: bool) {
        if !down {
            return;
        }
        let nav = match code {
            KeyCode::Up | KeyCode::W => Nav::Up,
            KeyCode::Down | KeyCode::S | KeyCode::Tab => Nav::Down,
            KeyCode::Left | KeyCode::A => Nav::Left,
            KeyCode::Right | KeyCode::D => Nav::Right,
            KeyCode::Enter | KeyCode::KpEnter | KeyCode::Space => Nav::Activate,
            KeyCode::Escape | KeyCode::Back => Nav::Back,
            KeyCode::PageUp => Nav::PageUp,
            KeyCode::PageDown => Nav::PageDown,
            _ => return,
        };
        self.nav = Some(nav);
    }

    pub fn ignore_current(&mut self) {
        self.nav = None;
    }

    pub fn take(&mut self) -> Option<Nav> {
        self.nav.take()
    }
}
