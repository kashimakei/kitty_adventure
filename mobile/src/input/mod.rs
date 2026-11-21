//! Keyboard, mouse and touch, turned into [`Controls`] once per frame.
//!
//! Every input event is recorded in order between frames (through macroquad's input subscriber)
//! and replayed at the top of the next one, so nothing is lost when several arrive together: a
//! quick tap still counts even if its down and up land in the same frame. The touch gestures
//! themselves are worked out in [`touch`].

pub mod touch;

use macroquad::input::utils::{register_input_subscriber, repeat_all_miniquad_input};
use macroquad::miniquad::{date, EventHandler, KeyCode, KeyMods, MouseButton, TouchPhase};
use macroquad::prelude::*;

use crate::camera::View;
use crate::platform;
use touch::{MarkKind, Phase, Recogniser, Thresholds, MARK_LIFE, MOUSE_ID};

/// What the player is doing this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    /// -1..1: walk or run left or right (the slider, or the arrow keys; Shift walks).
    pub move_x: f32,
    /// The jump finger or key went down since the last step.
    pub press: bool,
    /// The jump finger or key is held (charging); false once the finger swiped.
    pub charge_held: bool,
    /// The jump finger or key was released since the last step (not after a swipe).
    pub leap: bool,
    /// Swiped down, or Down/S: let go of a branch or drop through it.
    pub drop: bool,
    pub pause: bool,
    /// Android's Back button or gesture.
    pub back: bool,
    /// A tap or a continue key, for "tap to continue" screens.
    pub any_tap: bool,
}

/// The one-off parts of [`Controls`], kept from the frame they happen until a physics step uses
/// them: some frames run no step and some run several.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pending {
    pub press: bool,
    pub leap: bool,
    pub drop: bool,
}

impl Pending {
    pub fn add(&mut self, controls: &Controls) {
        self.press |= controls.press;
        self.leap |= controls.leap;
        self.drop |= controls.drop;
    }

    /// Controls for one physics step: what's held now plus every edge since the last step. A
    /// press and a leap together are a quick tap (the press came first).
    pub fn take(&mut self, controls: &Controls) -> Controls {
        let Pending { press, leap, drop } = std::mem::take(self);
        Controls { press, leap, drop, ..*controls }
    }
}

/// Logical screen pixels per millimetre where the platform doesn't say: a desktop monitor
/// (96 dpi), and a phone browser's CSS pixels once a touch shows it's a phone.
const DESKTOP_PX_PER_MM: f32 = 3.78;
const WEB_TOUCH_PX_PER_MM: f32 = 6.3;
/// A typical Android phone, in case the activity can't be asked.
const ANDROID_PX_PER_MM: f32 = 16.5;
/// Shift with the arrow keys walks instead of running.
const KEY_WALK_SPEED: f32 = 0.45;
/// A gap this long between frames means the app was hidden (a browser tab just stops drawing).
const AWAY_FRAME_TIME: f32 = 1.0;

/// The pause button, top right in playfield coordinates.
const PAUSE_CENTRE: Vec2 = Vec2::from_array([572.0, 92.0]);
const PAUSE_RADIUS: f32 = 24.0;
/// The slider's knob, and the track's thickness.
const KNOB_RADIUS: f32 = 34.0;
const TRACK_THICKNESS: f32 = 12.0;

/// An input event as it arrived, positions in raw window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Raw {
    Touch { id: u64, phase: Phase, x: f32, y: f32 },
    Key { code: KeyCode, down: bool, repeat: bool },
    Minimized,
}

/// Collects input events in order between frames; the mouse plays a finger. Public so the tests
/// can drive it.
#[derive(Default)]
pub struct Recorder {
    pub events: Vec<Raw>,
    mouse_down: bool,
    mouse_at: Vec2,
    pub touch_seen: bool,
}

impl Recorder {
    fn mouse(&mut self, phase: Phase, x: f32, y: f32) {
        self.mouse_at = vec2(x, y);
        self.events.push(Raw::Touch { id: MOUSE_ID, phase, x, y });
    }
}

impl EventHandler for Recorder {
    fn update(&mut self) {}
    fn draw(&mut self) {}

    fn touch_event(&mut self, phase: TouchPhase, id: u64, x: f32, y: f32) {
        self.touch_seen = true;
        let phase = match phase {
            TouchPhase::Started => Phase::Down,
            TouchPhase::Moved => Phase::Move,
            TouchPhase::Ended => Phase::Up,
            TouchPhase::Cancelled => Phase::Cancel,
        };
        self.events.push(Raw::Touch { id, phase, x, y });
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button == MouseButton::Left {
            self.mouse_down = true;
            self.mouse(Phase::Down, x, y);
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        if self.mouse_down {
            self.mouse(Phase::Move, x, y);
        }
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button == MouseButton::Left && self.mouse_down {
            self.mouse_down = false;
            self.mouse(Phase::Up, x, y);
        }
    }

    fn key_down_event(&mut self, code: KeyCode, _mods: KeyMods, repeat: bool) {
        self.events.push(Raw::Key { code, down: true, repeat });
    }

    fn key_up_event(&mut self, code: KeyCode, _mods: KeyMods) {
        self.events.push(Raw::Key { code, down: false, repeat: false });
    }

    fn window_minimized_event(&mut self) {
        self.mouse_down = false;
        self.events.push(Raw::Minimized);
    }
}

/// The keyboard's jump key, with the same press/hold/release edges as the action finger.
#[derive(Default)]
struct Keys {
    jump_held: Vec<KeyCode>,
    /// The jump key was already down when the screen changed: it does nothing until let go.
    inert: bool,
    press: bool,
    leap: bool,
    drop: bool,
    pause: bool,
    back: bool,
    any_tap: bool,
}

impl Keys {
    fn event(&mut self, code: KeyCode, down: bool, repeat: bool) {
        let jump_key = matches!(code, KeyCode::Space | KeyCode::Up | KeyCode::W);
        if !down {
            if jump_key && self.jump_held.contains(&code) {
                self.jump_held.retain(|&k| k != code);
                if self.jump_held.is_empty() {
                    self.leap |= !self.inert;
                    self.inert = false;
                }
            }
            return;
        }
        if repeat {
            return;
        }
        if jump_key && !self.jump_held.contains(&code) {
            self.press |= self.jump_held.is_empty() && !self.inert;
            self.jump_held.push(code);
        }
        match code {
            KeyCode::Down | KeyCode::S => self.drop = true,
            KeyCode::Escape | KeyCode::P => self.pause = true,
            KeyCode::Back => self.back = true,
            _ => {}
        }
        // Only the keys that mean "go on": Back, volume and other system keys arrive as key
        // presses too and mustn't skip screens
        self.any_tap |= matches!(code, KeyCode::Enter | KeyCode::KpEnter | KeyCode::Space);
    }

    /// Keys held when the app was put away never send their release.
    fn release_all(&mut self) {
        self.jump_held.clear();
        self.inert = false;
    }

    fn ignore_current(&mut self) {
        self.inert = !self.jump_held.is_empty();
        self.press = false;
        self.leap = false;
        self.drop = false;
    }

    fn take(&mut self) -> Controls {
        let left = is_key_down(KeyCode::Left) || is_key_down(KeyCode::A);
        let right = is_key_down(KeyCode::Right) || is_key_down(KeyCode::D);
        let walk = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let speed = if walk { KEY_WALK_SPEED } else { 1.0 };
        let controls = Controls {
            move_x: (right as i32 - left as i32) as f32 * speed,
            press: self.press,
            charge_held: !self.jump_held.is_empty() && !self.inert,
            leap: self.leap,
            drop: self.drop,
            pause: self.pause,
            back: self.back,
            any_tap: self.any_tap,
        };
        let (jump_held, inert) = (std::mem::take(&mut self.jump_held), self.inert);
        *self = Keys { jump_held, inert, ..Keys::default() };
        controls
    }
}

/// All the player's input, polled once per frame.
pub struct Input {
    subscriber: usize,
    recorder: Recorder,
    touch: Recogniser,
    keys: Keys,
    /// The display's density, from the platform where it can tell.
    device_px_per_mm: Option<f32>,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    pub fn new() -> Self {
        Self {
            subscriber: register_input_subscriber(),
            recorder: Recorder::default(),
            touch: Recogniser::new(Thresholds::new(1.0)),
            keys: Keys::default(),
            device_px_per_mm: platform::pixels_per_mm(),
        }
    }

    /// Replay the input since the last frame. Returns the controls and whether the app was away
    /// (put in the background, or a browser tab hidden), so play can pause.
    pub fn poll(&mut self, view: &View) -> (Controls, bool) {
        repeat_all_miniquad_input(&mut self.recorder, self.subscriber);
        // A drag that left the browser window never gets its mouse-up: end it here instead
        if self.recorder.mouse_down && !is_mouse_button_down(MouseButton::Left) {
            let at = self.recorder.mouse_at;
            self.recorder.mouse_down = false;
            self.recorder.mouse(Phase::Cancel, at.x, at.y);
        }
        self.touch.thresholds = Thresholds::new(self.px_per_mm() / view.scale);
        let now = date::now();
        self.touch.frame(now);
        let dpi = screen_dpi_scale();
        let mut away = false;
        for event in self.recorder.events.drain(..) {
            match event {
                Raw::Touch { id, phase, x, y } => {
                    self.touch.event(id, phase, view.to_playfield(vec2(x, y) / dpi), now);
                }
                Raw::Key { code, down, repeat } => self.keys.event(code, down, repeat),
                Raw::Minimized => {
                    self.touch.release_all();
                    self.keys.release_all();
                    away = true;
                }
            }
        }
        let keys = self.keys.take();
        let fingers = self.touch.take();
        let controls = Controls {
            move_x: (keys.move_x + fingers.move_x).clamp(-1.0, 1.0),
            press: keys.press || fingers.press,
            charge_held: keys.charge_held || fingers.charge_held,
            leap: keys.leap || fingers.leap,
            drop: keys.drop || fingers.drop,
            pause: keys.pause || fingers.pause,
            back: keys.back,
            any_tap: keys.any_tap || fingers.any_tap,
        };
        (controls, away || get_frame_time() >= AWAY_FRAME_TIME)
    }

    /// The screen changed: fingers and the jump key already down do nothing until let go.
    pub fn ignore_current(&mut self) {
        self.touch.ignore_current();
        self.keys.ignore_current();
    }

    /// Whether the player is on a touch screen (for showing gestures rather than keys).
    pub fn is_touch(&self) -> bool {
        cfg!(target_os = "android") || self.recorder.touch_seen
    }

    fn px_per_mm(&self) -> f32 {
        match self.device_px_per_mm {
            Some(known) => known,
            None if cfg!(target_os = "android") => ANDROID_PX_PER_MM,
            None if self.recorder.touch_seen => WEB_TOUCH_PX_PER_MM,
            None => DESKTOP_PX_PER_MM,
        }
    }

    /// Draw the slider while a thumb holds it, brief rings and chevrons acknowledging the
    /// action finger, and the pause button (in the screen camera).
    pub fn draw(&self) {
        if let Some(slider) = self.touch.slider() {
            let full = self.touch.thresholds.full_speed;
            let (x, y) = (slider.origin.x, slider.origin.y);
            draw_rectangle(x - full, y - TRACK_THICKNESS / 2.0, full * 2.0, TRACK_THICKNESS, Color::new(1.0, 1.0, 1.0, 0.16));
            let knob = x + (slider.finger.x - x).clamp(-full, full);
            draw_circle(knob, y, KNOB_RADIUS, Color::new(1.0, 1.0, 1.0, 0.35));
            draw_circle_lines(knob, y, KNOB_RADIUS, 2.0, Color::new(0.15, 0.15, 0.15, 0.3));
        }
        let now = date::now();
        for mark in self.touch.marks() {
            let age = ((now - mark.time) / MARK_LIFE) as f32;
            if !(0.0..1.0).contains(&age) {
                continue;
            }
            let color = Color::new(1.0, 1.0, 1.0, 0.55 * (1.0 - age));
            let at = mark.at;
            match mark.kind {
                MarkKind::Landing => draw_circle_lines(at.x, at.y, 20.0 + 22.0 * age, 3.0, color),
                MarkKind::Swipe => {
                    // A downward chevron that slips a little further down as it fades
                    let tip = at + vec2(0.0, 10.0 + 16.0 * age);
                    for side in [-1.0, 1.0] {
                        draw_line(tip.x + 18.0 * side, tip.y - 14.0, tip.x, tip.y, 4.0, color);
                    }
                }
            }
        }
        draw_circle(PAUSE_CENTRE.x, PAUSE_CENTRE.y, PAUSE_RADIUS, Color::new(1.0, 1.0, 1.0, 0.35));
        for dx in [-6.0, 6.0] {
            draw_rectangle(PAUSE_CENTRE.x + dx - 3.0, PAUSE_CENTRE.y - 9.0, 6.0, 18.0, Color::new(0.1, 0.1, 0.1, 0.7));
        }
    }
}
