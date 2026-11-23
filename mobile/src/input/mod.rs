//! Keyboard, mouse and touch, turned into [`Controls`] once per frame.
//!
//! Every input event is recorded in order between frames (through macroquad's input subscriber)
//! and replayed at the top of the next one, so nothing is lost when several arrive together: a
//! quick tap still counts even if its down and up land in the same frame. The touch gestures
//! themselves are worked out in [`touch`] for play and in [`menu`] for the menus and map.

pub mod menu;
pub mod touch;

use macroquad::input::utils::{register_input_subscriber, repeat_all_miniquad_input};
use macroquad::miniquad::{date, EventHandler, KeyCode, KeyMods, MouseButton, TouchPhase};
use macroquad::prelude::*;

use crate::camera::View;
use crate::platform;
use menu::{MenuInput, MenuKeys, MenuRecogniser, MenuThresholds};
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
const KNOB_RADIUS: f32 = 30.0;
const TRACK_THICKNESS: f32 = 12.0;
/// The arrows just past either end of the track.
const ARROW_GAP: f32 = 8.0;
const ARROW_SIZE: f32 = 10.0;

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
    /// Where the mouse is (raw window pixels), for menus to light up what it's over. Not an
    /// event: play never sees it.
    pub hover: Option<Vec2>,
    /// Mouse wheel turned since the last frame.
    pub wheel: f32,
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
        self.hover = Some(vec2(x, y));
        if self.mouse_down {
            self.mouse(Phase::Move, x, y);
        }
    }

    fn mouse_wheel_event(&mut self, _x: f32, y: f32) {
        self.wheel += y;
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

/// The keyboard's jump key, with the same press/hold/release edges as the action finger. Public
/// so the tests can drive it.
#[derive(Default)]
pub struct Keys {
    /// Every key down now. A key's auto-repeat (which Android sends as fresh presses, with no
    /// repeat flag) is ignored until it's let go.
    down: Vec<KeyCode>,
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
    pub fn event(&mut self, code: KeyCode, down: bool, repeat: bool) {
        let jump_key = matches!(code, KeyCode::Space | KeyCode::Up | KeyCode::W);
        if !down {
            self.down.retain(|&k| k != code);
            if jump_key && self.jump_held.contains(&code) {
                self.jump_held.retain(|&k| k != code);
                if self.jump_held.is_empty() {
                    self.leap |= !self.inert;
                    self.inert = false;
                }
            }
            return;
        }
        if repeat || self.down.contains(&code) {
            return;
        }
        self.down.push(code);
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
    pub fn release_all(&mut self) {
        self.down.clear();
        self.jump_held.clear();
        self.inert = false;
    }

    pub fn ignore_current(&mut self) {
        self.inert = !self.jump_held.is_empty();
        self.press = false;
        self.leap = false;
        self.drop = false;
    }

    /// This frame's controls from the keyboard, with walking read from the keys down now.
    fn take(&mut self) -> Controls {
        let left = is_key_down(KeyCode::Left) || is_key_down(KeyCode::A);
        let right = is_key_down(KeyCode::Right) || is_key_down(KeyCode::D);
        let walk = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let speed = if walk { KEY_WALK_SPEED } else { 1.0 };
        Controls { move_x: (right as i32 - left as i32) as f32 * speed, ..self.take_edges() }
    }

    /// This frame's presses, releases, drops and the rest (everything but walking), cleared for
    /// the next.
    pub fn take_edges(&mut self) -> Controls {
        let controls = Controls {
            move_x: 0.0,
            press: self.press,
            charge_held: !self.jump_held.is_empty() && !self.inert,
            leap: self.leap,
            drop: self.drop,
            pause: self.pause,
            back: self.back,
            any_tap: self.any_tap,
        };
        let (down, jump_held, inert) = (std::mem::take(&mut self.down), std::mem::take(&mut self.jump_held), self.inert);
        *self = Keys { down, jump_held, inert, ..Keys::default() };
        controls
    }
}

/// All the player's input, polled once per frame.
pub struct Input {
    subscriber: usize,
    recorder: Recorder,
    touch: Recogniser,
    keys: Keys,
    menu: MenuRecogniser,
    menu_keys: MenuKeys,
    /// This frame's menu input, worked out by `poll`.
    menu_input: MenuInput,
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
            menu: MenuRecogniser::new(MenuThresholds::new(1.0)),
            menu_keys: MenuKeys::default(),
            menu_input: MenuInput::default(),
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
        self.menu.thresholds = MenuThresholds::new(self.px_per_mm() / view.scale);
        let now = date::now();
        self.touch.frame(now);
        // A long gap since the last frame means the app was away without saying so (a browser
        // tab just stops drawing): anything held then was let go where we couldn't see
        let mut away = get_frame_time() >= AWAY_FRAME_TIME;
        if away {
            self.release_all();
        }
        let dpi = screen_dpi_scale();
        for event in self.recorder.events.drain(..) {
            match event {
                Raw::Touch { id, phase, x, y } => {
                    let at = view.to_playfield(vec2(x, y) / dpi);
                    self.touch.event(id, phase, at, now);
                    self.menu.event(id, phase, at, now);
                }
                Raw::Key { code, down, repeat } => {
                    self.keys.event(code, down, repeat);
                    self.menu_keys.event(code, down, repeat);
                }
                Raw::Minimized => {
                    self.touch.release_all();
                    self.keys.release_all();
                    self.menu.release_all();
                    self.menu_keys.release_all();
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
        let hover = self.recorder.hover.filter(|_| !self.recorder.mouse_down).map(|at| view.to_playfield(at / dpi));
        self.menu_input = MenuInput { hover, wheel: std::mem::take(&mut self.recorder.wheel), nav: self.menu_keys.take(), ..self.menu.take() };
        (controls, away)
    }

    /// This frame's taps, drags, keys and pointing in menus (worked out by `poll`).
    pub fn menu(&self) -> MenuInput {
        self.menu_input
    }

    /// The app went away: every finger and key is let go, without leaping.
    fn release_all(&mut self) {
        self.touch.release_all();
        self.keys.release_all();
        self.menu.release_all();
        self.menu_keys.release_all();
    }

    /// The screen changed: fingers and the jump key already down do nothing until let go.
    pub fn ignore_current(&mut self) {
        self.touch.ignore_current();
        self.keys.ignore_current();
        self.menu.ignore_current();
        self.menu_keys.ignore_current();
        self.menu_input = MenuInput::default();
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

    /// Draw the slider bar (always, on a touch screen) with its knob where the thumb has it,
    /// brief rings and chevrons acknowledging the action finger, and the pause button (in the
    /// screen camera).
    pub fn draw(&self) {
        let slider = self.touch.slider();
        if self.is_touch() || slider.is_some() {
            draw_slider(slider.map_or(0.0, |s| s.offset()), slider.is_some());
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

/// The slider bar: a track with rounded ends and arrows past them, a notch in the middle where
/// Kitty stands still, and the knob `offset` from it (brighter while held). Everything is see-through
/// with a dark rim, so it reads over sky and leaves alike.
fn draw_slider(offset: f32, held: bool) {
    let (centre, reach) = (touch::SLIDER_CENTRE, touch::SLIDER_REACH);
    let edge = Color::new(0.1, 0.1, 0.1, 0.35);
    let white = Color::new(1.0, 1.0, 1.0, 0.25);
    let r = TRACK_THICKNESS / 2.0;
    capsule(centre, reach, r + 1.5, edge);
    capsule(centre, reach, r, white);
    draw_rectangle(centre.x - 1.5, centre.y - r - 4.0, 3.0, TRACK_THICKNESS + 8.0, Color::new(1.0, 1.0, 1.0, 0.35));
    for side in [-1.0, 1.0] {
        let tip = centre.x + side * (reach + r + ARROW_GAP + ARROW_SIZE);
        let back = tip - side * ARROW_SIZE;
        let points = [vec2(tip, centre.y), vec2(back, centre.y - ARROW_SIZE), vec2(back, centre.y + ARROW_SIZE)];
        let middle = (points[0] + points[1] + points[2]) / 3.0;
        // A dark rim (the arrow grown about its middle), then the arrow
        for (grow, color) in [(1.3, edge), (1.0, Color::new(1.0, 1.0, 1.0, 0.45))] {
            let [a, b, c] = points.map(|p| middle + (p - middle) * grow);
            draw_triangle(a, b, c, color);
        }
    }
    let knob = centre.x + offset;
    let fill = if held { 0.45 } else { 0.28 };
    draw_circle(knob, centre.y, KNOB_RADIUS, Color::new(1.0, 1.0, 1.0, fill));
    draw_circle_lines(knob, centre.y, KNOB_RADIUS, 2.0, Color::new(0.15, 0.15, 0.15, 0.35));
}

/// A see-through bar `reach` either way of `centre`, `r` thick each side, with round ends. Its
/// pieces don't overlap, so it's evenly see-through, with no brighter seams.
fn capsule(centre: Vec2, reach: f32, r: f32, color: Color) {
    draw_rectangle(centre.x - reach, centre.y - r, reach * 2.0, r * 2.0, color);
    const SLICES: usize = 10;
    for side in [-1.0f32, 1.0] {
        let end = vec2(centre.x + side * reach, centre.y);
        // The outer half of a disc, as a fan of slices
        let point = |i: usize| {
            let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / SLICES as f32;
            end + vec2(side * angle.cos(), angle.sin()) * r
        };
        for i in 0..SLICES {
            draw_triangle(end, point(i), point(i + 1), color);
        }
    }
}
