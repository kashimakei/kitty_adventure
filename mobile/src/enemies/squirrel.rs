//! A squirrel: scampers to and fro along its limb, riding it as it bends, and lobs acorns at Kitty
//! when she comes within range. The wind-up, with the acorn held overhead, is the tell. Stomp it and
//! it topples off the tree.

use macroquad::prelude::*;

use super::{Enemy, EnemyCtx, Kind, Projectile, Sfx, ACORN_GRAVITY};
use crate::consts::{GROUND_Y, STEP};
use crate::kitty::{Push, Support};
use crate::level::Difficulty;
use crate::limb::{Limb, LimbId};
use crate::sprites::{Animator, Assets, Clip, Sprites};

/// Its weight on the limb: enough to make thin wood dip, nothing like Kitty's.
const WEIGHT: f32 = 5000.0;
const RUN_SPEED: f32 = 95.0; // px/s
const IDLE_TIME: (f32, f32) = (0.8, 2.2);
/// Shorter hops than this aren't worth getting up for.
const MIN_SCAMPER: f32 = 25.0;
/// On Kitty's limb it keeps at least this far from her, on its own side.
const KEEP_AWAY: f32 = 90.0;
/// It throws when Kitty is this close (px across, px up or down) and both are on screen.
const THROW_RANGE: Vec2 = Vec2::new(330.0, 300.0);
/// It only throws down or across: once Kitty's feet are more than this far above its own, she's
/// climbed past it and it leaves her be.
const ABOVE: f32 = 24.0;
/// Seconds between throws on the first level; the level's pace shortens them.
const THROW_COOLDOWN: (f32, f32) = (2.6, 3.8);
/// A grace period when the level starts, so squirrels don't all throw at once.
const FIRST_THROW: (f32, f32) = (1.0, 3.0);
/// The acorn leaves the paw on this frame of the throw (throw_3, the arm coming forward), after
/// the wind-up holds it overhead for a beat.
const RELEASE_FRAME: usize = 4;
/// It stays in the follow-through pose a moment before carrying on.
const THROW_HOLD: f32 = 0.15;
/// Where the paw lets go of the acorn, from the feet, facing right.
const PAW: Vec2 = Vec2::new(22.0, -36.0);
/// Lob flight time: longer throws take longer, so the arc stays easy to read and dodge.
const FLIGHT_TIME: (f32, f32) = (0.5, 1.0);
const FLIGHT_TIME_PER_PX: f32 = 1.0 / 600.0;
/// It aims at Kitty's middle give or take this much, so it isn't a perfect shot.
const AIM_SPREAD: f32 = 18.0;
const HURT_TIME: f32 = 0.5;
/// Its back half from the tail tip to the nose, top down to mid-body: generous for a thumb on a phone.
const STOMP_AREA: Rect = Rect { x: -40.0, y: -66.0, w: 68.0, h: 36.0 };

static IDLE: Clip = Clip { frames: &["idle_0", "idle_1", "idle_2", "idle_3"], fps: 5.0, looped: true };
static RUN: Clip = Clip { frames: &["run_0", "run_1", "run_2", "run_3", "run_4"], fps: 12.0, looped: true };
// Reach back, acorn behind, acorn overhead (held: the tell), let fly, follow through
static THROW: Clip = Clip {
    frames: &["throw_0", "throw_1", "throw_2", "throw_2", "throw_3", "throw_4"],
    fps: 10.0,
    looped: false,
};
// Ouch, then dizzy
static HURT: Clip = Clip { frames: &["hurt_0", "hurt_1", "hurt_1"], fps: 6.0, looped: false };
static TUMBLE: Clip = Clip { frames: &["hurt_0"], fps: 1.0, looped: true };

#[derive(Clone, Copy)]
enum State {
    /// Sitting up, looking about.
    Idle { timer: f32 },
    /// Running along the limb to x = `target`.
    Scamper { target: f32 },
    /// Winding up and throwing; `thrown` once the acorn has left the paw.
    Throw { timer: f32, thrown: bool },
    /// Just stomped: dazed a moment before it topples off.
    Hurt { timer: f32 },
    Falling(Tumble),
}

pub struct Squirrel {
    limb: LimbId,
    /// Where along the limb's walk range it starts; it's placed on the first step.
    frac: f32,
    placed: bool,
    pos: Vec2, // feet
    facing: f32,
    state: State,
    anim: Animator,
    cooldown: f32,
    alive: bool,
}

impl Squirrel {
    pub fn new(limb: LimbId, frac: f32) -> Self {
        Self {
            limb,
            frac,
            placed: false,
            pos: vec2(-9999.0, -9999.0),
            facing: 1.0,
            state: State::Idle { timer: 0.0 },
            anim: Animator::new(&IDLE),
            cooldown: 0.0,
            alive: true,
        }
    }

    fn on_kitty_limb(&self, ctx: &EnemyCtx) -> bool {
        ctx.kitty.standing_on == Some(Support::Limb(self.limb))
    }

    fn kitty_above(&self, ctx: &EnemyCtx) -> bool {
        ctx.kitty.y < self.pos.y - ABOVE
    }

    fn wants_to_throw(&self, ctx: &EnemyCtx) -> bool {
        let kitty = ctx.kitty.body.center();
        let d = kitty - self.pos;
        self.cooldown <= 0.0
            && !self.kitty_above(ctx)
            && d.x.abs() < THROW_RANGE.x
            && d.y.abs() < THROW_RANGE.y
            && ctx.view.contains(kitty)
            && ctx.view.contains(self.pos - vec2(0.0, 30.0))
    }

    fn idle(&mut self, ctx: &mut EnemyCtx) {
        self.state = State::Idle { timer: ctx.rng.pair(IDLE_TIME) };
        self.anim.play(&IDLE);
    }

    fn start_throw(&mut self, ctx: &EnemyCtx) {
        self.facing = if ctx.kitty.x >= self.pos.x { 1.0 } else { -1.0 };
        self.state = State::Throw { timer: 0.0, thrown: false };
        self.anim.restart(&THROW);
    }

    /// Somewhere else along the limb, staying on its own side of Kitty if she's on it too.
    fn pick_target(&self, lo: f32, hi: f32, ctx: &mut EnemyCtx) -> f32 {
        let mut target = ctx.rng.range(lo, hi);
        if self.on_kitty_limb(ctx) {
            let side = if self.pos.x >= ctx.kitty.x { 1.0 } else { -1.0 };
            let limit = ctx.kitty.x + side * KEEP_AWAY;
            target = if side > 0.0 { target.max(limit) } else { target.min(limit) };
        }
        target.clamp(lo, hi)
    }

    /// Lob an acorn on an arc that comes down on Kitty (where she is now: moving dodges it).
    fn release(&mut self, ctx: &mut EnemyCtx) {
        let target = ctx.kitty.body.center() + vec2(ctx.rng.range(-AIM_SPREAD, AIM_SPREAD), 0.0);
        if target.x != self.pos.x {
            self.facing = (target.x - self.pos.x).signum();
        }
        let paw = self.pos + vec2(PAW.x * self.facing, PAW.y);
        let d = target - paw;
        let t = (FLIGHT_TIME.0 + d.x.abs() * FLIGHT_TIME_PER_PX).min(FLIGHT_TIME.1);
        let vel = vec2(d.x / t, d.y / t - 0.5 * ACORN_GRAVITY * t);
        ctx.projectiles.push(Projectile::acorn(paw, vel));
        ctx.sfx.push(Sfx::Throw);
        self.cooldown = ctx.rng.pair(THROW_COOLDOWN) / pace(ctx.difficulty);
    }

    fn fall(&mut self, vel: Vec2) {
        let spin = if vel.x < 0.0 { -TUMBLE_SPIN } else { TUMBLE_SPIN };
        self.state = State::Falling(Tumble::new(self.pos, vel, spin));
        self.anim.play(&TUMBLE);
    }

    /// Stand on the limb where it is now. False if there's no wood underfoot any more.
    fn ride(&mut self, limb: &Limb) -> bool {
        let (lo, hi) = limb.walk_range();
        self.pos.x = self.pos.x.clamp(lo, hi);
        match limb.surface_y(self.pos.x) {
            Some(y) => {
                self.pos.y = y;
                true
            }
            None => false,
        }
    }
}

impl Enemy for Squirrel {
    fn step(&mut self, ctx: &mut EnemyCtx) {
        if let State::Falling(tumble) = &mut self.state {
            tumble.step();
            self.pos = tumble.pos;
            self.alive = !tumble.gone(ctx.view);
            return;
        }
        let limbs = ctx.limbs;
        let limb = &limbs[self.limb];
        let (lo, hi) = limb.walk_range();
        if !self.placed {
            self.placed = true;
            self.pos.x = lo + (hi - lo) * self.frac;
            self.cooldown = ctx.rng.pair(FIRST_THROW);
            self.idle(ctx);
        }
        if limb.broken() {
            self.fall(Vec2::ZERO);
            return;
        }
        self.cooldown -= STEP;

        match self.state {
            State::Idle { timer } => {
                if self.wants_to_throw(ctx) {
                    self.start_throw(ctx);
                } else if timer > STEP {
                    self.state = State::Idle { timer: timer - STEP };
                    // Keep an eye on Kitty while she's about
                    if (ctx.kitty.x - self.pos.x).abs() < THROW_RANGE.x * 1.3 {
                        self.facing = if ctx.kitty.x >= self.pos.x { 1.0 } else { -1.0 };
                    }
                } else {
                    let target = self.pick_target(lo, hi, ctx);
                    if (target - self.pos.x).abs() < MIN_SCAMPER {
                        self.idle(ctx);
                    } else {
                        self.state = State::Scamper { target };
                        self.anim.play(&RUN);
                    }
                }
            }
            State::Scamper { target } => {
                let dir = if target >= self.pos.x { 1.0 } else { -1.0 };
                let toward_kitty = (ctx.kitty.x - self.pos.x) * dir;
                let blocked = self.on_kitty_limb(ctx) && toward_kitty > 0.0 && toward_kitty < KEEP_AWAY;
                if self.wants_to_throw(ctx) {
                    self.start_throw(ctx);
                } else if blocked {
                    self.idle(ctx);
                } else if (target - self.pos.x).abs() <= RUN_SPEED * STEP {
                    self.pos.x = target;
                    self.idle(ctx);
                } else {
                    self.pos.x += dir * RUN_SPEED * STEP;
                    self.facing = dir;
                }
            }
            // Kitty climbed above it during the wind-up: it gives up rather than lob upward
            State::Throw { thrown: false, .. } if self.kitty_above(ctx) => self.idle(ctx),
            State::Throw { timer, mut thrown } => {
                let timer = timer + STEP;
                if !thrown && timer >= RELEASE_FRAME as f32 / THROW.fps {
                    self.release(ctx);
                    thrown = true;
                }
                if timer >= THROW.duration() + THROW_HOLD {
                    self.idle(ctx);
                } else {
                    self.state = State::Throw { timer, thrown };
                }
            }
            State::Hurt { timer } => {
                let timer = timer + STEP;
                if timer >= HURT_TIME {
                    // Topples off away from Kitty
                    let away = if self.pos.x >= ctx.kitty.x { 1.0 } else { -1.0 };
                    self.fall(vec2(away * 70.0, -260.0));
                    return;
                }
                self.state = State::Hurt { timer };
            }
            State::Falling(_) => unreachable!("handled above"),
        }

        if self.ride(limb) {
            ctx.pushes.push(Push::Load { limb: self.limb, x: self.pos.x, weight: WEIGHT });
        } else {
            self.fall(Vec2::ZERO);
        }
    }

    fn animate(&mut self, dt: f32) {
        self.anim.update(dt);
    }

    fn draw(&self, assets: &Assets) {
        let frame = self.anim.frame();
        match &self.state {
            State::Falling(tumble) => tumble.draw(&assets.squirrel, frame, self.facing),
            State::Hurt { timer } => assets.squirrel.draw(frame, self.pos, self.facing, hurt_tint(*timer)),
            _ => assets.squirrel.draw(frame, self.pos, self.facing, WHITE),
        }
    }

    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        match self.state {
            State::Idle { .. } | State::Scamper { .. } | State::Throw { .. } => {
                assets.squirrel.strikes(self.anim.frame(), self.pos, self.facing, body)
            }
            State::Hurt { .. } | State::Falling(_) => false,
        }
    }

    fn stomp_box(&self, _assets: &Assets) -> Option<Rect> {
        match self.state {
            State::Idle { .. } | State::Scamper { .. } | State::Throw { .. } if self.placed => {
                Some(faced(STOMP_AREA, self.pos, self.facing))
            }
            _ => None,
        }
    }

    fn stomped(&mut self) {
        if matches!(self.state, State::Idle { .. } | State::Scamper { .. } | State::Throw { .. }) {
            self.state = State::Hurt { timer: 0.0 };
            self.anim.restart(&HURT);
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn position(&self) -> Vec2 {
        self.pos - vec2(0.0, 28.0)
    }

    fn kind(&self) -> Kind {
        Kind::Squirrel
    }
}

// ---- Shared with the snake and the crow, which get knocked out of the tree the same way ----

const TUMBLE_GRAVITY: f32 = 1500.0; // px/s^2
pub(super) const TUMBLE_SPIN: f32 = 7.0; // rad/s
/// A faller sinks this far into the grass as it fades out of sight.
const TUMBLE_FADE: f32 = 50.0;
/// Red flash when stomped.
const HURT_TINT: Color = Color::new(1.0, 0.5, 0.45, 1.0);
const HURT_FLASH: f32 = 0.15;

/// The level's general pace: 1 on the first level, rising to about 1.9 on the hardest laps.
/// Difficulty has no knobs for the small enemies, so they follow the dogs' speed ramp.
pub(super) fn pace(d: &Difficulty) -> f32 {
    d.dog_speed.max(1.0)
}

pub(super) fn hurt_tint(time_hurt: f32) -> Color {
    if time_hurt < HURT_FLASH { HURT_TINT } else { WHITE }
}

/// An area given relative to an anchor as if facing right, mirrored when facing left.
pub(super) fn faced(area: Rect, at: Vec2, facing: f32) -> Rect {
    let x = if facing < 0.0 { -(area.x + area.w) } else { area.x };
    Rect::new(at.x + x, at.y + area.y, area.w, area.h)
}

/// Knocked off the tree: flung up and away, then spinning down under gravity until it drops out
/// of the view (or into the grass).
#[derive(Clone, Copy)]
pub(super) struct Tumble {
    /// Where the frame's anchor is.
    pub pos: Vec2,
    vel: Vec2,
    spin: f32,
    rotation: f32,
}

impl Tumble {
    pub fn new(pos: Vec2, vel: Vec2, spin: f32) -> Self {
        Self { pos, vel, spin, rotation: 0.0 }
    }

    pub fn step(&mut self) {
        self.vel.y += TUMBLE_GRAVITY * STEP;
        self.pos += self.vel * STEP;
        self.rotation += self.spin * STEP;
    }

    pub fn gone(&self, view: Rect) -> bool {
        self.pos.y > view.bottom() + 120.0 || self.pos.y > GROUND_Y + TUMBLE_FADE
    }

    /// Spin about the middle of the frame rather than its anchor (usually the feet).
    pub fn draw(&self, sprites: &Sprites, name: &str, facing: f32) {
        let frame = sprites.get(name);
        let mid = frame.size() / 2.0 - frame.anchor_for(facing);
        let at = self.pos + mid - Vec2::from_angle(self.rotation).rotate(mid);
        let alpha = (1.0 - (self.pos.y - GROUND_Y) / TUMBLE_FADE).clamp(0.0, 1.0);
        sprites.draw_ex(name, at, facing, 1.0, self.rotation, Color::new(1.0, 1.0, 1.0, alpha));
    }
}
