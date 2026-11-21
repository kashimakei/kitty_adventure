//! Kitty, ported from the pygame game's kitty.py. Movement is per 60 Hz step exactly as before,
//! plus hearts with knockback, springboard jumps off rebounding limbs, and stomp bounces.

use macroquad::prelude::*;

use crate::consts::*;
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
pub enum Push {
    Load { limb: LimbId, x: f32, weight: f32 },
    Kick { limb: LimbId, x: f32, speed: f32 },
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
// Crouch, push off, tuck: the crouch flashes by so the jump still leaves instantly
static RISE: KittyClip = kclip!(&["jump_prep_0", "jump_prep_1", "mid_jump_0"], 14.0, false);
static APEX: KittyClip = kclip!(&["mid_jump_1"], 1.0, true);
static FALL: KittyClip = kclip!(&["mid_jump_2"], 1.0, true);
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

const RUN_AFTER: f32 = 0.6;
const SLEEP_AFTER: f32 = 15.0;
const FIDGET_DELAY: (f32, f32) = (3.0, 7.0);

pub struct Kitty {
    pub x: f32,
    pub y: f32, // feet
    prev_y: f32,
    pub velocity: f32,
    pub jump: bool,
    jump_steps: u32,
    pub falling: bool,
    pub facing: f32,
    walk_dir: f32,
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
    // Animation
    clip: &'static KittyClip,
    clip_time: f32,
    move_time: f32,
    idle_time: f32,
    next_fidget: f32,
    just_landed: bool,
    just_grabbed: bool,
    just_climbed: bool,
    pub jumped: bool, // this step, for sound
}

impl Kitty {
    pub fn new(x: f32) -> Self {
        Self {
            x,
            y: GROUND_Y,
            prev_y: GROUND_Y,
            velocity: 0.0,
            jump: false,
            jump_steps: 0,
            falling: false,
            facing: 1.0,
            walk_dir: 0.0,
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
            clip: &SIT,
            clip_time: 0.0,
            move_time: 0.0,
            idle_time: 0.0,
            next_fidget: 4.0,
            just_landed: false,
            just_grabbed: false,
            just_climbed: false,
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

    pub fn airborne(&self) -> bool {
        self.standing_on.is_none() && self.hanging_on.is_none() && self.climbing_on.is_none()
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

    // ------------------ Movement (per 60 Hz step) -------------------

    pub fn step(&mut self, left: bool, right: bool, breeze: f32) {
        self.prev_y = self.y;
        self.jumped = false;
        self.walk_dir = (right as i32 - left as i32) as f32;
        if self.walk_dir != 0.0 {
            self.facing = self.walk_dir;
        }
        if self.released_steps > 0 {
            self.released_steps -= 1;
            if self.released_steps == 0 {
                self.released_from = None;
            }
        }
        if self.invulnerable > 0 {
            self.invulnerable -= 1;
        }
        if self.climbing_on.is_some() {
            return;
        }
        if self.hanging_on.is_some() {
            // Shuffle paw over paw along the branch
            self.x = (self.x + self.walk_dir * HANG_SHIMMY).clamp(KITTY_BOX.0 / 2.0, WORLD_W - KITTY_BOX.0 / 2.0);
            return;
        }
        self.x += self.walk_dir * KITTY_SPEED + self.knock_vx;
        self.knock_vx *= 0.88;
        if self.knock_vx.abs() < 0.2 {
            self.knock_vx = 0.0;
        }
        // The wind pushes Kitty around while airborne
        if self.jump || self.falling {
            self.x += breeze * WIND_DRIFT;
        }
        self.x = self.x.clamp(KITTY_BOX.0 / 2.0, WORLD_W - KITTY_BOX.0 / 2.0);

        if self.jump {
            self.jump_steps += 1;
            if self.jump_steps <= KITTY_JUMP_STEPS {
                self.velocity += KITTY_JUMP_ACCEL;
            } else {
                self.falling = true;
                self.jump = false;
                self.velocity = 0.0;
            }
        }
        if self.falling {
            self.velocity += KITTY_GRAVITY;
        }
        // Walking off a support is noticed in `resolve`, which can see the limbs
        self.y += self.velocity;
    }

    fn support_surface(&self, support: Support, limbs: &[Limb], xs: &[f32]) -> Option<f32> {
        match support {
            Support::Ground => xs.iter().any(|&x| (0.0..=WORLD_W).contains(&x)).then_some(GROUND_Y),
            Support::Limb(id) => limbs[id].surface_under(xs),
        }
    }

    /// Ride the support Kitty is on, hang from or climb a limb, or catch one while falling.
    /// Returns the support Kitty just landed or climbed onto, if any.
    pub fn resolve(&mut self, limbs: &[Limb]) -> Option<Support> {
        if let Some(id) = self.climbing_on {
            return self.climb_step(limbs, id);
        }
        if let Some(id) = self.hanging_on {
            match limbs[id].surface_y(self.x) {
                None => self.let_go(limbs[id].broken()),
                Some(s) => {
                    self.y = s - GRIP_DEPTH + KITTY_BOX.1;
                    self.pushes.push(Push::Load { limb: id, x: self.x, weight: KITTY_WEIGHT });
                }
            }
            return None;
        }
        if let Some(support) = self.standing_on {
            match self.support_surface(support, limbs, &self.feet()) {
                Some(s) => {
                    self.y = s;
                    if let Support::Limb(id) = support {
                        // Press down through whichever paw is on the branch, centre first
                        let mut feet = self.feet();
                        feet.sort_by(|a, b| (a - self.x).abs().total_cmp(&(b - self.x).abs()));
                        if let Some(&x) = feet.iter().find(|&&x| limbs[id].surface_y(x).is_some()) {
                            self.pushes.push(Push::Load { limb: id, x, weight: KITTY_WEIGHT });
                        }
                    }
                    return None;
                }
                None => {
                    // Walked off the tip, back past the base through the trunk, or it snapped
                    if let Support::Limb(id) = support {
                        if limbs[id].broken() {
                            self.panicking = true;
                        }
                    }
                    self.standing_on = None;
                    if !self.falling && !self.jump {
                        self.falling = true;
                        self.velocity = 0.0;
                    }
                }
            }
        }

        // Branches are one-way: Kitty jumps up through them and only lands coming down
        if self.jump || self.velocity < 0.0 {
            return None;
        }
        let feet = self.feet();
        let mut best: Option<(Support, f32)> = None;
        let candidates = std::iter::once(Support::Ground).chain((0..limbs.len()).map(Support::Limb));
        for support in candidates {
            if let Some(s) = self.support_surface(support, limbs, &feet) {
                if self.prev_y <= s + LANDING_TOLERANCE && self.y >= s && best.map_or(true, |(_, b)| s < b) {
                    best = Some((support, s));
                }
            }
        }
        let Some((support, s)) = best else {
            self.try_grab(limbs);
            return None;
        };
        if let Support::Limb(id) = support {
            self.pushes.push(Push::Kick { limb: id, x: self.x, speed: self.velocity * LANDING_KICK });
        }
        self.y = s;
        self.falling = false;
        self.velocity = 0.0;
        self.standing_on = Some(support);
        self.just_landed = true;
        self.panicking = false;
        Some(support)
    }

    /// Falling just short of a branch: catch it if the raised paws cross its top.
    fn try_grab(&mut self, limbs: &[Limb]) {
        if self.jump || self.velocity < 0.0 {
            return;
        }
        let prev_top = self.prev_y - KITTY_BOX.1;
        for (id, limb) in limbs.iter().enumerate() {
            if limb.broken() || Some(id) == self.released_from {
                continue;
            }
            let Some(s) = limb.surface_y(self.x) else { continue };
            if prev_top + GRAB_REACH <= s && s <= self.top() + GRAB_REACH {
                self.hanging_on = Some(id);
                self.falling = false;
                self.velocity = 0.0;
                self.panicking = false;
                self.just_grabbed = true;
                self.y = s - GRIP_DEPTH + KITTY_BOX.1;
                self.pushes.push(Push::Kick { limb: id, x: self.x, speed: LANDING_KICK * 6.0 });
                return;
            }
        }
    }

    /// Haul up from hanging to standing, easing the box up past the branch.
    fn climb_step(&mut self, limbs: &[Limb], id: LimbId) -> Option<Support> {
        let Some(s) = limbs[id].surface_y(self.x) else {
            self.let_go(limbs[id].broken());
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

    /// Jump, or haul up when hanging. Jumping as the limb underfoot springs back up launches
    /// Kitty higher.
    pub fn do_jump(&mut self, limbs: &[Limb]) {
        if self.hanging_on.is_some() {
            self.climb_up();
            return;
        }
        if self.climbing_on.is_some() || self.jump || self.falling {
            return;
        }
        let mut boost = 0.0;
        if let Some(Support::Limb(id)) = self.standing_on {
            boost = (-limbs[id].surface_velocity(self.x) * SPRING_BOOST).clamp(0.0, SPRING_BOOST_MAX);
            self.pushes.push(Push::Kick { limb: id, x: self.x, speed: JUMP_KICK });
        }
        self.standing_on = None;
        self.jump = true;
        self.jump_steps = 0;
        self.velocity = -boost;
        self.jumped = true;
    }

    /// Letting go of jump early ends the climb, so taps make little hops.
    pub fn stop_jump(&mut self) {
        if self.jump {
            self.velocity = self.velocity.max(KITTY_GRAVITY);
            self.falling = true;
            self.jump = false;
        }
    }

    pub fn climb_up(&mut self) {
        if let Some(id) = self.hanging_on.take() {
            self.climbing_on = Some(id);
            self.climb_step = 0;
        }
    }

    pub fn let_go(&mut self, panic: bool) {
        if self.hanging_on.is_none() && self.climbing_on.is_none() {
            return;
        }
        self.released_from = self.hanging_on.or(self.climbing_on);
        self.released_steps = REGRAB_STEPS;
        self.hanging_on = None;
        self.climbing_on = None;
        self.falling = true;
        self.velocity = 0.0;
        self.panicking = panic;
    }

    /// Take a hit from something at `from_x`. Returns false while still blinking from the last.
    pub fn hurt(&mut self, from_x: f32) -> bool {
        if self.invulnerable > 0 || self.hearts == 0 {
            return false;
        }
        self.hearts -= 1;
        self.invulnerable = INVULNERABLE_STEPS;
        self.let_go(false);
        self.standing_on = None;
        self.jump = false;
        self.falling = true;
        self.velocity = KNOCKBACK_HOP;
        self.knock_vx = if self.x >= from_x { KNOCKBACK_SPEED } else { -KNOCKBACK_SPEED };
        true
    }

    /// Bounce up off an enemy Kitty just stomped.
    pub fn bounce(&mut self) {
        self.jump = false;
        self.falling = true;
        self.standing_on = None;
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
        if self.climbing_on.is_some() {
            self.move_time = 0.0;
            self.idle_time = 0.0;
            self.play(&CLIMB);
        } else if self.hanging_on.is_some() {
            self.move_time = 0.0;
            self.idle_time = 0.0;
            if self.just_grabbed {
                self.restart(&GRAB);
            } else if !self.is(&GRAB) || self.finished() {
                self.play(&HANG);
            }
        } else if self.standing_on.is_none() {
            self.move_time = 0.0;
            self.idle_time = 0.0;
            if self.released_from.is_some() && !self.panicking {
                self.play(&LET_GO);
            } else if self.panicking || self.velocity > PANIC_SPEED || self.invulnerable > INVULNERABLE_STEPS - 20 {
                self.play(&PANIC);
            } else if self.jump {
                self.play(&RISE);
            } else if self.velocity < 3.0 {
                self.play(&APEX);
            } else {
                self.play(&FALL);
            }
        } else if self.walk_dir != 0.0 {
            self.idle_time = 0.0;
            self.move_time += dt;
            if self.move_time < RUN_AFTER {
                self.play(&WALK);
            } else if self.is(&WALK) {
                self.play(&RUN_START);
            } else if !self.is(&RUN_START) || self.finished() {
                self.play(&RUN);
            }
        } else {
            self.move_time = 0.0;
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
        self.just_landed = false;
        self.just_grabbed = false;
        self.just_climbed = false;
        self.clip_time += dt;
    }

    pub fn draw(&self, assets: &Assets) {
        // Blink while invulnerable
        if self.invulnerable > 0 && (self.invulnerable / 5) % 2 == 1 {
            return;
        }
        let clip = &self.clip.clip;
        let i = (self.clip_time * clip.fps) as usize;
        let n = clip.frames.len();
        let name = clip.frames[if clip.looped { i % n } else { i.min(n - 1) }];
        let pin_y = match self.clip.pin {
            Pin::Grip => self.top(),
            Pin::Feet => self.y,
        };
        assets.kitty.draw(name, vec2(self.x, pin_y), self.facing, WHITE);
    }
}
