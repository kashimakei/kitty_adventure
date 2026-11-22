//! A snake: lies coiled on its limb, riding it as it bends, and now and then slithers somewhere
//! else along it. When Kitty comes close it rears back with a hiss (the tell), then lunges at her
//! with its jaws open before settling again. Only the open jaws hurt. Stomp it from above, any time
//! but mid-lunge, and it falls off the tree.

use macroquad::prelude::*;

use super::squirrel::{faced, hurt_tint, pace, Tumble, TUMBLE_SPIN};
use super::{Enemy, EnemyCtx, Kind, Sfx};
use crate::consts::STEP;
use crate::kitty::{Push, Support};
use crate::limb::{Limb, LimbId};
use crate::sprites::{Animator, Assets, Clip};

/// Its weight on the limb, far lighter than Kitty's.
const WEIGHT: f32 = 4000.0;
const SLITHER_SPEED: f32 = 26.0; // px/s
const COIL_TIME: (f32, f32) = (2.0, 4.5);
const MIN_SLITHER: f32 = 30.0;
/// It raises its head to watch Kitty when she's this near.
const WATCH_RANGE: f32 = 260.0;
/// Kitty this near sets it off: px across, and px above or below its feet. It can't strike down
/// through its own limb, so it only bothers with Kitty a little below it.
const STRIKE_ACROSS: f32 = 150.0;
const STRIKE_ABOVE: f32 = 90.0;
const STRIKE_BELOW: f32 = 45.0;
/// Standing anywhere this near on its own limb sets it off too.
const SAME_LIMB_RANGE: f32 = 200.0;
/// The rear-back tell before the lunge.
const REAR_TIME: f32 = 0.42;
/// The lunge: out fast, a moment at full stretch, then back.
const LUNGE_OUT: f32 = 0.08;
const LUNGE_HOLD: f32 = 0.18;
const LUNGE_TIME: f32 = 0.3;
/// The whole snake slides along the limb toward Kitty by up to `LUNGE_MAX`, stopping its jaws a
/// bite into her body: they reach ~46 px ahead of its anchor and her body is 54 px wide, so it
/// lunges her distance less 46 + 27 - 12 px. Jaws reach Kitty up to ~137 px away.
const LUNGE_SHORT: f32 = 61.0;
const LUNGE_MAX: f32 = 64.0;
/// It springs up toward Kitty when she's above it, by up to `LIFT_MAX`; `JAW_TOP` is how far
/// above its feet the jaws reach in the strike frames.
const LIFT_MAX: f32 = 44.0;
const JAW_TOP: f32 = 51.0;
const LIFT_MARGIN: f32 = 14.0;
/// While rearing it draws back this far from Kitty and rattles, so the tell reads even at a glance
/// (its head-up watching pose looks much the same otherwise).
const REAR_PULL: f32 = 9.0;
const REAR_SHAKE: f32 = 2.0;
const RECOVER_TIME: f32 = 0.3;
/// Seconds before it can strike again on the first level; the level's pace shortens them.
const STRIKE_COOLDOWN: (f32, f32) = (1.4, 2.2);
const HURT_TIME: f32 = 0.5;
/// Over its coils and raised head, generous for a thumb on a phone.
const STOMP_AREA: Rect = Rect { x: -38.0, y: -54.0, w: 70.0, h: 42.0 };

// Resting with an occasional look round; head up with tongue flicks when Kitty's about
static COILED: Clip = Clip { frames: &["coiled_0", "coiled_0", "coiled_3", "coiled_0"], fps: 1.5, looped: true };
static ALERT: Clip = Clip { frames: &["coiled_1", "coiled_2"], fps: 3.0, looped: true };
static SLITHER: Clip = Clip {
    frames: &["slither_0", "slither_1", "slither_2", "slither_3", "slither_4"],
    fps: 7.0,
    looped: true,
};
static REAR: Clip = Clip { frames: &["strike_0", "strike_1"], fps: 6.0, looped: false };
static STRIKE: Clip = Clip { frames: &["strike_2", "strike_3"], fps: 2.0 / LUNGE_TIME, looped: false };
static RECOVER: Clip = Clip { frames: &["strike_4"], fps: 1.0, looped: false };
static HURT: Clip = Clip { frames: &["hurt_0", "hurt_1"], fps: 4.0, looped: false };
static TUMBLE: Clip = Clip { frames: &["hurt_1"], fps: 1.0, looped: true };

#[derive(Clone, Copy)]
enum State {
    Coiled { timer: f32 },
    /// Sliding slowly along the limb to x = `target`.
    Slither { target: f32 },
    /// Drawn back to strike: the tell.
    Rear { timer: f32 },
    /// Lunging by `reach` and back.
    Lunge { timer: f32, reach: Vec2 },
    Recover { timer: f32 },
    Hurt { timer: f32 },
    Falling(Tumble),
}

pub struct Snake {
    limb: LimbId,
    /// Where along the limb's walk range it starts; it's placed on the first step.
    frac: f32,
    placed: bool,
    pos: Vec2, // where its belly meets the limb
    /// How far it's lunged (or shaking) from `pos`, for drawing and biting.
    offset: Vec2,
    facing: f32,
    state: State,
    anim: Animator,
    cooldown: f32,
    alive: bool,
}

impl Snake {
    pub fn new(limb: LimbId, frac: f32) -> Self {
        Self {
            limb,
            frac,
            placed: false,
            pos: vec2(-9999.0, -9999.0),
            offset: Vec2::ZERO,
            facing: 1.0,
            state: State::Coiled { timer: 0.0 },
            anim: Animator::new(&COILED),
            cooldown: 0.0,
            alive: true,
        }
    }

    fn face_kitty(&mut self, ctx: &EnemyCtx) {
        self.facing = if ctx.kitty.x >= self.pos.x { 1.0 } else { -1.0 };
    }

    fn kitty_close(&self, ctx: &EnemyCtx) -> bool {
        let k = &ctx.kitty;
        let across = (k.x - self.pos.x).abs();
        let above = self.pos.y - k.y;
        let near = across < STRIKE_ACROSS && above < STRIKE_ABOVE && above > -STRIKE_BELOW;
        let same_limb = k.standing_on == Some(Support::Limb(self.limb)) && across < SAME_LIMB_RANGE;
        self.cooldown <= 0.0 && (near || same_limb) && ctx.view.contains(self.pos - vec2(0.0, 20.0))
    }

    fn coil(&mut self, ctx: &mut EnemyCtx) {
        self.state = State::Coiled { timer: ctx.rng.pair(COIL_TIME) };
        self.anim.play(&COILED);
    }

    fn rear(&mut self, ctx: &mut EnemyCtx) {
        self.face_kitty(ctx);
        self.state = State::Rear { timer: 0.0 };
        self.anim.restart(&REAR);
        ctx.sfx.push(Sfx::Hiss);
    }

    /// Aim the lunge at Kitty as she is now, without leaving the limb.
    fn lunge(&mut self, ctx: &EnemyCtx, lo: f32, hi: f32) {
        self.face_kitty(ctx);
        let k = &ctx.kitty;
        let room = if self.facing > 0.0 { hi - self.pos.x } else { self.pos.x - lo };
        let ahead = ((k.x - self.pos.x).abs() - LUNGE_SHORT).clamp(0.0, LUNGE_MAX).min(room.max(0.0));
        let lift = (self.pos.y - JAW_TOP - k.y + LIFT_MARGIN).clamp(0.0, LIFT_MAX);
        self.state = State::Lunge { timer: 0.0, reach: vec2(ahead * self.facing, -lift) };
        self.anim.restart(&STRIKE);
    }

    fn fall(&mut self, vel: Vec2) {
        let spin = if vel.x < 0.0 { -TUMBLE_SPIN } else { TUMBLE_SPIN };
        self.offset = Vec2::ZERO;
        self.state = State::Falling(Tumble::new(self.pos, vel, spin));
        self.anim.play(&TUMBLE);
    }

    /// Lie on the limb where it is now. False if there's no wood underneath any more.
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

    fn mid_lunge(&self) -> bool {
        matches!(self.state, State::Lunge { .. })
    }

    fn healthy(&self) -> bool {
        !matches!(self.state, State::Hurt { .. } | State::Falling(_))
    }
}

/// How far out the lunge is (0..1) `t` seconds in.
fn lunge_extent(t: f32) -> f32 {
    if t < LUNGE_OUT {
        t / LUNGE_OUT
    } else if t < LUNGE_HOLD {
        1.0
    } else {
        (1.0 - (t - LUNGE_HOLD) / (LUNGE_TIME - LUNGE_HOLD)).max(0.0)
    }
}

impl Enemy for Snake {
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
            self.coil(ctx);
        }
        if limb.fell_away(self.pos.x) {
            self.fall(Vec2::ZERO);
            return;
        }
        self.cooldown -= STEP;
        self.offset = Vec2::ZERO;

        match self.state {
            State::Coiled { timer } => {
                if self.kitty_close(ctx) {
                    self.rear(ctx);
                } else if timer > STEP {
                    self.state = State::Coiled { timer: timer - STEP };
                    if (ctx.kitty.x - self.pos.x).abs() < WATCH_RANGE && (ctx.kitty.y - self.pos.y).abs() < WATCH_RANGE {
                        self.face_kitty(ctx);
                        self.anim.play(&ALERT);
                    } else {
                        self.anim.play(&COILED);
                    }
                } else {
                    let target = ctx.rng.range(lo, hi);
                    if (target - self.pos.x).abs() < MIN_SLITHER {
                        self.coil(ctx);
                    } else {
                        self.state = State::Slither { target };
                        self.anim.play(&SLITHER);
                    }
                }
            }
            State::Slither { target } => {
                let dir = if target >= self.pos.x { 1.0 } else { -1.0 };
                if self.kitty_close(ctx) {
                    self.rear(ctx);
                } else if (target - self.pos.x).abs() <= SLITHER_SPEED * STEP {
                    self.pos.x = target;
                    self.coil(ctx);
                } else {
                    self.pos.x += dir * SLITHER_SPEED * STEP;
                    self.facing = dir;
                }
            }
            State::Rear { timer } => {
                let timer = timer + STEP;
                if timer >= REAR_TIME {
                    self.lunge(ctx, lo, hi);
                } else {
                    self.state = State::Rear { timer };
                    let u = timer / REAR_TIME;
                    self.offset.x = -self.facing * REAR_PULL * u * (2.0 - u) + (timer * 70.0).sin() * REAR_SHAKE;
                }
            }
            State::Lunge { timer, reach } => {
                let timer = timer + STEP;
                if timer >= LUNGE_TIME {
                    self.state = State::Recover { timer: 0.0 };
                    self.anim.restart(&RECOVER);
                    self.cooldown = ctx.rng.pair(STRIKE_COOLDOWN) / pace(ctx.difficulty);
                } else {
                    self.state = State::Lunge { timer, reach };
                    // Springs forward from where it drew back to
                    let extent = lunge_extent(timer);
                    let back = if timer < LUNGE_OUT { REAR_PULL * (1.0 - extent) } else { 0.0 };
                    self.offset = reach * extent - vec2(self.facing * back, 0.0);
                }
            }
            State::Recover { timer } => {
                let timer = timer + STEP;
                if timer >= RECOVER_TIME {
                    self.coil(ctx);
                } else {
                    self.state = State::Recover { timer };
                }
            }
            State::Hurt { timer } => {
                let timer = timer + STEP;
                if timer >= HURT_TIME {
                    // Slides off away from Kitty
                    let away = if self.pos.x >= ctx.kitty.x { 1.0 } else { -1.0 };
                    self.fall(vec2(away * 60.0, -220.0));
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
            State::Falling(tumble) => tumble.draw(&assets.snake, frame, self.facing),
            State::Hurt { timer } => assets.snake.draw(frame, self.pos, self.facing, hurt_tint(*timer)),
            _ => assets.snake.draw(frame, self.pos + self.offset, self.facing, WHITE),
        }
    }

    /// Only the open jaws of the lunge bite.
    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        self.mid_lunge() && assets.snake.strikes(self.anim.frame(), self.pos + self.offset, self.facing, body)
    }

    fn stomp_box(&self, _assets: &Assets) -> Option<Rect> {
        (self.placed && self.healthy() && !self.mid_lunge()).then(|| faced(STOMP_AREA, self.pos, self.facing))
    }

    fn in_front(&self) -> bool {
        self.mid_lunge()
    }

    fn stomped(&mut self) {
        if self.healthy() && !self.mid_lunge() {
            self.offset = Vec2::ZERO;
            self.state = State::Hurt { timer: 0.0 };
            self.anim.restart(&HURT);
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn position(&self) -> Vec2 {
        self.pos + self.offset - vec2(0.0, 22.0)
    }

    fn kind(&self) -> Kind {
        Kind::Snake
    }
}
