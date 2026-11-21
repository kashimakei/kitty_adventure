//! Eagle, ported from the pygame game's platform.py: cruises across drifting toward Kitty's
//! height, and once she's ahead and below it glides with wings set (the warning), swoops at her
//! with talons out, then pulls away and leaves. Only its beak and talons hurt.

use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use super::{Enemy, EnemyCtx, Kind, KittyView, Sfx};
use crate::consts::*;
use crate::rng::Rng;
use crate::sprites::{Animator, Assets, Clip};

const HITBOX: Vec2 = vec2(100.0, 72.0);
/// Where the head (every frame's anchor) sits relative to the hitbox centre, facing right.
const HEAD_OFFSET: Vec2 = vec2(42.0, -15.0);
/// Starts this far outside the view so no wingtip pops into sight.
const ENTRY_MARGIN: f32 = 120.0;
const CRUISE_SPEED: (f32, f32) = (150.0, 250.0); // px/s, scaled by the level's eagle speed
const MAX_DRIFT: f32 = 100.0; // px/s up or down while cruising toward Kitty's height
const SWOOP_RANGE: f32 = 280.0; // px ahead at which it commits to a swoop
const SWOOP_ABOVE: f32 = 40.0; // px Kitty may be above the eagle and still get swooped at
const WINDUP: f32 = 0.35; // seconds gliding, wings set, before diving: the player's cue
const SWOOP_SPEED: f32 = 340.0; // px/s, scaled by the level's eagle speed
const HOMING: f32 = 0.6; // rad/s it can bend its swoop; low enough to dodge
const SWOOP_TIME: f32 = 1.4; // seconds before a swoop gives up
const MIN_FORWARD: f32 = 60.0; // px/s; a swoop never doubles back
const OVERSHOOT: f32 = 30.0; // px past Kitty at which it pulls away
const LEAVE_SPEED: f32 = 200.0; // px/s at least, scaled, as it flies off
const PULL_UP: f32 = 160.0; // px/s climbing out of the swoop
const LEVEL_OFF: f32 = 150.0; // px/s^2 the climb eases by
const CLIMB_OUT: f32 = 40.0; // px/s it keeps rising as it leaves
const RECOVER_TIME: f32 = 0.6;
/// Lowest the hitbox centre goes: a swoop at Kitty on the ground skims the grass with its talons
/// instead of plunging into it.
const FLOOR: f32 = GROUND_Y - 50.0;
const TALON_REACH: f32 = 170.0; // px from Kitty at which the talons come forward
const PLUNGE_STEEPNESS: f32 = 1.3; // falling this many times faster than flying forward looks like a dive
/// Gone once this far past the view's side it's heading to, or below it; or, its swoop spent,
/// once this far out of sight any way at all.
const EXIT_MARGIN: f32 = 80.0;
const EXIT_BELOW: f32 = 150.0;

// Built from the frames sliced out of the eagle sheet (as in the pygame game)
static FLAP: Clip = Clip { frames: &["flap_0", "flap_1", "flap_2", "flap_3", "flap_4"], fps: 10.0, looped: true };
static GLIDE: Clip = Clip { frames: &["dive_0", "dive_1"], fps: 4.0, looped: true }; // wings set, lining up
static PLUNGE: Clip = Clip { frames: &["dive_2", "dive_3"], fps: 8.0, looped: true }; // steep dive
static TALONS: Clip = Clip { frames: &["talon_0", "talon_1", "talon_2", "talon_3", "talon_4"], fps: 12.0, looped: false };
static RECOVER: Clip = Clip { frames: &["dive_4", "flap_0", "flap_1"], fps: 8.0, looped: false };

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Cruise,
    Windup,
    Swoop,
    Recover,
}

pub struct Eagle {
    /// Hitbox centre.
    pos: Vec2,
    vel: Vec2,
    facing: f32,
    speed_scale: f32,
    state: State,
    state_time: f32,
    /// It swoops once per pass.
    swooped: bool,
    /// How far Kitty was at the last step, for picking the talon pose.
    kitty_distance: f32,
    alive: bool,
    animator: Animator,
}

impl Eagle {
    pub fn new(from_left: bool, y: f32, view: Rect, kitty: &KittyView, speed_scale: f32) -> Self {
        let facing = if from_left { 1.0 } else { -1.0 };
        let x = if from_left { view.left() - ENTRY_MARGIN } else { view.right() + ENTRY_MARGIN };
        // The spawner picked `y` at random, so it seeds this eagle's speed too
        let mut rng = Rng::new(y.to_bits() as u64);
        let vx = rng.pair(CRUISE_SPEED) * speed_scale * facing;
        // Drift toward Kitty's height over the time it takes to cross the screen
        let vy = ((kitty.rect.top() - y) * vx.abs() / VIEW_W).clamp(-MAX_DRIFT, MAX_DRIFT);
        Self {
            pos: vec2(x, y),
            vel: vec2(vx, vy),
            facing,
            speed_scale,
            state: State::Cruise,
            state_time: 0.0,
            swooped: false,
            kitty_distance: f32::INFINITY,
            alive: true,
            animator: Animator::new(&FLAP),
        }
    }

    fn set_state(&mut self, state: State) {
        self.state = state;
        self.state_time = 0.0;
    }

    /// Where the frames' anchor goes.
    fn head(&self) -> Vec2 {
        self.pos + vec2(HEAD_OFFSET.x * self.facing, HEAD_OFFSET.y)
    }

    fn hitbox(&self) -> Rect {
        Rect::new(self.pos.x - HITBOX.x / 2.0, self.pos.y - HITBOX.y / 2.0, HITBOX.x, HITBOX.y)
    }

    /// Flown out of the view for good?
    fn gone(&self, view: Rect) -> bool {
        let hitbox = self.hitbox();
        let m = EXIT_MARGIN;
        let past_side = if self.facing > 0.0 { hitbox.left() > view.right() + m } else { hitbox.right() < view.left() - m };
        let around = Rect::new(view.x - m, view.y - m, view.w + 2.0 * m, view.h + 2.0 * m);
        let spent = self.swooped && self.state == State::Cruise && !around.overlaps(&hitbox);
        past_side || hitbox.top() > view.bottom() + EXIT_BELOW || spent
    }
}

impl Enemy for Eagle {
    fn step(&mut self, ctx: &mut EnemyCtx) {
        self.state_time += STEP;
        let target = ctx.kitty.rect.center();
        let to_kitty = target - self.pos;
        let ahead = to_kitty.x * self.facing; // how far in front of the eagle Kitty is

        match self.state {
            State::Cruise => {
                // Only wind up where Kitty can see it, so the screech and glide are a fair warning
                let v = ctx.view;
                let on_screen = Rect::new(v.x + 40.0, v.y + 40.0, v.w - 80.0, v.h - 80.0).contains(self.pos);
                if !self.swooped && on_screen && 0.0 < ahead && ahead < SWOOP_RANGE
                    && target.y > self.pos.y - SWOOP_ABOVE {
                    self.set_state(State::Windup);
                    self.swooped = true;
                    self.vel.y = 0.0;
                    ctx.sfx.push(Sfx::Screech);
                }
            }
            State::Windup => {
                // Wings set, gliding level: the tell that a dive is coming
                if self.state_time >= WINDUP {
                    self.set_state(State::Swoop);
                    self.vel = to_kitty.normalize_or(vec2(self.facing, 0.0)) * SWOOP_SPEED * self.speed_scale;
                }
            }
            State::Swoop => {
                // Bend gently toward Kitty; quick feet can still slip out from under the talons
                let speed = self.vel.length();
                let heading = self.vel.to_angle();
                let turn = (to_kitty.to_angle() - heading + PI).rem_euclid(TAU) - PI;
                let heading = heading + turn.clamp(-HOMING * STEP, HOMING * STEP);
                self.vel = Vec2::from_angle(heading) * speed;
                if self.vel.x * self.facing < MIN_FORWARD {
                    self.vel.x = MIN_FORWARD * self.facing;
                }
                if ahead < -OVERSHOOT || self.state_time > SWOOP_TIME {
                    self.set_state(State::Recover);
                    self.vel.x = self.facing * self.vel.x.abs().max(LEAVE_SPEED * self.speed_scale);
                    self.vel.y = -PULL_UP;
                }
            }
            State::Recover => {
                // Pull up, then level off into a gentle climb
                self.vel.y = (self.vel.y + LEVEL_OFF * STEP).min(-CLIMB_OUT);
                if self.state_time > RECOVER_TIME {
                    self.set_state(State::Cruise);
                }
            }
        }

        self.pos += self.vel * STEP;
        if self.pos.y > FLOOR {
            self.pos.y = FLOOR;
            self.vel.y = self.vel.y.min(0.0);
        }
        self.kitty_distance = self.pos.distance(target);
        if self.gone(ctx.view) {
            self.alive = false;
        }
    }

    fn animate(&mut self, dt: f32) {
        let anim = &self.animator;
        let clip = match self.state {
            State::Windup => &GLIDE,
            State::Swoop if self.vel.y > PLUNGE_STEEPNESS * self.vel.x.abs() => &PLUNGE,
            // Once the talons swing forward they stay out for the rest of the swoop
            State::Swoop if self.kitty_distance < TALON_REACH || anim.is(&TALONS) => &TALONS,
            State::Swoop => &GLIDE,
            State::Recover => &RECOVER,
            State::Cruise if anim.is(&RECOVER) && !anim.finished() => &RECOVER,
            State::Cruise => &FLAP,
        };
        self.animator.play(clip);
        self.animator.update(dt);
    }

    fn draw(&self, assets: &Assets) {
        assets.eagle.draw(self.animator.frame(), self.head(), self.facing, WHITE);
    }

    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        assets.eagle.strikes(self.animator.frame(), self.head(), self.facing, body)
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn position(&self) -> Vec2 {
        self.pos
    }

    fn kind(&self) -> Kind {
        Kind::Eagle
    }
}
