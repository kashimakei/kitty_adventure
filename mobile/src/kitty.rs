//! Kitty, ported from the pygame game's kitty.py and grown from there: analog walking and running,
//! a hold-to-charge pounce on a natural arc, branches that act like trampolines, dropping through
//! limbs, hearts with knockback, and stomp bounces. Movement is per 60 Hz step.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::*;

use crate::consts::*;
use crate::input::Controls;
use crate::limb::{Limb, LimbId};
use crate::rng::Rng;
use crate::sprites::{Assets, Clip};

/// What Kitty is standing on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Support {
    Ground,
    Limb(LimbId),
}

/// A load or jolt for the world to apply (it also passes loads on forks down to their parents).
#[derive(Clone, Copy, Debug)]
pub enum Push {
    Load { limb: LimbId, x: f32, weight: f32 },
    Kick { limb: LimbId, x: f32, speed: f32 },
    /// A hard landing's sudden extra load at `x` on top of the weight, and the part of it that
    /// lingers (see `Limb::jolt`).
    Jolt { limb: LimbId, x: f32, force: f32, fatigue: f32 },
}

#[derive(Clone, Copy, PartialEq)]
enum Pin {
    Feet,
    Grip,
}

struct KittyClip {
    clip: Clip,
    pin: Pin,
    /// Body hurtbox: (px forward of the anchor, px from the pin, width, height)
    body: (f32, f32, f32, f32),
}

const BODY: (f32, f32, f32, f32) = (0.0, 0.0, 54.0, 64.0);
/// Crouched low to pounce: longer and much lower.
const CROUCH_BODY: (f32, f32, f32, f32) = (6.0, 0.0, 60.0, 46.0);

macro_rules! kclip {
    ($frames:expr, $fps:expr, $looped:expr) => {
        KittyClip { clip: Clip { frames: $frames, fps: $fps, looped: $looped }, pin: Pin::Feet, body: BODY }
    };
    ($frames:expr, $fps:expr, $looped:expr, $pin:expr, $body:expr) => {
        KittyClip { clip: Clip { frames: $frames, fps: $fps, looped: $looped }, pin: $pin, body: $body }
    };
}

// Kitty's moves, built from the frames sliced out of assets/source/ (as in the pygame game)
static SIT: KittyClip = kclip!(&["sit_0"], 1.0, true);
static WALK: KittyClip = kclip!(&["walk_0", "walk_1", "walk_2", "walk_3"], 10.0, true);
static RUN_START: KittyClip = kclip!(&["run_start_0"], 8.0, false);
static RUN: KittyClip = kclip!(&["run_0", "run_1", "run_2"], 12.0, true);
/// Crouched and charging. The frame follows the charge rather than the clock (see `frame`).
static CHARGE: KittyClip = kclip!(&["crouch_0", "crouch_1", "crouch_2", "pounce_1"], 1.0, true, Pin::Feet, CROUCH_BODY);
/// Springing off, then tucked
static RISE: KittyClip = kclip!(&["pounce_2", "mid_jump_0"], 8.0, false);
static APEX: KittyClip = kclip!(&["mid_jump_1"], 1.0, true);
static FALL: KittyClip = kclip!(&["mid_jump_2"], 1.0, true);
/// Dropping through a branch: a quick duck, then down
static DROP: KittyClip = kclip!(&["jump_prep_0", "mid_jump_2"], 8.0, false);
static LAND: KittyClip = kclip!(&["land_0", "land_1", "land_2"], 14.0, false);
static PANIC: KittyClip = kclip!(&["dangle_0", "dangle_1"], 6.0, true, Pin::Feet, (0.0, 40.0, 46.0, 70.0));
static GROOM: KittyClip = kclip!(&["clean_0", "clean_1", "clean_2", "clean_1", "clean_2", "groom_1", "groom_1", "sit_alert_0"], 5.0, false);
static WAVE: KittyClip = kclip!(&["wave_0", "wave_0", "wave_0", "sit_0"], 3.0, false);
static SLEEP: KittyClip = kclip!(&["sleep_0", "sleep_1"], 1.2, true, Pin::Feet, (0.0, 0.0, 64.0, 48.0));
static GRAB: KittyClip = kclip!(&["hang_0"], 5.0, false, Pin::Grip, (-14.0, 24.0, 46.0, 64.0));
static HANG: KittyClip = kclip!(&["hang_1", "hang_2", "hang_3", "hang_2"], 4.0, true, Pin::Grip, (-12.0, 22.0, 44.0, 62.0));
static LET_GO: KittyClip = kclip!(&["let_go_0", "let_go_1"], 6.0, false, Pin::Feet, (0.0, 40.0, 46.0, 70.0));
static CLIMB: KittyClip = kclip!(&["climb_0", "climb_1", "climb_2", "climb_3", "climb_4"], 5.0 * 60.0 / CLIMB_STEPS as f32, false);
static CLIMB_DONE: KittyClip = kclip!(&["climb_5"], 3.0, false);

/// The crouch deepening as the charge builds: each frame up to its charge level.
const CROUCH: [(f32, &str); 3] = [(0.2, "crouch_0"), (0.55, "crouch_1"), (1.0, "crouch_2")];
/// Fully charged: a butt wiggle between these, with a little squash and sway.
const WIGGLE: [&str; 2] = ["crouch_2", "pounce_1"];
const WIGGLE_FPS: f32 = 6.0;
const WIGGLE_SQUASH: f32 = 0.04;
const WIGGLE_SWAY: f32 = 0.035;
/// How much the crouch squashes her as it deepens, before the wiggle.
const CROUCH_SQUASH: f32 = 0.05;

/// She runs above this share of top speed, and is back to walking below `WALK_BELOW`.
const RUN_ABOVE: f32 = 0.7;
const WALK_BELOW: f32 = 0.6;
/// The walk cycle's paws keep pace with the ground at this share of top speed; slower or faster
/// walking plays it slower or faster (within `WALK_RATE`), and the run plays at its speed.
const WALK_CYCLE_SPEED: f32 = 0.5;
const WALK_RATE: (f32, f32) = (0.4, 1.6);
const RUN_RATE_MIN: f32 = 0.75;
const SLEEP_AFTER: f32 = 15.0;
const FIDGET_DELAY: (f32, f32) = (3.0, 7.0);

/// The charge ring round her feet: radii, how far back from her front paws (her position) it's
/// centred so it sits under her whole crouch, line width, and how many pieces it's drawn in.
const RING_RADII: (f32, f32) = (40.0, 11.0);
const RING_BACK: f32 = 16.0;
const RING_WIDTH: f32 = 4.0;
const RING_PIECES: usize = 48;
/// Its colours (shared with the pictures on the how-to-play and pause cards): the empty track,
/// filling, fully charged, and glowing while a release would ride the branch's rebound.
pub const RING_TRACK: Color = Color::new(1.0, 1.0, 1.0, 0.22);
pub const RING_FILL: Color = Color::new(1.0, 0.8, 0.3, 0.9);
pub const RING_FULL: Color = Color::new(1.0, 0.62, 0.2, 1.0);
pub const RING_SWEET: Color = Color::new(0.72, 0.98, 1.0, 1.0);

/// Whether limbs `a` and `b` are one piece of wood: the same limb, or a fork and its parent.
fn joined(limbs: &[Limb], a: LimbId, b: LimbId) -> bool {
    let parent = |id: LimbId| limbs[id].attachment.map(|attached| attached.parent);
    a == b || parent(a) == Some(b) || parent(b) == Some(a)
}

/// Dropping through a limb (or falling from one that gave way): which one, the limbs that can't
/// catch her (it, and any level with her as she left), the height she left at, and steps since.
#[derive(Clone)]
struct DropThrough {
    limb: LimbId,
    skip: Vec<LimbId>,
    from_y: f32,
    steps: u32,
}

#[derive(Clone)]
pub struct Kitty {
    pub x: f32,
    pub y: f32, // feet
    prev_y: f32,
    pub velocity: f32,
    /// In the air under gravity: leaping, falling or knocked back.
    pub falling: bool,
    pub facing: f32,
    /// Walking or running this step, -1..1.
    move_x: f32,
    pub standing_on: Option<Support>,
    pub hanging_on: Option<LimbId>,
    pub climbing_on: Option<LimbId>,
    climb_step: u32,
    released_from: Option<LimbId>,
    released_steps: u32,
    pub panicking: bool,
    pub hearts: u32,
    pub invulnerable: u32,
    knock_vx: f32,
    pub pushes: Vec<Push>,
    // The charge jump
    /// Steps the jump has been held, while it is (it stops growing at `CHARGE_STEPS`).
    charge: Option<u32>,
    /// Released in the air: that charge, and the steps left in which landing still leaps.
    buffered: Option<(u32, u32)>,
    /// Steps left, after walking off an edge, in which a release still leaps.
    coyote: u32,
    /// Pressed while hanging: steps the press has been held so far.
    climb_wait: Option<u32>,
    /// A press made while hanging or climbing (or a charge she caught a branch with), still held:
    /// once she's nearly up, it becomes a crouch, so the release leaps.
    hold_carry: bool,
    /// The limb she last leapt from, until she lands: landing back on it braced is a bounce.
    leapt_from: Option<LimbId>,
    drop_through: Option<DropThrough>,
    /// Dropped through a limb and not caught yet (for the pose).
    dropping: bool,
    /// Leapt and not yet landed or caught (for the pose).
    leapt: bool,
    /// Charging on wood that's springing up: a release now rides the rebound.
    pub sweet_spot: bool,
    /// Steps since she landed, while her legs take her weight (it comes on over `LANDING_STEPS`).
    on_feet: u32,
    // Animation
    clip: &'static KittyClip,
    clip_time: f32,
    running: bool,
    idle_time: f32,
    next_fidget: f32,
    /// Seconds spent fully charged, for the wiggle.
    full_time: f32,
    just_landed: bool,
    just_grabbed: bool,
    just_climbed: bool,
    just_leapt: bool,
    pub jumped: bool, // this step, for sound
}

impl Kitty {
    pub fn new(x: f32) -> Self {
        Self {
            x,
            y: GROUND_Y,
            prev_y: GROUND_Y,
            velocity: 0.0,
            falling: false,
            facing: 1.0,
            move_x: 0.0,
            standing_on: Some(Support::Ground),
            hanging_on: None,
            climbing_on: None,
            climb_step: 0,
            released_from: None,
            released_steps: 0,
            panicking: false,
            hearts: MAX_HEARTS,
            invulnerable: 0,
            knock_vx: 0.0,
            pushes: Vec::new(),
            charge: None,
            buffered: None,
            coyote: 0,
            climb_wait: None,
            hold_carry: false,
            leapt_from: None,
            drop_through: None,
            dropping: false,
            leapt: false,
            sweet_spot: false,
            on_feet: LANDING_STEPS,
            clip: &SIT,
            clip_time: 0.0,
            running: false,
            idle_time: 0.0,
            next_fidget: 4.0,
            full_time: 0.0,
            just_landed: false,
            just_grabbed: false,
            just_climbed: false,
            just_leapt: false,
            jumped: false,
        }
    }

    /// The movement box (as in the pygame game): 100x100, feet at its bottom centre.
    pub fn rect(&self) -> Rect {
        Rect::new(self.x - KITTY_BOX.0 / 2.0, self.y - KITTY_BOX.1, KITTY_BOX.0, KITTY_BOX.1)
    }

    fn top(&self) -> f32 {
        self.y - KITTY_BOX.1
    }

    fn feet(&self) -> [f32; 3] {
        [self.x - KITTY_FOOT_REACH, self.x, self.x + KITTY_FOOT_REACH]
    }

    /// The paw bearing down on `limb`: the middle one if it's on the wood, else an outer one.
    fn paw_on(&self, limb: &Limb) -> Option<f32> {
        let [left, middle, right] = self.feet();
        [middle, left, right].into_iter().find(|&x| limb.surface_y(x).is_some())
    }

    pub fn airborne(&self) -> bool {
        self.standing_on.is_none() && self.hanging_on.is_none() && self.climbing_on.is_none()
    }

    /// Crouched on something, charging a leap.
    pub fn crouching(&self) -> bool {
        self.charge.is_some() && self.standing_on.is_some()
    }

    /// How far the charge has built, 0..1, while the jump is held.
    pub fn charge_level(&self) -> Option<f32> {
        self.charge.map(|steps| (steps as f32 / CHARGE_STEPS as f32).min(1.0))
    }

    /// The part of Kitty that can be hurt: torso and head, not the tail or ear tips.
    pub fn body(&self) -> Rect {
        let (ahead, offset, w, h) = self.clip.body;
        let x = self.x + ahead * self.facing;
        match self.clip.pin {
            Pin::Grip => Rect::new(x - w / 2.0, self.top() + offset, w, h),
            Pin::Feet => Rect::new(x - w / 2.0, self.y - offset - h, w, h),
        }
    }

    /// Feet rectangle used to stomp small enemies.
    pub fn stomp_feet(&self) -> Rect {
        Rect::new(self.x - 26.0, self.y - 14.0, 52.0, 16.0)
    }

    // ------------------ The jump and drop controls -------------------

    /// This step's presses, releases and drops. Held in the air, the jump charges ahead of
    /// landing; released just before landing, or just after walking off an edge, it still leaps
    /// (the landing leap comes in `resolve`).
    pub fn input(&mut self, controls: &Controls, limbs: &[Limb]) {
        self.jumped = false;
        if let Some((charge, left)) = self.buffered {
            self.buffered = (left > 0).then(|| (charge, left - 1));
        }
        // A press and a release in one step are a quick tap, press first, unless she was already
        // charging: then it's the release, and a fresh press after it
        let repress = controls.press && controls.leap && controls.charge_held && self.charge.is_some();
        if controls.press && !repress {
            self.press();
        }
        if controls.drop {
            self.drop(limbs);
        }
        if controls.leap {
            self.release(limbs);
        } else if !controls.charge_held {
            // Swiped away, or never really pressed: no leap
            self.charge = None;
            self.climb_wait = None;
        }
        if repress {
            self.press();
        }
        if !controls.charge_held {
            self.hold_carry = false;
        } else if self.hold_carry && self.charge.is_none() && self.climb_wait.is_none() {
            // Still holding the press she climbed with (or made while climbing): crouch as soon
            // as she's nearly up, as a press then would
            let nearly_up = self.climbing_on.is_some() && self.climb_step + CLIMB_CARRY_STEPS >= CLIMB_STEPS;
            if self.standing_on.is_some() || nearly_up {
                self.charge = Some(0);
                self.hold_carry = false;
            }
        }
        if controls.charge_held {
            if let Some(steps) = &mut self.charge {
                *steps = (*steps + 1).min(CHARGE_STEPS);
            }
            if let Some(held) = &mut self.climb_wait {
                *held += 1;
                if *held >= CLIMB_HOLD_STEPS {
                    self.climb_up();
                }
            }
        }
    }

    fn press(&mut self) {
        if self.hanging_on.is_some() {
            self.climb_wait.get_or_insert(0);
            self.hold_carry = true;
        } else if self.climbing_on.is_some() {
            // Nearly up: the press carries over into a charge for when she's on her feet. Earlier,
            // it does too if it's still held by then (a quick tap is let go first and does nothing)
            if self.climb_step + CLIMB_CARRY_STEPS >= CLIMB_STEPS {
                self.charge = Some(0);
            } else {
                self.hold_carry = true;
            }
        } else {
            self.charge = Some(0);
            self.buffered = None;
        }
    }

    fn release(&mut self, limbs: &[Limb]) {
        if self.climb_wait.take().is_some() {
            self.climb_up();
            return;
        }
        let Some(charge) = self.charge.take() else { return };
        if self.standing_on.is_some() || (self.coyote > 0 && self.airborne()) {
            self.leap(charge, limbs);
        } else {
            // In the air, or finishing a climb: leap the moment she's on her feet
            let wait = if self.climbing_on.is_some() { CLIMB_CARRY_STEPS + LEAP_BUFFER_STEPS } else { LEAP_BUFFER_STEPS };
            self.buffered = Some((charge, wait));
        }
    }

    /// Spring off with the power of `charge` steps of crouching, less what a bendy limb soaks up
    /// (it whips down as she leaves), plus however fast the wood under her paws is rising (or
    /// minus, sinking).
    fn leap(&mut self, charge: u32, limbs: &[Limb]) {
        let t = (charge as f32 / CHARGE_STEPS as f32).min(1.0);
        let (tap, full) = LEAP_SPEED;
        let legs = tap + (full - tap) * (1.0 - (1.0 - t).powi(2));
        let mut speed = legs;
        self.leapt_from = None;
        if let Some(Support::Limb(id)) = self.standing_on {
            self.leapt_from = Some(id);
            let limb = &limbs[id];
            if let Some(paw) = self.paw_on(limb) {
                let lost = legs * LEAP_LOSS * limb.give_at(paw);
                speed = legs - lost - limb.surface_velocity(paw) * SURFACE_CARRY;
                self.pushes.push(Push::Kick { limb: id, x: paw, speed: lost * PUSH_KICK });
            }
        }
        self.standing_on = None;
        self.falling = true;
        self.velocity = -speed.max(tap * 0.5);
        self.charge = None;
        self.buffered = None;
        self.hold_carry = false;
        self.coyote = 0;
        self.jumped = true;
        self.leapt = true;
        self.just_leapt = true;
    }

    /// Swiped down: let go of the branch she's hanging from, or drop through the one she's
    /// standing on (nothing happens on the ground). Either way the charge is off.
    fn drop(&mut self, limbs: &[Limb]) {
        self.charge = None;
        self.buffered = None;
        self.climb_wait = None;
        self.hold_carry = false;
        if self.hanging_on.is_some() || self.climbing_on.is_some() {
            self.let_go(false);
        } else if let Some(Support::Limb(id)) = self.standing_on {
            if limbs[id].gone() {
                return;
            }
            self.standing_on = None;
            self.falling = true;
            self.velocity = DROP_SPEED;
            // Nor can she catch hold of it as it passes her face
            self.released_from = Some(id);
            self.released_steps = REGRAB_STEPS;
            let feet = self.feet();
            let level = |other: &Limb| other.surface_under(&feet).is_some_and(|s| (s - self.y).abs() <= DROP_SKIP_NEAR);
            let skip = (0..limbs.len()).filter(|&other| other == id || level(&limbs[other])).collect();
            self.drop_through = Some(DropThrough { limb: id, skip, from_y: self.y, steps: 0 });
            self.dropping = true;
        }
    }

    /// The screen changed (paused, a life lost): forget a held or buffered jump.
    pub fn forget_input(&mut self) {
        self.charge = None;
        self.buffered = None;
        self.climb_wait = None;
        self.hold_carry = false;
    }

    // ------------------ Movement (per 60 Hz step) -------------------

    /// Walk or run at `move_x` (-1..1) of top speed, creeping while crouched and shuffling while
    /// hanging; in the air, fly her arc with the wind (`breeze`) pushing her about.
    pub fn step(&mut self, move_x: f32, breeze: f32) {
        self.prev_y = self.y;
        self.move_x = move_x.clamp(-1.0, 1.0);
        if self.move_x != 0.0 {
            self.facing = self.move_x.signum();
        }
        if self.released_steps > 0 {
            self.released_steps -= 1;
            if self.released_steps == 0 {
                self.released_from = None;
            }
        }
        self.invulnerable = self.invulnerable.saturating_sub(1);
        self.coyote = self.coyote.saturating_sub(1);
        if self.climbing_on.is_some() {
            return;
        }
        if self.hanging_on.is_some() {
            // Shuffle paw over paw along the branch
            self.x = (self.x + self.move_x * HANG_SHIMMY).clamp(KITTY_BOX.0 / 2.0, WORLD_W - KITTY_BOX.0 / 2.0);
            return;
        }
        let speed = if self.crouching() { KITTY_SPEED * CHARGE_CREEP } else { KITTY_SPEED };
        self.x += self.move_x * speed + self.knock_vx;
        self.knock_vx *= 0.88;
        if self.knock_vx.abs() < 0.2 {
            self.knock_vx = 0.0;
        }
        // The wind pushes Kitty around while airborne
        if self.falling {
            self.x += breeze * WIND_DRIFT;
        }
        self.x = self.x.clamp(KITTY_BOX.0 / 2.0, WORLD_W - KITTY_BOX.0 / 2.0);

        if self.falling {
            let mut gravity = if self.velocity < 0.0 { RISE_GRAVITY } else { FALL_GRAVITY };
            if self.velocity.abs() < APEX_FLOAT_SPEED {
                gravity *= 0.5;
            }
            self.velocity = (self.velocity + gravity).min(TERMINAL_SPEED);
        }
        // Walking off a support is noticed in `resolve`, which can see the limbs
        self.y += self.velocity;
        if let Some(drop) = &mut self.drop_through {
            drop.steps += 1;
            if drop.steps >= DROP_STEPS || self.y > drop.from_y + DROP_CLEAR {
                self.drop_through = None;
            }
        }
    }

    fn support_surface(&self, support: Support, limbs: &[Limb], xs: &[f32]) -> Option<f32> {
        match support {
            Support::Ground => xs.iter().any(|&x| (0.0..=WORLD_W).contains(&x)).then_some(GROUND_Y),
            Support::Limb(id) => limbs[id].surface_under(xs),
        }
    }

    /// Dropping through a limb: it, and anything level with where she left it, can't catch her,
    /// nor can the fork or parent it joins until she's well below (that wood springs up as her
    /// weight comes off it, and a fork's base is hidden inside its parent).
    fn passes_through(&self, limbs: &[Limb], support: Support, surface: f32) -> bool {
        match (&self.drop_through, support) {
            (Some(drop), Support::Limb(id)) => {
                drop.skip.contains(&id) || (joined(limbs, id, drop.limb) && surface < drop.from_y + DROP_JOINED_NEAR)
            }
            _ => false,
        }
    }

    /// Ride the support Kitty is on, hang from or climb a limb, or catch one while falling.
    /// Returns the support Kitty just landed or climbed onto, if any.
    pub fn resolve(&mut self, limbs: &[Limb]) -> Option<Support> {
        self.sweet_spot = false;
        if let Some(id) = self.climbing_on {
            let arrived = self.climb_step(limbs, id);
            if arrived.is_some() {
                if let Some((charge, _)) = self.buffered.take() {
                    self.leap(charge, limbs);
                }
            }
            return arrived;
        }
        if let Some(id) = self.hanging_on {
            // Shuffle in with the tip as the branch bends, rather than slip off its end (unless
            // the part she's holding has snapped off)
            if !limbs[id].fell_away(self.x) {
                let (lo, hi) = limbs[id].walk_range();
                self.x = self.x.clamp(lo, hi);
            }
            match limbs[id].surface_y(self.x) {
                None => self.let_go(limbs[id].gave_way()),
                // A low limb sagging under her has lowered her feet to the ground: stand there
                Some(s) if s - GRIP_DEPTH + KITTY_BOX.1 >= GROUND_Y => {
                    self.let_go(false);
                    self.land(Support::Ground, GROUND_Y);
                    return Some(Support::Ground);
                }
                Some(s) => {
                    self.y = s - GRIP_DEPTH + KITTY_BOX.1;
                    self.pushes.push(Push::Load { limb: id, x: self.x, weight: KITTY_WEIGHT });
                }
            }
            return None;
        }
        if let Some(support) = self.standing_on {
            // When a limb snaps under her, she goes down with the piece that falls, even if a
            // back paw is still on the stub
            let fell = matches!(support, Support::Limb(id) if limbs[id].just_snapped && limbs[id].fell_away(self.x));
            if let Support::Limb(id) = support {
                self.hold_tip(&limbs[id]);
            }
            let surface = if fell { None } else { self.support_surface(support, limbs, &self.feet()) };
            match surface {
                Some(s) => {
                    self.y = s;
                    if let Support::Limb(id) = support {
                        // Press down through whichever paw is on the branch, centre first
                        if let Some(x) = self.paw_on(&limbs[id]) {
                            self.on_feet = (self.on_feet + 1).min(LANDING_STEPS);
                            let legs = self.on_feet as f32 / LANDING_STEPS as f32;
                            let weight = if self.charge.is_some() { KITTY_WEIGHT * CHARGE_LOAD } else { KITTY_WEIGHT } * legs;
                            self.pushes.push(Push::Load { limb: id, x, weight });
                            self.sweet_spot = self.charge.is_some() && -limbs[id].surface_velocity(x) > SWEET_SPOT_SPEED;
                        }
                    }
                    return None;
                }
                None => {
                    // Walked off the tip, back past the base through the trunk, or it snapped
                    if let Support::Limb(id) = support {
                        if limbs[id].gave_way() {
                            self.fall_with(id);
                        } else if let Some((next, s)) = self.step_across(limbs, id) {
                            // Walking down a fork onto its parent, or onto a branch just
                            // alongside: step across instead of dropping through it
                            self.standing_on = Some(next);
                            self.y = s;
                            return None;
                        }
                    }
                    self.standing_on = None;
                    if !self.falling {
                        self.falling = true;
                        self.velocity = 0.0;
                        if !self.panicking {
                            self.coyote = COYOTE_STEPS;
                        }
                    }
                }
            }
        }

        // Branches are one-way: Kitty leaps up through them and only lands coming down
        if self.velocity < 0.0 {
            return None;
        }
        let feet = self.feet();
        let mut best: Option<(Support, f32)> = None;
        let candidates = std::iter::once(Support::Ground).chain((0..limbs.len()).map(Support::Limb));
        for support in candidates {
            if let Some(s) = self.support_surface(support, limbs, &feet) {
                // The ground is solid: nothing is below it, so it catches Kitty however she
                // got under its surface (e.g. letting go of a limb sagging into the grass)
                let from_above = support == Support::Ground || self.prev_y <= s + LANDING_TOLERANCE;
                if from_above && self.y >= s && !self.passes_through(limbs, support, s) && best.is_none_or(|(_, b)| s < b) {
                    best = Some((support, s));
                }
            }
        }
        let Some((support, s)) = best else {
            self.try_grab(limbs);
            return None;
        };
        if let Support::Limb(id) = support {
            self.impact(id, &limbs[id]);
        }
        self.land(support, s);
        if let Some((charge, _)) = self.buffered.take() {
            self.leap(charge, limbs);
        }
        Some(support)
    }

    /// Landing on a limb. Braced (still holding the jump) her whole impact goes into it, so it
    /// dips and throws her back up; relaxed, her legs soak up most of it. Either way a hard
    /// landing jolts the wood, less where it gives. Bouncing (landing braced back on the limb she
    /// leapt from) drives the whole jolt into it too and leaves some strain lingering; any other
    /// landing, braced or not, jolts it no harder than a relaxed one, so a long fall onto a
    /// limb while holding the jump doesn't snap it.
    fn impact(&mut self, id: LimbId, limb: &Limb) {
        let x = self.paw_on(limb).unwrap_or(self.x);
        let braced = self.charge.is_some();
        let bounce = braced && self.leapt_from == Some(id);
        let share = |full: bool| if full { 1.0 } else { RELAXED_LANDING };
        let speed = self.velocity.max(0.0);
        self.pushes.push(Push::Kick { limb: id, x, speed: speed * LANDING_KICK * share(braced) });
        let force = KITTY_WEIGHT * IMPACT_LOAD * (speed / TERMINAL_SPEED).powi(2) * (1.0 - IMPACT_SOFTEN * limb.give_at(x)) * share(bounce);
        let fatigue = if bounce { force * FATIGUE_SHARE } else { 0.0 };
        self.pushes.push(Push::Jolt { limb: id, x, force, fatigue });
    }

    /// Standing at a tip that bends down and draws in under her, keep her inner paw on it, unless
    /// she's heading off the end anyway.
    fn hold_tip(&mut self, limb: &Limb) {
        let dir = limb.direction;
        if limb.gave_way() || self.move_x * dir > 0.0 {
            return;
        }
        let edge = limb.end().x - dir * TIP_GRIP;
        if (self.x - dir * KITTY_FOOT_REACH - edge) * dir > 0.0 {
            self.x = edge + dir * KITTY_FOOT_REACH;
        }
    }

    fn land(&mut self, support: Support, s: f32) {
        self.y = s;
        self.falling = false;
        self.velocity = 0.0;
        self.standing_on = Some(support);
        self.just_landed = true;
        self.on_feet = 0;
        self.panicking = false;
        self.dropping = false;
        self.leapt = false;
        self.leapt_from = None;
        self.coyote = 0;
        self.drop_through = None;
    }

    /// Another limb under Kitty's paws within a small step up or down of where she is, for
    /// when she walks off the end of the one she was on.
    fn step_across(&self, limbs: &[Limb], from: LimbId) -> Option<(Support, f32)> {
        let feet = self.feet();
        (0..limbs.len())
            .filter(|&id| id != from && !limbs[id].gone())
            .filter_map(|id| limbs[id].surface_under(&feet).map(|s| (Support::Limb(id), s)))
            .filter(|&(_, s)| s >= self.y - STEP_ACROSS_UP && s <= self.y + LANDING_TOLERANCE)
            .min_by(|a, b| (a.1 - self.y).abs().total_cmp(&(b.1 - self.y).abs()))
    }

    /// Falling just short of a branch: catch it if the raised paws cross its top. Holding the
    /// jump as she catches it counts as pressing to climb.
    fn try_grab(&mut self, limbs: &[Limb]) {
        if self.velocity < 0.0 {
            return;
        }
        let prev_top = self.prev_y - KITTY_BOX.1;
        for (id, limb) in limbs.iter().enumerate() {
            if limb.gone() || Some(id) == self.released_from {
                continue;
            }
            let Some(s) = limb.surface_y(self.x) else { continue };
            // Too low to hang from (her feet would be in the ground), or part of what she's
            // dropping through: also the fork or parent it joins, so she doesn't catch the
            // wood she just left
            let through = self.drop_through.as_ref().is_some_and(|drop| drop.skip.contains(&id) || joined(limbs, id, drop.limb));
            if s - GRIP_DEPTH + KITTY_BOX.1 >= GROUND_Y || through {
                continue;
            }
            if prev_top + GRAB_REACH <= s && s <= self.top() + GRAB_REACH {
                self.hanging_on = Some(id);
                self.falling = false;
                self.velocity = 0.0;
                self.panicking = false;
                self.dropping = false;
                self.leapt = false;
                self.leapt_from = None;
                self.just_grabbed = true;
                self.buffered = None;
                if self.charge.take().is_some() {
                    self.climb_wait = Some(0);
                    self.hold_carry = true;
                }
                self.y = s - GRIP_DEPTH + KITTY_BOX.1;
                self.pushes.push(Push::Kick { limb: id, x: self.x, speed: GRAB_KICK });
                return;
            }
        }
    }

    /// Haul up from hanging to standing, easing the box up past the branch.
    fn climb_step(&mut self, limbs: &[Limb], id: LimbId) -> Option<Support> {
        // Keep to the wood as a bending tip draws in
        if !limbs[id].fell_away(self.x) {
            let (lo, hi) = limbs[id].walk_range();
            self.x = self.x.clamp(lo, hi);
        }
        let Some(s) = limbs[id].surface_y(self.x) else {
            self.let_go(limbs[id].gave_way());
            return None;
        };
        self.climb_step += 1;
        let progress = (self.climb_step as f32 / CLIMB_STEPS as f32).min(1.0);
        let eased = 1.0 - (1.0 - progress).powi(2);
        let hang_bottom = s - GRIP_DEPTH + KITTY_BOX.1;
        self.y = hang_bottom + (s - hang_bottom) * eased;
        self.pushes.push(Push::Load { limb: id, x: self.x, weight: KITTY_WEIGHT });
        if progress < 1.0 {
            return None;
        }
        self.climbing_on = None;
        self.standing_on = Some(Support::Limb(id));
        self.just_climbed = true;
        Some(Support::Limb(id))
    }

    pub fn climb_up(&mut self) {
        if let Some(id) = self.hanging_on.take() {
            self.climbing_on = Some(id);
            self.climb_step = 0;
        }
        self.climb_wait = None;
    }

    /// Let go of the limb she's hanging from or climbing; `panic` if it gave way.
    pub fn let_go(&mut self, panic: bool) {
        let Some(id) = self.hanging_on.or(self.climbing_on) else { return };
        self.released_from = Some(id);
        self.released_steps = REGRAB_STEPS;
        self.hanging_on = None;
        self.climbing_on = None;
        self.climb_wait = None;
        self.falling = true;
        self.velocity = 0.0;
        self.panicking = panic;
        if panic {
            self.fall_with(id);
        }
    }

    /// The limb she was on or holding gave way: she goes down in a panic with the piece that fell.
    /// What's left of it (a stub, springing back up as it loses that piece and her weight) can't
    /// catch her, as if she were dropping through it, until she's well below it.
    fn fall_with(&mut self, id: LimbId) {
        self.panicking = true;
        self.drop_through = Some(DropThrough { limb: id, skip: vec![id], from_y: self.y, steps: 0 });
    }

    /// Take a hit from something at `from_x`. Returns false while still blinking from the last.
    /// A hit spoils a charge.
    pub fn hurt(&mut self, from_x: f32) -> bool {
        if self.invulnerable > 0 || self.hearts == 0 {
            return false;
        }
        self.hearts -= 1;
        self.invulnerable = INVULNERABLE_STEPS;
        self.let_go(false);
        self.forget_input();
        self.standing_on = None;
        self.falling = true;
        self.leapt = false;
        self.leapt_from = None;
        self.dropping = false;
        self.coyote = 0;
        self.velocity = KNOCKBACK_HOP;
        self.knock_vx = if self.x >= from_x { KNOCKBACK_SPEED } else { -KNOCKBACK_SPEED };
        true
    }

    /// Bounce up off an enemy Kitty just stomped.
    pub fn bounce(&mut self) {
        self.falling = true;
        self.standing_on = None;
        self.leapt = false;
        self.velocity = STOMP_BOUNCE;
    }

    /// Back on the ground at `x` with full hearts (after losing a life or starting a level).
    pub fn respawn(&mut self, x: f32) {
        *self = Kitty::new(x);
        self.invulnerable = INVULNERABLE_STEPS;
    }

    // ------------------ Animation -------------------

    fn play(&mut self, clip: &'static KittyClip) {
        if !std::ptr::eq(self.clip, clip) {
            self.clip = clip;
            self.clip_time = 0.0;
        }
    }

    fn restart(&mut self, clip: &'static KittyClip) {
        self.clip = clip;
        self.clip_time = 0.0;
    }

    fn finished(&self) -> bool {
        !self.clip.clip.looped && self.clip_time >= self.clip.clip.duration()
    }

    fn is(&self, clip: &'static KittyClip) -> bool {
        std::ptr::eq(self.clip, clip)
    }

    /// Pick the clip that matches what Kitty is doing, then advance it.
    pub fn animate(&mut self, dt: f32, rng: &mut Rng) {
        let mut rate = 1.0;
        let speed = self.move_x.abs();
        if self.climbing_on.is_some() {
            self.idle_time = 0.0;
            self.play(&CLIMB);
        } else if self.hanging_on.is_some() {
            self.idle_time = 0.0;
            if self.just_grabbed {
                self.restart(&GRAB);
            } else if !self.is(&GRAB) || self.finished() {
                self.play(&HANG);
            }
        } else if self.standing_on.is_none() {
            self.idle_time = 0.0;
            if self.dropping {
                self.play(&DROP);
            } else if self.released_from.is_some() && !self.panicking {
                self.play(&LET_GO);
            } else if self.panicking || self.velocity > PANIC_SPEED || self.invulnerable > INVULNERABLE_STEPS - 20 {
                self.play(&PANIC);
            } else if self.just_leapt {
                self.restart(&RISE);
            } else if self.leapt && self.velocity < -3.0 {
                self.play(&RISE);
            } else if self.velocity < 3.0 {
                self.play(&APEX);
            } else {
                self.play(&FALL);
            }
        } else if self.charge.is_some() {
            self.idle_time = 0.0;
            self.play(&CHARGE);
        } else if speed > 0.0 {
            self.idle_time = 0.0;
            self.running = if self.running { speed > WALK_BELOW } else { speed > RUN_ABOVE };
            let clip = if !self.running {
                &WALK
            } else if self.is(&RUN) || (self.is(&RUN_START) && self.finished()) {
                &RUN
            } else {
                &RUN_START
            };
            self.play(clip);
            // Paws keep pace with the ground
            rate = if self.running { speed.max(RUN_RATE_MIN) } else { (speed / WALK_CYCLE_SPEED).clamp(WALK_RATE.0, WALK_RATE.1) };
        } else {
            self.running = false;
            if self.just_landed {
                self.restart(&LAND);
            } else if self.just_climbed {
                self.restart(&CLIMB_DONE);
            }
            let busy = (self.is(&LAND) || self.is(&CLIMB_DONE) || self.is(&GROOM) || self.is(&WAVE)) && !self.finished();
            if busy {
                // let the squash or the fidget play out
            } else if self.is(&SLEEP) || self.idle_time >= SLEEP_AFTER {
                self.play(&SLEEP);
            } else if self.idle_time >= self.next_fidget {
                let fidget = if rng.chance(0.5) { &GROOM } else { &WAVE };
                self.play(fidget);
                self.next_fidget = self.idle_time + fidget.clip.duration() + rng.pair(FIDGET_DELAY);
            } else {
                self.play(&SIT);
            }
            self.idle_time += dt;
        }
        if self.idle_time == 0.0 {
            self.next_fidget = rng.pair(FIDGET_DELAY);
        }
        self.full_time = if self.charge_level() == Some(1.0) { self.full_time + dt } else { 0.0 };
        self.just_landed = false;
        self.just_grabbed = false;
        self.just_climbed = false;
        self.just_leapt = false;
        self.clip_time += dt * rate;
    }

    /// The frame to show now. Crouching, the frame follows the charge, then wiggles at full.
    fn frame(&self) -> &'static str {
        if self.is(&CHARGE) {
            let t = self.charge_level().unwrap_or(1.0);
            if t < 1.0 {
                return CROUCH.iter().find(|&&(upto, _)| t < upto).map_or(WIGGLE[0], |&(_, frame)| frame);
            }
            return WIGGLE[(self.full_time * WIGGLE_FPS) as usize % WIGGLE.len()];
        }
        let clip = &self.clip.clip;
        let i = (self.clip_time * clip.fps) as usize;
        let n = clip.frames.len();
        clip.frames[if clip.looped { i % n } else { i.min(n - 1) }]
    }

    /// `offset` shifts the drawing only (World uses it to blend between physics steps).
    pub fn draw(&self, assets: &Assets, offset: Vec2) {
        // Blink while invulnerable
        if self.invulnerable > 0 && (self.invulnerable / 5) % 2 == 1 {
            return;
        }
        let pin_y = match self.clip.pin {
            Pin::Grip => self.top(),
            Pin::Feet => self.y,
        };
        let at = vec2(self.x, pin_y) + offset;
        if self.is(&CHARGE) {
            // Squashing down into the crouch, then a wiggle and sway, ready to pounce
            let t = self.charge_level().unwrap_or(1.0);
            let (scale, sway) = if t < 1.0 {
                (vec2(1.0 + CROUCH_SQUASH * 0.6 * t, 1.0 - CROUCH_SQUASH * t), 0.0)
            } else {
                let beat = (self.full_time * WIGGLE_FPS * PI).sin();
                let squash = CROUCH_SQUASH + WIGGLE_SQUASH * beat.abs();
                (vec2(1.0 + squash * 0.6, 1.0 - squash), WIGGLE_SWAY * beat * self.facing)
            };
            assets.kitty.draw_stretched(self.frame(), at, self.facing, scale, sway, WHITE);
        } else {
            assets.kitty.draw(self.frame(), at, self.facing, WHITE);
        }
    }

    /// The charge ring round her feet, in two halves so it wraps around her: the far half is
    /// drawn before her and the near half after. It fills from the front as the charge builds,
    /// sparkles at full, and glows while a release would ride a limb springing up.
    pub fn draw_ring(&self, offset: Vec2, time: f32, near: bool) {
        let Some(t) = self.charge_level() else { return };
        if self.hanging_on.is_some() || self.climbing_on.is_some() {
            return;
        }
        let centre = vec2(self.x - RING_BACK * self.facing, self.y + 2.0) + offset;
        let (rx, ry) = RING_RADII;
        let point = |angle: f32| centre + vec2(rx * angle.cos(), ry * angle.sin());
        // Angles with a positive sine are the near (lower) half
        let in_half = |angle: f32| (angle.sin() >= 0.0) == near;
        let full = t >= 1.0;
        let pulse = 0.5 + 0.5 * (time * 9.0).sin();
        let fill = if self.sweet_spot {
            RING_SWEET
        } else if full {
            Color { a: 0.75 + 0.25 * pulse, ..RING_FULL }
        } else {
            RING_FILL
        };
        for k in 0..RING_PIECES {
            let (a, b) = (k as f32 / RING_PIECES as f32 * TAU, (k + 1) as f32 / RING_PIECES as f32 * TAU);
            let mid = (a + b) / 2.0;
            if !in_half(mid) {
                continue;
            }
            // How far round from the front this piece is, 0..1 either way
            let round = ((mid - FRAC_PI_2 + PI).rem_euclid(TAU) - PI).abs() / PI;
            let (p, q) = (point(a), point(b));
            if self.sweet_spot {
                draw_line(p.x, p.y, q.x, q.y, RING_WIDTH * 2.6, Color { a: 0.35, ..RING_SWEET });
            }
            let color = if round <= t { fill } else { RING_TRACK };
            draw_line(p.x, p.y, q.x, q.y, RING_WIDTH, color);
        }
        if full {
            // Sparkles chasing round the ring
            for k in 0..4 {
                let angle = time * 1.6 + k as f32 * FRAC_PI_2;
                if !in_half(angle.rem_euclid(TAU)) {
                    continue;
                }
                let at = point(angle);
                let size = 3.0 + 3.0 * (0.5 + 0.5 * (time * 7.0 + k as f32 * 1.9).sin());
                let color = Color::new(1.0, 1.0, 0.85, 0.95);
                draw_line(at.x - size, at.y, at.x + size, at.y, 2.0, color);
                draw_line(at.x, at.y - size, at.x, at.y + size, 2.0, color);
            }
        }
    }
}
