//! A crow: flaps in from one side on a wavy path, cawing as it arrives. When it passes over Kitty it
//! may brake and flap hard for a moment (the tell), then dive at her beak first, pull up and carry
//! on. Some crows stop on a limb tip for a rest, riding it as it bends; from there one caws and dives
//! if Kitty passes below, and hops off if she walks up to it. Only the beak hurts. Stomp it, perched
//! or in flight, and it tumbles out of the sky.

use macroquad::prelude::*;

use super::squirrel::{faced, hurt_tint, pace, Tumble, TUMBLE_SPIN};
use super::{Enemy, EnemyCtx, Kind, Sfx};
use crate::consts::{GROUND_Y, STEP};
use crate::kitty::{Push, Support};
use crate::limb::{Limb, LimbId};
use crate::sprites::{Animator, Assets, Clip};

const CRUISE_SPEED: f32 = 135.0; // px/s
/// The wavy flight: px up and down, and how fast it bobs (rad/s).
const WAVE_AMP: f32 = 22.0;
const WAVE_FREQ: f32 = 3.4;
/// It caws once it's this far inside the view.
const ARRIVE_MARGIN: f32 = 30.0;
/// Gone once it's this far past the edge it's heading for, or this far outside the view any way.
const EXIT_MARGIN: f32 = 140.0;
const LOST_MARGIN: f32 = 600.0;

/// It dives when Kitty's middle is this far ahead of it (px) and this far below.
const DIVE_AHEAD: (f32, f32) = (40.0, 230.0);
const DIVE_BELOW: (f32, f32) = (110.0, 520.0);
/// Not every pass ends in a dive.
const DIVE_CHANCE: f32 = 0.75;
/// The tell: it brakes, bobs up and flaps hard for this long before it dives.
const TELL_TIME: f32 = 0.45;
const TELL_RISE: f32 = 14.0;
const DIVE_SPEED: f32 = 420.0; // px/s
/// Where the beak is from its body in the dive frames, facing right: it aims this at Kitty.
const DIVE_BEAK: Vec2 = Vec2::new(28.0, 28.0);
/// It switches to the peck frame this near the end of the dive, and gives up after `DIVE_MAX_TIME`.
const PECK_DIST: f32 = 60.0;
const DIVE_MAX_TIME: f32 = 1.6;
/// Pulling up out of the dive (px/s^2), turning back to cruising (px/s^2), and climbing (px/s).
const PULL_ACCEL: f32 = 1800.0;
const TURN_ACCEL: f32 = 500.0;
const CLIMB_SPEED: f32 = 170.0;
/// It never swoops lower than this above the grass.
const GROUND_CLEARANCE: f32 = 30.0;
/// Seconds between dives on the first level; the level's pace shortens them.
const DIVE_COOLDOWN: (f32, f32) = (2.5, 3.5);

/// Share of crows that stop for a rest on the way across.
const PERCH_CHANCE: f32 = 0.45;
/// Limb tips it considers landing on: this far ahead (px), within this much height of it.
const PERCH_AHEAD: (f32, f32) = (40.0, 380.0);
const PERCH_HEIGHT: f32 = 320.0;
/// It won't pick a perch this close to Kitty, or under the score bar or low down where thumbs
/// cover the screen.
const PERCH_CLEARANCE: f32 = 170.0;
const PERCH_VIEW_TOP: f32 = 160.0;
const PERCH_VIEW_BOTTOM: f32 = 260.0;
/// Gliding in to land takes this long (s).
const APPROACH_TIME: (f32, f32) = (0.9, 4.5);
/// How far in from the end of the walkable wood it lands.
const PERCH_INSET: f32 = 12.0;
const PERCH_TIME: (f32, f32) = (2.5, 4.5);
/// Its weight on the limb (crows are light) and the bob it gives the limb landing or taking off.
const WEIGHT: f32 = 2500.0;
const PERCH_KICK: f32 = 0.35;
/// Perched frames are anchored at the feet, flying ones at the body; this far apart.
const PERCH_DROP: f32 = 34.0;
/// Taking off, it climbs this far before cruising on.
const TAKEOFF_RISE: f32 = 70.0;
/// From the perch it dives at Kitty when her middle is this near across and this far below it.
const PERCH_DIVE_ACROSS: f32 = 260.0;
const PERCH_DIVE_BELOW: (f32, f32) = (50.0, 520.0);
/// It turns to watch Kitty when she's this near.
const PERCH_WATCH: f32 = 600.0;
/// The caw before it dives from the perch.
const PERCH_TELL_TIME: f32 = 0.45;
/// Kitty walking up its limb this close makes it hop off.
const FLEE_RANGE: f32 = 110.0;

const HURT_TIME: f32 = 0.35;
/// Over its back and head, from its body: generous for a thumb on a phone.
const STOMP_AREA: Rect = Rect { x: -34.0, y: -30.0, w: 66.0, h: 34.0 };
/// Perched, its head (and the open beak of the caw) stands higher.
const PERCH_STOMP_AREA: Rect = Rect { x: -28.0, y: -36.0, w: 60.0, h: 40.0 };

static FLY: Clip = Clip { frames: &["fly_0", "fly_1", "fly_2", "fly_3", "fly_4", "fly_5"], fps: 12.0, looped: true };
// The tell: hanging in the air, flapping hard
static FLAP: Clip = Clip { frames: &["fly_0", "fly_1", "fly_2", "fly_3", "fly_4", "fly_5"], fps: 22.0, looped: true };
// Tuck, then plunge beak first
static DIVE: Clip = Clip { frames: &["dive_0", "dive_1"], fps: 8.0, looped: false };
static PECK: Clip = Clip { frames: &["dive_2"], fps: 1.0, looped: false };
// Wings spread and feet down: pulling up out of a dive, or flaring to land
static FLARE: Clip = Clip { frames: &["dive_3"], fps: 1.0, looped: false };
// Looking about and shuffling its feet
static PERCHED: Clip = Clip {
    frames: &["perched_0", "perched_0", "perched_1", "perched_0", "perched_2", "perched_0"],
    fps: 2.5,
    looped: true,
};
// The perch tell: caw at Kitty, then spread the wings to launch
static CAW: Clip = Clip { frames: &["perched_3", "perched_3", "dive_3"], fps: 3.0 / PERCH_TELL_TIME, looped: false };
static HURT: Clip = Clip { frames: &["hurt_1"], fps: 1.0, looped: false };
static TUMBLE: Clip = Clip { frames: &["hurt_0"], fps: 1.0, looped: true };

#[derive(Clone, Copy)]
enum State {
    /// Cruising across on a wave about `base_y`.
    Fly { base_y: f32, phase: f32 },
    /// Braking and flapping hard before a dive.
    Tell { timer: f32, from: Vec2 },
    /// Plunging at where Kitty was: `left` px to go; it climbs back to `from_y` afterwards.
    Dive { vel: Vec2, left: f32, timer: f32, from_y: f32 },
    /// Pulling up, or taking off, then levelling out at `target_y`.
    Climb { vel: Vec2, target_y: f32 },
    /// Gliding in to land on `perch`, from where it was `timer` seconds ago.
    Approach { perch: Perch, from: Vec2, timer: f32, duration: f32 },
    Perched { perch: Perch, timer: f32 },
    /// Cawing at Kitty below before diving off the perch.
    PerchTell { perch: Perch, timer: f32 },
    /// Just stomped: stunned a moment (still on its perch, if it had one) before it drops.
    Hurt { timer: f32, perch: Option<Perch> },
    Falling(Tumble),
}

/// Where on a limb it stands.
#[derive(Clone, Copy)]
struct Perch {
    limb: LimbId,
    x: f32,
}

pub struct Crow {
    /// The middle of its body (perched frames are drawn `PERCH_DROP` lower, at its feet).
    pos: Vec2,
    /// The way it's travelling across, and the way it's looking.
    dir: f32,
    facing: f32,
    speed: f32,
    state: State,
    anim: Animator,
    arrived: bool,
    /// Whether it's the kind that stops on a limb, decided when it arrives, and whether it has.
    percher: bool,
    rested: bool,
    /// Passing over Kitty, it decides once whether to dive.
    considered: bool,
    cooldown: f32,
    alive: bool,
}

impl Crow {
    pub fn new(from_left: bool, y: f32, view: Rect, speed_scale: f32) -> Self {
        let dir = if from_left { 1.0 } else { -1.0 };
        let x = if from_left { view.left() - 60.0 } else { view.right() + 60.0 };
        Self {
            pos: vec2(x, y),
            dir,
            facing: dir,
            speed: CRUISE_SPEED * speed_scale,
            state: State::Fly { base_y: y, phase: 0.0 },
            anim: Animator::new(&FLY),
            arrived: false,
            percher: false,
            rested: false,
            considered: false,
            cooldown: 0.0,
            alive: true,
        }
    }

    fn healthy(&self) -> bool {
        !matches!(self.state, State::Hurt { .. } | State::Falling(_))
    }

    /// The frame showing now and where its anchor goes.
    fn frame(&self) -> (&'static str, Vec2) {
        let frame = self.anim.frame();
        let perched = frame.starts_with("perched");
        (frame, if perched { self.pos + vec2(0.0, PERCH_DROP) } else { self.pos })
    }

    /// Kitty is ahead and below, in reach of a dive, and both are on screen.
    fn over_kitty(&self, ctx: &EnemyCtx) -> bool {
        let d = ctx.kitty.body.center() - self.pos;
        let ahead = d.x * self.dir;
        ahead > DIVE_AHEAD.0 && ahead < DIVE_AHEAD.1 && d.y > DIVE_BELOW.0 && d.y < DIVE_BELOW.1
            && ctx.view.contains(self.pos) && ctx.view.contains(ctx.kitty.body.center())
    }

    /// The nearest limb tip ahead that's on screen and not too near Kitty (or her friend's nest).
    fn find_perch(&self, ctx: &EnemyCtx) -> Option<Perch> {
        let view = Rect::new(ctx.view.x + 40.0, ctx.view.y + PERCH_VIEW_TOP, ctx.view.w - 80.0,
                             ctx.view.h - PERCH_VIEW_TOP - PERCH_VIEW_BOTTOM);
        let kitty = ctx.kitty.body.center();
        ctx.limbs.iter().enumerate()
            .filter(|(id, limb)| {
                !limb.is_goal && !limb.gone() && ctx.kitty.standing_on != Some(Support::Limb(*id))
            })
            .filter_map(|(id, limb)| {
                let (lo, hi) = limb.walk_range();
                let x = if limb.direction > 0.0 { hi - PERCH_INSET } else { lo + PERCH_INSET };
                let body = vec2(x, limb.surface_y(x)? - PERCH_DROP);
                let ahead = (x - self.pos.x) * self.dir;
                let ok = ahead > PERCH_AHEAD.0 && ahead < PERCH_AHEAD.1
                    && (body.y - self.pos.y).abs() < PERCH_HEIGHT
                    && view.contains(body)
                    && body.distance(kitty) > PERCH_CLEARANCE;
                ok.then_some((Perch { limb: id, x }, ahead))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(perch, _)| perch)
    }

    /// Stand on the perch where the limb is now. False if the limb broke or bent away.
    fn ride(&mut self, limbs: &[Limb], perch: Perch) -> bool {
        let limb = &limbs[perch.limb];
        if limb.fell_away(perch.x) {
            return false;
        }
        let (lo, hi) = limb.walk_range();
        let x = perch.x.clamp(lo, hi);
        match limb.surface_y(x) {
            Some(y) => {
                self.pos = vec2(x, y - PERCH_DROP);
                true
            }
            None => false,
        }
    }

    fn face(&mut self, x: f32) {
        if (x - self.pos.x).abs() > 4.0 {
            self.facing = (x - self.pos.x).signum();
        }
    }

    fn kick(&self, ctx: &mut EnemyCtx, perch: Perch) {
        if !ctx.limbs[perch.limb].fell_away(perch.x) {
            ctx.pushes.push(Push::Kick { limb: perch.limb, x: perch.x, speed: PERCH_KICK });
        }
    }

    /// Fly up off the perch and carry on the way `dir` points.
    fn take_off(&mut self, ctx: &mut EnemyCtx, perch: Perch, dir: f32) {
        self.kick(ctx, perch);
        self.dir = dir;
        self.facing = dir;
        self.state = State::Climb { vel: vec2(dir * self.speed * 0.4, -CLIMB_SPEED), target_y: self.pos.y - TAKEOFF_RISE };
        self.anim.restart(&FLY);
    }

    /// Plunge at Kitty as she is now (moving dodges it), or give up if she isn't below any more.
    fn dive(&mut self, ctx: &EnemyCtx, from_y: f32) {
        let target = ctx.kitty.body.center();
        self.face(target.x);
        let aim = target - vec2(DIVE_BEAK.x * self.facing, DIVE_BEAK.y);
        let d = aim - self.pos;
        if d.y < DIVE_BEAK.y {
            self.state = State::Climb { vel: vec2(self.dir * self.speed * 0.3, 0.0), target_y: from_y };
            self.anim.play(&FLY);
            return;
        }
        self.dir = self.facing;
        self.state = State::Dive { vel: d.normalize() * DIVE_SPEED * self.speed / CRUISE_SPEED, left: d.length(), timer: 0.0, from_y };
        self.anim.restart(&DIVE);
    }

    fn fall(&mut self) {
        let spin = if self.facing < 0.0 { -TUMBLE_SPIN } else { TUMBLE_SPIN };
        self.state = State::Falling(Tumble::new(self.pos, vec2(-self.facing * 40.0, -160.0), spin));
        self.anim.play(&TUMBLE);
    }

    /// Flown off the edge it was heading for, or left far behind by the camera.
    fn gone(&self, view: Rect) -> bool {
        let past = if self.dir > 0.0 { self.pos.x > view.right() + EXIT_MARGIN } else { self.pos.x < view.left() - EXIT_MARGIN };
        let far = Rect::new(view.x - LOST_MARGIN, view.y - LOST_MARGIN, view.w + 2.0 * LOST_MARGIN, view.h + 2.0 * LOST_MARGIN);
        past || !far.contains(self.pos)
    }
}

/// Move `value` toward `target` by at most `step`.
fn approach(value: f32, target: f32, step: f32) -> f32 {
    value + (target - value).clamp(-step, step)
}

impl Enemy for Crow {
    fn step(&mut self, ctx: &mut EnemyCtx) {
        if let State::Falling(tumble) = &mut self.state {
            tumble.step();
            self.pos = tumble.pos;
            self.alive = !tumble.gone(ctx.view);
            return;
        }
        let limbs = ctx.limbs;
        self.cooldown -= STEP;
        if !self.arrived {
            let inside = Rect::new(ctx.view.x + ARRIVE_MARGIN, ctx.view.y, ctx.view.w - 2.0 * ARRIVE_MARGIN, ctx.view.h);
            if inside.contains(self.pos) {
                self.arrived = true;
                self.percher = ctx.rng.chance(PERCH_CHANCE);
                ctx.sfx.push(Sfx::Caw);
            }
        }

        match self.state {
            State::Fly { base_y, phase } => {
                let phase = phase + WAVE_FREQ * STEP;
                self.pos.x += self.dir * self.speed * STEP;
                self.pos.y = base_y + WAVE_AMP * phase.sin();
                self.facing = self.dir;
                self.state = State::Fly { base_y, phase };
                self.anim.play(&FLY);
                let over = self.over_kitty(ctx);
                if over && !self.considered {
                    self.considered = true;
                    if self.cooldown <= 0.0 && ctx.rng.chance(DIVE_CHANCE) {
                        self.state = State::Tell { timer: 0.0, from: self.pos };
                        self.anim.restart(&FLAP);
                    }
                } else if !over {
                    self.considered = false;
                    if self.arrived && self.percher && !self.rested {
                        if let Some(perch) = self.find_perch(ctx) {
                            self.rested = true;
                            // It glides in slowing all the way, so starting at cruising speed
                            // means taking twice as long as cruising there would
                            let duration = (2.0 * (perch.x - self.pos.x).abs() / self.speed).clamp(APPROACH_TIME.0, APPROACH_TIME.1);
                            self.state = State::Approach { perch, from: self.pos, timer: 0.0, duration };
                        }
                    }
                }
            }
            State::Tell { timer, from } => {
                let timer = timer + STEP;
                let u = (timer / TELL_TIME).min(1.0);
                self.pos.x += self.dir * self.speed * (1.0 - u) * STEP;
                self.pos.y = from.y - TELL_RISE * u * u * (3.0 - 2.0 * u);
                if timer >= TELL_TIME {
                    self.dive(ctx, from.y);
                } else {
                    self.state = State::Tell { timer, from };
                }
            }
            State::Dive { vel, left, timer, from_y } => {
                let (left, timer) = (left - vel.length() * STEP, timer + STEP);
                self.pos += vel * STEP;
                if left < PECK_DIST {
                    self.anim.play(&PECK);
                }
                if left <= 0.0 || timer >= DIVE_MAX_TIME || self.pos.y > GROUND_Y - GROUND_CLEARANCE {
                    self.state = State::Climb { vel, target_y: from_y };
                    self.anim.play(&FLARE);
                    self.cooldown = ctx.rng.pair(DIVE_COOLDOWN) / pace(ctx.difficulty);
                } else {
                    self.state = State::Dive { vel, left, timer, from_y };
                }
            }
            State::Climb { mut vel, target_y } => {
                let want = ((target_y - self.pos.y) * 3.0).clamp(-CLIMB_SPEED, CLIMB_SPEED);
                vel.y = approach(vel.y, want, PULL_ACCEL * STEP);
                vel.x = approach(vel.x, self.dir * self.speed, TURN_ACCEL * STEP);
                self.pos += vel * STEP;
                self.pos.y = self.pos.y.min(GROUND_Y - GROUND_CLEARANCE);
                if vel.y < 0.0 {
                    self.anim.play(&FLY);
                }
                if (self.pos.y - target_y).abs() < 3.0 && vel.y.abs() < 25.0 {
                    self.state = State::Fly { base_y: self.pos.y, phase: 0.0 };
                } else {
                    self.state = State::Climb { vel, target_y };
                }
            }
            State::Approach { perch, from, timer, duration } => {
                let timer = timer + STEP;
                let limb = &limbs[perch.limb];
                let Some(y) = limb.surface_y(perch.x).filter(|_| !limb.fell_away(perch.x)) else {
                    // The limb went; fly on
                    self.state = State::Climb { vel: vec2(self.dir * self.speed, 0.0), target_y: from.y };
                    return;
                };
                // Glide in, slowing all the way, keeping height at first and then dropping onto it
                let u = (timer / duration).min(1.0);
                let ease_out = 1.0 - (1.0 - u) * (1.0 - u);
                let smooth = u * u * (3.0 - 2.0 * u);
                let target = vec2(perch.x, y - PERCH_DROP);
                self.pos = vec2(from.x + (target.x - from.x) * ease_out, from.y + (target.y - from.y) * smooth);
                self.facing = self.dir;
                if u > 0.75 {
                    self.anim.play(&FLARE);
                }
                if u >= 1.0 {
                    self.kick(ctx, perch);
                    self.state = State::Perched { perch, timer: ctx.rng.pair(PERCH_TIME) };
                    self.anim.restart(&PERCHED);
                } else {
                    self.state = State::Approach { perch, from, timer, duration };
                }
            }
            State::Perched { perch, timer } => {
                if !self.ride(limbs, perch) {
                    self.take_off(ctx, perch, self.dir);
                    return;
                }
                ctx.pushes.push(Push::Load { limb: perch.limb, x: perch.x, weight: WEIGHT });
                let kitty = ctx.kitty.body.center();
                let d = kitty - self.pos;
                if d.x.abs() < PERCH_WATCH && d.y.abs() < PERCH_WATCH {
                    self.face(kitty.x);
                }
                let on_limb = ctx.kitty.standing_on == Some(Support::Limb(perch.limb));
                let below = d.x.abs() < PERCH_DIVE_ACROSS && d.y > PERCH_DIVE_BELOW.0 && d.y < PERCH_DIVE_BELOW.1;
                if on_limb && d.x.abs() < FLEE_RANGE {
                    let away = if d.x > 0.0 { -1.0 } else { 1.0 };
                    self.take_off(ctx, perch, away);
                } else if below && self.cooldown <= 0.0 && ctx.view.contains(self.pos) {
                    self.state = State::PerchTell { perch, timer: 0.0 };
                    self.anim.restart(&CAW);
                    ctx.sfx.push(Sfx::Caw);
                } else if timer <= STEP {
                    self.take_off(ctx, perch, self.dir);
                } else {
                    self.state = State::Perched { perch, timer: timer - STEP };
                }
            }
            State::PerchTell { perch, timer } => {
                if !self.ride(limbs, perch) {
                    self.take_off(ctx, perch, self.dir);
                    return;
                }
                ctx.pushes.push(Push::Load { limb: perch.limb, x: perch.x, weight: WEIGHT });
                self.face(ctx.kitty.x);
                let timer = timer + STEP;
                if timer >= PERCH_TELL_TIME {
                    self.kick(ctx, perch);
                    let from_y = self.pos.y - TAKEOFF_RISE;
                    self.dive(ctx, from_y);
                } else {
                    self.state = State::PerchTell { perch, timer };
                }
            }
            State::Hurt { timer, mut perch } => {
                if let Some(p) = perch {
                    if self.ride(limbs, p) {
                        ctx.pushes.push(Push::Load { limb: p.limb, x: p.x, weight: WEIGHT });
                    } else {
                        perch = None;
                    }
                }
                let timer = timer + STEP;
                if timer >= HURT_TIME {
                    self.fall();
                    return;
                }
                self.state = State::Hurt { timer, perch };
            }
            State::Falling(_) => unreachable!("handled above"),
        }

        if self.gone(ctx.view) {
            self.alive = false;
        }
    }

    fn animate(&mut self, dt: f32) {
        self.anim.update(dt);
    }

    fn draw(&self, assets: &Assets) {
        let (frame, at) = self.frame();
        match &self.state {
            State::Falling(tumble) => tumble.draw(&assets.crow, frame, self.facing),
            State::Hurt { timer, .. } => assets.crow.draw(frame, at, self.facing, hurt_tint(*timer)),
            _ => assets.crow.draw(frame, at, self.facing, WHITE),
        }
    }

    /// The beak hurts whatever it's doing, until it's stomped.
    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        let (frame, at) = self.frame();
        self.healthy() && assets.crow.strikes(frame, at, self.facing, body)
    }

    fn stomp_box(&self) -> Option<Rect> {
        let area = if self.frame().0.starts_with("perched") { PERCH_STOMP_AREA } else { STOMP_AREA };
        self.healthy().then(|| faced(area, self.pos, self.facing))
    }

    fn in_front(&self) -> bool {
        matches!(self.state, State::Dive { .. })
    }

    fn stomped(&mut self) {
        let perch = match self.state {
            State::Perched { perch, .. } | State::PerchTell { perch, .. } => Some(perch),
            _ => None,
        };
        if self.healthy() {
            self.state = State::Hurt { timer: 0.0, perch };
            self.anim.restart(&HURT);
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn position(&self) -> Vec2 {
        self.pos
    }

    fn kind(&self) -> Kind {
        Kind::Crow
    }
}
