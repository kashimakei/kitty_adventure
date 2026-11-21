//! Touch gestures as pure logic: finger events in playfield coordinates with timestamps go in, the
//! player's intent comes out. No GL or window calls, so the tests drive it headless.
//!
//! A finger's job is fixed the moment it lands: the corner by the pause button pauses, the left
//! half is a floating slider for walking and running, and the right half is the action finger
//! (hold to charge, release to leap, swipe down to drop). It keeps that job wherever it wanders.
//! Every edge is latched as its event arrives, so a tap whose down and up land in the same frame,
//! or a down followed by a move before the game looks, still counts.

use macroquad::math::Vec2;

/// The finger id the mouse plays on desktop and web.
pub const MOUSE_ID: u64 = u64::MAX;

/// A finger landing at x of at least this and y of at most this (the letterbox beyond included)
/// pauses: the pause button's corner, generously.
pub const PAUSE_ZONE_X: f32 = 540.0;
pub const PAUSE_ZONE_Y: f32 = 130.0;
/// Fingers landing left of this drive the slider; the rest are action fingers.
pub const SLIDER_ZONE_X: f32 = 300.0;

/// The slider ignores the first 1.5 mm (a resting thumb wobbles) and reaches full speed at 15 mm.
pub const DEAD_ZONE_MM: f32 = 1.5;
pub const FULL_SPEED_MM: f32 = 15.0;
/// A swipe down: at least 7 mm within 150 ms, and steeper than it is wide by this factor.
pub const SWIPE_MM: f32 = 7.0;
pub const SWIPE_WINDOW: f64 = 0.150;
pub const SWIPE_STEEPNESS: f32 = 1.2;
/// Just past the dead zone Kitty already walks at this fraction of full speed, so the slider
/// never feels like it does nothing.
const MIN_SPEED: f32 = 0.2;
/// Speeds this close to full count as full, so running doesn't need the thumb at the very end.
const SNAP_TO_FULL: f32 = 0.95;
/// The slider's centre trails a thumb that goes further than this many full-speed distances
/// from it, so sliding back always slows down promptly.
const TRAIL: f32 = 1.2;
/// How long the landing ring and swipe chevron stay on screen, in seconds.
pub const MARK_LIFE: f64 = 0.25;

/// Sane bounds for each threshold in playfield units, whatever density the platform reports.
const DEAD_ZONE_RANGE: (f32, f32) = (4.0, 24.0);
const FULL_SPEED_RANGE: (f32, f32) = (50.0, 180.0);
const SWIPE_RANGE: (f32, f32) = (28.0, 110.0);

/// The gesture distances in playfield units for the current screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    pub dead_zone: f32,
    pub full_speed: f32,
    pub swipe: f32,
}

impl Thresholds {
    /// Thresholds for a screen showing `units_per_mm` playfield units per millimetre. Nonsense
    /// (zero, infinite or NaN) lands on the bounds rather than breaking the controls.
    pub fn new(units_per_mm: f32) -> Self {
        let size = |mm: f32, (lo, hi): (f32, f32)| (mm * units_per_mm).max(lo).min(hi);
        Self {
            dead_zone: size(DEAD_ZONE_MM, DEAD_ZONE_RANGE),
            full_speed: size(FULL_SPEED_MM, FULL_SPEED_RANGE),
            swipe: size(SWIPE_MM, SWIPE_RANGE),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Down,
    Move,
    Up,
    /// The system took the touch (a gesture, a dialog): it ends without counting as a release.
    Cancel,
}

/// What the fingers did since the last [`Recogniser::take`], plus what they're doing now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gestures {
    pub move_x: f32,
    pub press: bool,
    pub charge_held: bool,
    pub leap: bool,
    pub drop: bool,
    pub pause: bool,
    pub any_tap: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Pause,
    Slider,
    Action,
    /// Superseded, swiped, or already down when the screen changed: ignored until lifted.
    Inert,
}

struct Finger {
    id: u64,
    role: Role,
    /// Recent positions of an action finger, oldest first, for spotting a swipe.
    trail: Vec<(f64, Vec2)>,
}

/// The slider while a thumb holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slider {
    /// The centre: where the thumb landed, dragged along if the thumb goes far past full speed.
    pub origin: Vec2,
    pub finger: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    /// Where an action finger landed.
    Landing,
    /// Where a swipe down let go.
    Swipe,
}

/// A brief on-screen acknowledgement of a gesture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub kind: MarkKind,
    pub at: Vec2,
    pub time: f64,
}

/// Turns finger events into [`Gestures`].
pub struct Recogniser {
    pub thresholds: Thresholds,
    fingers: Vec<Finger>,
    slider: Option<Slider>,
    /// Edges since the last take (the held fields are worked out when taken).
    edges: Gestures,
    marks: Vec<Mark>,
    /// When the previous frame's events were replayed (see [`Recogniser::frame`]).
    last_frame: Option<f64>,
    /// The earliest a sample can be stamped and still count toward a swipe in this frame.
    swipe_since: f64,
}

impl Recogniser {
    pub fn new(thresholds: Thresholds) -> Self {
        Self {
            thresholds,
            fingers: Vec::new(),
            slider: None,
            edges: Gestures::default(),
            marks: Vec::new(),
            last_frame: None,
            swipe_since: f64::INFINITY,
        }
    }

    /// A new frame's events are about to be replayed, all stamped `now`. They really happened
    /// some time since the last frame, so a sample from the last frame may be a frame older than
    /// its stamp says: the swipe window stretches back to cover it. At 60 Hz that's a few
    /// milliseconds; when frames hitch, a quick flick split across them still counts.
    pub fn frame(&mut self, now: f64) {
        self.swipe_since = self.last_frame.unwrap_or(now).min(now) - SWIPE_WINDOW;
        self.last_frame = Some(now);
    }

    /// One finger event, in the order they happened. `pos` is in playfield coordinates and
    /// `time` in seconds.
    pub fn event(&mut self, id: u64, phase: Phase, pos: Vec2, time: f64) {
        match phase {
            Phase::Down => self.down(id, pos, time),
            Phase::Move => self.moved(id, pos, time),
            Phase::Up => self.lift(id, pos, time, true),
            Phase::Cancel => self.lift(id, pos, time, false),
        }
    }

    /// The app was put away: every finger is gone, and none of them counts as a release.
    pub fn release_all(&mut self) {
        self.fingers.clear();
        self.slider = None;
    }

    /// The screen changed: fingers already down do nothing until they're lifted, so a tap that
    /// dismissed a menu can't carry on into play.
    pub fn ignore_current(&mut self) {
        for finger in &mut self.fingers {
            finger.role = Role::Inert;
            finger.trail.clear();
        }
        self.slider = None;
        self.edges = Gestures::default();
    }

    /// The edges since the last take, and what's held now.
    pub fn take(&mut self) -> Gestures {
        Gestures {
            move_x: self.move_x(),
            charge_held: self.fingers.iter().any(|f| f.role == Role::Action),
            ..std::mem::take(&mut self.edges)
        }
    }

    pub fn slider(&self) -> Option<Slider> {
        self.slider
    }

    /// Recent landing rings and swipe chevrons (some may have expired; check their time).
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    fn down(&mut self, id: u64, pos: Vec2, time: f64) {
        // A down for a finger we think is still down means its lift went missing: forget it
        // quietly rather than count a release nobody made
        if self.fingers.iter().any(|f| f.id == id) {
            self.forget(id);
        }
        self.edges.any_tap = true;
        let role = if pos.x >= PAUSE_ZONE_X && pos.y <= PAUSE_ZONE_Y {
            self.edges.pause = true;
            Role::Pause
        } else if pos.x < SLIDER_ZONE_X {
            // The newest thumb on the left takes over the slider, centred where it lands
            self.supersede(Role::Slider);
            self.slider = Some(Slider { origin: pos, finger: pos });
            Role::Slider
        } else {
            self.supersede(Role::Action);
            self.edges.press = true;
            self.mark(MarkKind::Landing, pos, time);
            Role::Action
        };
        let trail = if role == Role::Action { vec![(time, pos)] } else { Vec::new() };
        self.fingers.push(Finger { id, role, trail });
    }

    fn moved(&mut self, id: u64, pos: Vec2, time: f64) {
        let Some(index) = self.fingers.iter().position(|f| f.id == id) else { return };
        match self.fingers[index].role {
            Role::Slider => {
                let reach = TRAIL * self.thresholds.full_speed;
                if let Some(slider) = &mut self.slider {
                    slider.finger = pos;
                    let dx = pos.x - slider.origin.x;
                    if dx.abs() > reach {
                        slider.origin.x = pos.x - reach.copysign(dx);
                    }
                }
            }
            Role::Action => self.track(index, pos, time),
            Role::Pause | Role::Inert => {}
        }
    }

    fn lift(&mut self, id: u64, pos: Vec2, time: f64, released: bool) {
        let Some(index) = self.fingers.iter().position(|f| f.id == id) else { return };
        if self.fingers[index].role == Role::Action && released {
            // The last stretch can finish a swipe; otherwise letting go leaps
            self.track(index, pos, time);
            if self.fingers[index].role == Role::Action {
                self.edges.leap = true;
            }
        }
        self.forget(id);
    }

    /// Follow an action finger, and turn it into a drop if it just swiped down.
    fn track(&mut self, index: usize, pos: Vec2, time: f64) {
        let swipe = self.thresholds.swipe;
        let since = self.swipe_since.min(time - SWIPE_WINDOW);
        let finger = &mut self.fingers[index];
        // Keep the window's worth of positions, plus the last one before it: the finger was
        // still there when the window opened
        let before = finger.trail.iter().rposition(|&(t, _)| t < since).unwrap_or(0);
        finger.trail.drain(..before);
        let swiped = finger.trail.iter().any(|&(_, from)| {
            let d = pos - from;
            d.y >= swipe && d.y >= SWIPE_STEEPNESS * d.x.abs()
        });
        finger.trail.push((time, pos));
        if swiped {
            // One drop per swipe, the charge is cancelled, and lifting won't leap
            finger.role = Role::Inert;
            finger.trail.clear();
            self.edges.drop = true;
            self.mark(MarkKind::Swipe, pos, time);
        }
    }

    /// A new finger is taking `role`: the one that had it stops counting until lifted.
    fn supersede(&mut self, role: Role) {
        for finger in self.fingers.iter_mut().filter(|f| f.role == role) {
            finger.role = Role::Inert;
            finger.trail.clear();
        }
        if role == Role::Slider {
            self.slider = None;
        }
    }

    fn forget(&mut self, id: u64) {
        if let Some(index) = self.fingers.iter().position(|f| f.id == id) {
            if self.fingers[index].role == Role::Slider {
                self.slider = None;
            }
            self.fingers.remove(index);
        }
    }

    fn mark(&mut self, kind: MarkKind, at: Vec2, time: f64) {
        self.marks.retain(|m| time - m.time < MARK_LIFE);
        self.marks.push(Mark { kind, at, time });
    }

    /// -1..1 from the slider: nothing inside the dead zone, then from MIN_SPEED up to full.
    fn move_x(&self) -> f32 {
        let Some(slider) = self.slider else { return 0.0 };
        let Thresholds { dead_zone, full_speed, .. } = self.thresholds;
        let dx = slider.finger.x - slider.origin.x;
        if dx.abs() <= dead_zone {
            return 0.0;
        }
        let along = ((dx.abs() - dead_zone) / (full_speed - dead_zone)).min(1.0);
        let speed = MIN_SPEED + (1.0 - MIN_SPEED) * along;
        if speed >= SNAP_TO_FULL { 1.0f32.copysign(dx) } else { speed.copysign(dx) }
    }
}
