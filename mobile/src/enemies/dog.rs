//! Dog, ported from the pygame game's platform.py: lives on the ground, sniffs about and scratches,
//! snarls then charges at Kitty on its level, barks up at her overhead, and leaps onto branches
//! low enough to reach, bending them with its weight. Only its paws and jaws hurt.

use macroquad::prelude::*;

use super::{Enemy, EnemyCtx, Kind, Sfx};
use crate::consts::*;
use crate::kitty::{Push, Support};
use crate::limb::{Limb, LimbId};
use crate::rng::Rng;
use crate::sprites::{Animator, Assets, Clip};

/// Pushes on the branch underfoot (Kitty is 38000): a thin branch holds Kitty or a dog, not both.
const WEIGHT: f32 = 18000.0;
const WALK_SPEED: f32 = 45.0; // px/s sniffing along, scaled by the level's dog speed
const CHARGE_SPEED: f32 = 190.0; // px/s galloping at Kitty
const GRAVITY: f32 = 1800.0; // px/s^2
/// Tallest leap, measured from the ground so dogs can't climb the tree branch by branch.
const JUMP_HEIGHT: f32 = 170.0;
const JUMP_CLEARANCE: f32 = 30.0; // px a leap rises above the higher of its two ends
const MAX_LEAP_SPEED: f32 = 300.0; // px/s sideways
const HOP_DOWN: f32 = 70.0; // px out from where it stood that a dog jumping down lands
const EXPLORE_REACH: f32 = 30.0; // px ahead along a branch overhead it hops up to
/// Catching a branch: px it may spring up past the falling dog's feet and still hold it.
const LAND_CATCH: f32 = 4.0;
const LAND_KICK: f32 = LANDING_KICK * 8.0;
const SIGHT: f32 = 260.0; // px; Kitty this close on the same level gets snarled at, then chased
const SAME_LEVEL: f32 = 45.0; // px between feet that still counts as the same level
const SNARL_TIME: f32 = 0.5; // seconds of snarling before the charge: the player's cue
const BARK_RANGE: f32 = 160.0; // px sideways; Kitty above and within this gets barked at
const BARK_HEIGHT: f32 = 500.0; // px; higher than this the dog loses interest
const BARK_BEFORE_JUMP: f32 = 0.8; // seconds of barking before leaping up at Kitty's branch
/// Barking plays a woof at most this often, so a pack under Kitty isn't a wall of noise.
const BARK_SOUND_GAP: f32 = 1.2;
const SCRATCH_EVERY: (f32, f32) = (5.0, 10.0);
const EXPLORE_CHANCE: f32 = 0.15; // per second, while under a low branch, of hopping up to look
const BRANCH_STAY: (f32, f32) = (4.0, 8.0); // seconds a wandering dog stays up before hopping down
const GROUND_INSET: f32 = 10.0; // keeps the dog this far inside the world's edges
const AIR_POSE_SPEED: f32 = 150.0; // px/s up or down past which it shows the rising/dropping pose
const HALF_HEIGHT: f32 = 32.0; // of its 80x64 body, for `position`

// Built from the frames sliced out of the dog sheets (as in the pygame game)
static WALK: Clip = Clip { frames: &["sniff_1", "sniff_2", "sniff_3", "sniff_4"], fps: 5.0, looped: true };
static SCRATCH: Clip = Clip {
    frames: &["scratch_0", "scratch_1", "scratch_2", "scratch_3", "scratch_2", "scratch_3", "scratch_4", "scratch_5"],
    fps: 6.0,
    looped: false,
};
static SNARL: Clip = Clip { frames: &["menace_0", "menace_4", "menace_5", "menace_4"], fps: 6.0, looped: true };
static RUN: Clip = Clip {
    frames: &["run_a_0", "run_a_1", "run_a_2", "run_a_3", "run_b_0", "run_b_1", "run_b_2", "run_b_3"],
    fps: 16.0,
    looped: true,
};
static BARK: Clip = Clip { frames: &["bark_0", "bark_1", "bark_2", "bark_1"], fps: 6.0, looped: true };
static PREP: Clip = Clip { frames: &["prep_1", "prep_2", "prep_3"], fps: 10.0, looped: false };
static RISE: Clip = Clip { frames: &["air_0", "air_1"], fps: 8.0, looped: false };
static SOAR: Clip = Clip { frames: &["air_2"], fps: 1.0, looped: true };
static DROP: Clip = Clip { frames: &["air_3", "air_4"], fps: 8.0, looped: false };
static LAND: Clip = Clip { frames: &["land_0", "land_1", "land_3", "land_4"], fps: 12.0, looped: false };
static CRASH: Clip = Clip { frames: &["land_0", "land_2", "land_2", "land_3", "land_4"], fps: 8.0, looped: false };

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Walk,
    Scratch,
    Snarl,
    Run,
    Bark,
    /// Crouching to leap at `jump_target`.
    Prep,
    Air,
    Land,
    /// Landing in a heap after the branch snapped under it.
    Crash,
}

pub struct Dog {
    x: f32,
    y: f32, // feet
    speed_scale: f32,
    /// What it's standing on; None while airborne.
    support: Option<Support>,
    vx: f32,
    vy: f32,
    facing: f32,
    state: State,
    state_time: f32,
    next_scratch: f32,
    branch_time: f32,
    branch_stay: f32,
    jump_target: Option<Support>,
    jump_x: f32,
    crashed: bool,
    bark_cooldown: f32,
    animator: Animator,
}

/// Top of `support` under `x`; None off its end, or anywhere on a snapped limb.
fn surface(support: Support, limbs: &[Limb], x: f32) -> Option<f32> {
    match support {
        Support::Ground => (0.0..=WORLD_W).contains(&x).then_some(GROUND_Y),
        Support::Limb(id) if limbs[id].broken() => None,
        Support::Limb(id) => limbs[id].surface_y(x),
    }
}

fn walk_range(support: Support, limbs: &[Limb]) -> (f32, f32) {
    match support {
        Support::Ground => (GROUND_INSET, WORLD_W - GROUND_INSET),
        Support::Limb(id) => limbs[id].walk_range(),
    }
}

impl Dog {
    pub fn new(x: f32, speed_scale: f32) -> Self {
        // The spawner picked `x` at random, so it seeds this dog's quirks too
        let mut rng = Rng::new(x.to_bits() as u64);
        Self {
            x: x.clamp(GROUND_INSET, WORLD_W - GROUND_INSET),
            y: GROUND_Y,
            speed_scale,
            support: Some(Support::Ground),
            vx: 0.0,
            vy: 0.0,
            facing: rng.sign(),
            state: State::Walk,
            state_time: 0.0,
            next_scratch: rng.pair(SCRATCH_EVERY),
            branch_time: 0.0,
            branch_stay: rng.pair(BRANCH_STAY),
            jump_target: None,
            jump_x: x,
            crashed: false,
            bark_cooldown: 0.0,
            animator: Animator::new(&WALK),
        }
    }

    fn set_state(&mut self, state: State) {
        if state != self.state {
            self.state = state;
            self.state_time = 0.0;
        }
    }

    // ------------------ On the ground or a branch -------------------

    fn think(&mut self, ctx: &mut EnemyCtx, support: Support) {
        let snapped = matches!(support, Support::Limb(id) if ctx.limbs[id].broken());
        let Some(top) = surface(support, ctx.limbs, self.x) else {
            // The branch snapped, or the dog stepped off its end
            self.take_off(0.0, 0.0, snapped);
            return;
        };
        // Ride the branch as it bends, and bend it
        self.y = top;
        if let Support::Limb(id) = support {
            ctx.pushes.push(Push::Load { limb: id, x: self.x, weight: WEIGHT });
        }

        match self.state {
            State::Prep => {
                if self.state_time >= PREP.duration() {
                    self.launch(ctx.limbs);
                }
                return;
            }
            State::Land if self.state_time < LAND.duration() => return,
            State::Crash if self.state_time < CRASH.duration() => return,
            _ => {}
        }

        let kitty = ctx.kitty;
        let dx = kitty.x - self.x;
        let toward = if dx > 0.0 { 1.0 } else { -1.0 };
        let same_level = (kitty.y - self.y).abs() < SAME_LEVEL;
        let above = self.y - BARK_HEIGHT < kitty.y && kitty.y < self.y - SAME_LEVEL && dx.abs() < BARK_RANGE;
        let below = kitty.y > self.y + SAME_LEVEL && dx.abs() < SIGHT;
        let (lo, hi) = walk_range(support, ctx.limbs);

        if same_level && dx.abs() < SIGHT {
            self.facing = toward;
            if !matches!(self.state, State::Snarl | State::Run) {
                self.set_state(State::Snarl);
            } else if self.state == State::Snarl && self.state_time >= SNARL_TIME {
                self.set_state(State::Run);
            }
            if self.state == State::Run && !self.trot(CHARGE_SPEED, lo, hi, false) {
                self.set_state(State::Snarl); // cornered at the end of the branch
            }
            return;
        }
        if above {
            self.facing = toward;
            self.set_state(State::Bark);
            if self.bark_cooldown <= 0.0 {
                ctx.sfx.push(Sfx::Bark);
                self.bark_cooldown = BARK_SOUND_GAP;
            }
            let kitty_limb = match kitty.standing_on {
                Some(Support::Limb(id)) => Some(id),
                Some(Support::Ground) => None,
                None => kitty.hanging_on,
            };
            if let Some(id) = kitty_limb.filter(|&id| self.reachable(ctx.limbs, id)) {
                if self.state_time >= BARK_BEFORE_JUMP {
                    let (tlo, thi) = ctx.limbs[id].walk_range();
                    self.prepare_jump(Support::Limb(id), kitty.x.clamp(tlo, thi));
                }
            }
            return;
        }
        if below && support != Support::Ground {
            self.facing = toward;
            self.prepare_jump(Support::Ground, self.x + toward * HOP_DOWN);
            return;
        }

        // Nothing to chase: wander, scratch, explore
        if self.state == State::Scratch {
            if self.state_time >= SCRATCH.duration() {
                self.set_state(State::Walk);
                self.next_scratch = ctx.rng.pair(SCRATCH_EVERY);
            }
            return;
        }
        self.next_scratch -= STEP;
        if self.next_scratch <= 0.0 {
            self.set_state(State::Scratch);
            return;
        }
        self.set_state(State::Walk);
        self.trot(WALK_SPEED, lo, hi, true);
        if support == Support::Ground {
            if ctx.rng.chance(EXPLORE_CHANCE * STEP) {
                let overhead = (0..ctx.limbs.len())
                    .find(|&id| self.reachable(ctx.limbs, id) && ctx.limbs[id].surface_y(self.x).is_some());
                if let Some(id) = overhead {
                    let (blo, bhi) = ctx.limbs[id].walk_range();
                    self.prepare_jump(Support::Limb(id), (self.x + self.facing * EXPLORE_REACH).clamp(blo, bhi));
                }
            }
        } else {
            self.branch_time += STEP;
            if self.branch_time > self.branch_stay {
                self.prepare_jump(Support::Ground, self.x + self.facing * HOP_DOWN);
            }
        }
    }

    /// Trot along; returns false when stopped at the end of the walkable span (turning back
    /// there if `turn`).
    fn trot(&mut self, speed: f32, lo: f32, hi: f32, turn: bool) -> bool {
        self.x += self.facing * speed * self.speed_scale * STEP;
        if (lo..=hi).contains(&self.x) {
            return true;
        }
        self.x = self.x.clamp(lo, hi);
        if turn {
            self.facing = if self.x <= lo { 1.0 } else { -1.0 };
        }
        false
    }

    /// Is limb `id` an intact branch above the dog and low enough to leap onto?
    fn reachable(&self, limbs: &[Limb], id: LimbId) -> bool {
        let limb = &limbs[id];
        if self.support == Some(Support::Limb(id)) || limb.broken() {
            return false;
        }
        let (lo, hi) = limb.walk_range();
        match limb.surface_y(self.x.clamp(lo, hi)) {
            Some(top) => top < self.y && GROUND_Y - top <= JUMP_HEIGHT,
            None => false,
        }
    }

    // ------------------ Jumping and falling -------------------

    fn prepare_jump(&mut self, target: Support, x: f32) {
        let x = if target == Support::Ground { x.clamp(GROUND_INSET, WORLD_W - GROUND_INSET) } else { x };
        self.jump_target = Some(target);
        self.jump_x = x;
        self.facing = if x >= self.x { 1.0 } else { -1.0 };
        self.set_state(State::Prep);
    }

    /// Leap on an arc that clears the target's surface and comes down on it at `jump_x`.
    fn launch(&mut self, limbs: &[Limb]) {
        let Some(target_y) = self.jump_target.and_then(|t| surface(t, limbs, self.jump_x)) else {
            self.jump_target = None;
            self.set_state(State::Walk);
            return;
        };
        let rise = self.y - target_y; // positive when jumping up
        let apex = rise.max(0.0) + JUMP_CLEARANCE;
        let vy = -(2.0 * GRAVITY * apex).sqrt();
        let flight = -vy / GRAVITY + (2.0 * (apex - rise) / GRAVITY).sqrt();
        let vx = ((self.jump_x - self.x) / flight).clamp(-MAX_LEAP_SPEED, MAX_LEAP_SPEED);
        self.take_off(vx, vy, false);
    }

    fn take_off(&mut self, vx: f32, vy: f32, crashed: bool) {
        self.support = None;
        self.vx = vx;
        self.vy = vy;
        self.crashed = crashed;
        if crashed {
            self.jump_target = None;
        }
        self.set_state(State::Air);
    }

    /// Fly the arc; only the leap's target or the ground can catch the dog, and only coming down.
    fn fly(&mut self, ctx: &mut EnemyCtx) {
        let (lo, hi) = walk_range(Support::Ground, ctx.limbs);
        let previous_y = self.y;
        self.vy += GRAVITY * STEP;
        self.x = (self.x + self.vx * STEP).clamp(lo, hi);
        self.y += self.vy * STEP;
        if self.vy <= 0.0 {
            return;
        }
        for candidate in self.jump_target.into_iter().chain([Support::Ground]) {
            let Some(top) = surface(candidate, ctx.limbs, self.x) else { continue };
            if previous_y <= top + LAND_CATCH && self.y >= top {
                self.support = Some(candidate);
                self.y = top;
                self.vx = 0.0;
                self.vy = 0.0;
                self.branch_time = 0.0;
                self.branch_stay = ctx.rng.pair(BRANCH_STAY);
                if let Support::Limb(id) = candidate {
                    ctx.pushes.push(Push::Kick { limb: id, x: self.x, speed: LAND_KICK });
                }
                self.set_state(if self.crashed { State::Crash } else { State::Land });
                self.crashed = false;
                self.jump_target = None;
                return;
            }
        }
    }
}

impl Enemy for Dog {
    fn step(&mut self, ctx: &mut EnemyCtx) {
        self.state_time += STEP;
        self.bark_cooldown = (self.bark_cooldown - STEP).max(0.0);
        match self.support {
            None => self.fly(ctx),
            Some(support) => self.think(ctx, support),
        }
    }

    fn animate(&mut self, dt: f32) {
        let clip = match self.state {
            State::Walk => &WALK,
            State::Scratch => &SCRATCH,
            State::Snarl => &SNARL,
            State::Run => &RUN,
            State::Bark => &BARK,
            State::Prep => &PREP,
            State::Land => &LAND,
            State::Crash => &CRASH,
            State::Air if self.vy < -AIR_POSE_SPEED => &RISE,
            State::Air if self.vy > AIR_POSE_SPEED => &DROP,
            State::Air => &SOAR,
        };
        self.animator.play(clip);
        self.animator.update(dt);
    }

    fn draw(&self, assets: &Assets) {
        assets.dog.draw(self.animator.frame(), vec2(self.x, self.y), self.facing, WHITE);
    }

    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        assets.dog.strikes(self.animator.frame(), vec2(self.x, self.y), self.facing, body)
    }

    fn alive(&self) -> bool {
        true
    }

    fn position(&self) -> Vec2 {
        vec2(self.x, self.y - HALF_HEIGHT)
    }

    fn kind(&self) -> Kind {
        Kind::Dog
    }
}
