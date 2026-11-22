//! Kitty can bop crows and eagles in the air as well as perched, whatever they're doing, and a
//! bopped one can't be bopped again and tumbles out of the sky.

use kitty_mobile::consts::*;
use kitty_mobile::enemies::crow::Crow;
use kitty_mobile::enemies::eagle::Eagle;
use kitty_mobile::enemies::{Enemy, EnemyCtx, Kind, KittyView, Projectile, Sfx};
use kitty_mobile::kitty::{Kitty, Support};
use kitty_mobile::level;
use kitty_mobile::limb::Limb;
use kitty_mobile::rng::Rng;
use kitty_mobile::world::{build_limbs, stomp};
use macroquad::prelude::*;

/// A view high in the trees, out of the way of the ground.
const VIEW: Rect = Rect { x: 600.0, y: -2400.0, w: VIEW_W, h: VIEW_H };

/// Where the birds fly: open sky, or a stretch of a level's trees with limb tips to perch on.
struct Sky {
    limbs: Vec<Limb>,
    view: Rect,
    level: usize,
}

impl Sky {
    fn open(level: usize) -> Self {
        Self { limbs: Vec::new(), view: VIEW, level }
    }

    /// A level-4 tree, viewed with a sturdy limb's tip in the middle.
    fn tree(seed: u64) -> Option<Self> {
        let plan = level::generate(&level::endless(4, 0), seed);
        let mut rng = Rng::new(seed);
        let limbs = build_limbs(&plan, &mut rng);
        let tip = limbs.iter()
            .filter(|l| !l.thin && !l.is_goal && l.base().y < -700.0 && l.base().y > -1600.0)
            .map(|l| l.tip())
            .next()?;
        let view = Rect::new(tip.x - VIEW_W / 2.0, tip.y - VIEW_H / 2.0, VIEW_W, VIEW_H);
        Some(Self { limbs, view, level: 4 })
    }

    fn crow(&self, from_left: bool, y: f32) -> Box<dyn Enemy> {
        Box::new(Crow::new(from_left, y, self.view, 1.0))
    }

    fn eagle(&self, from_left: bool, y: f32, kitty: &Kitty) -> Box<dyn Enemy> {
        Box::new(Eagle::new(from_left, y, self.view, &kitty_view(kitty), 1.0))
    }

    /// One step of `enemy` with Kitty where she is, and a frame of its animation as the game draws
    /// one per step at 60 Hz; the sounds it made.
    fn step(&self, enemy: &mut dyn Enemy, kitty: &Kitty, rng: &mut Rng) -> Vec<Sfx> {
        let difficulty = level::difficulty(self.level, 0);
        let (mut pushes, mut projectiles, mut sfx): (Vec<_>, Vec<Projectile>, Vec<_>) = (Vec::new(), Vec::new(), Vec::new());
        let mut ctx = EnemyCtx {
            kitty: kitty_view(kitty),
            limbs: &self.limbs,
            pushes: &mut pushes,
            projectiles: &mut projectiles,
            view: self.view,
            rng,
            difficulty: &difficulty,
            sfx: &mut sfx,
        };
        enemy.step(&mut ctx);
        enemy.animate(STEP);
        sfx
    }
}

/// Kitty in the air with her feet at `feet_y`, coming down at `velocity`.
fn kitty_at(x: f32, feet_y: f32, velocity: f32) -> Kitty {
    let mut kitty = Kitty::new(x);
    kitty.y = feet_y;
    kitty.standing_on = None;
    kitty.falling = true;
    kitty.velocity = velocity;
    kitty
}

fn kitty_view(kitty: &Kitty) -> KittyView {
    KittyView { x: kitty.x, y: kitty.y, rect: kitty.rect(), body: kitty.body(), standing_on: None, hanging_on: None,
                airborne: true }
}

/// What a bird did on its way across.
struct Flight {
    /// How far below where it started it got.
    dropped: f32,
    screeched: bool,
    /// About the step it settled on a perch (it then stays put for a second at least).
    perched_at: Option<usize>,
    /// The tallest its stomp box got.
    tallest: f32,
}

/// Fly `enemy` until it leaves (or `steps` run out), checking it can be bopped on every step.
fn fly_across(sky: &Sky, mut enemy: Box<dyn Enemy>, kitty: &Kitty, seed: u64, steps: usize) -> Flight {
    let mut rng = Rng::new(seed);
    let start = enemy.position();
    let mut flight = Flight { dropped: 0.0, screeched: false, perched_at: None, tallest: 0.0 };
    let (mut last, mut still) = (start, 0);
    for i in 0..steps {
        flight.screeched |= sky.step(enemy.as_mut(), kitty, &mut rng).contains(&Sfx::Screech);
        if !enemy.alive() {
            return flight;
        }
        let at = enemy.position();
        flight.dropped = flight.dropped.max(at.y - start.y);
        still = if at.distance(last) < 0.01 { still + 1 } else { 0 };
        if still == 60 && flight.perched_at.is_none() {
            flight.perched_at = Some(i + 1 - still);
        }
        last = at;
        let Some(back) = enemy.stomp_box() else {
            panic!("seed {seed}: a {:?} at step {i} can't be bopped", enemy.kind());
        };
        flight.tallest = flight.tallest.max(back.h);
    }
    panic!("seed {seed}: a {:?} was still about after {steps} steps", enemy.kind());
}

/// Bop `enemy` after `after` steps, then check it can't be bopped again and falls out of the view.
fn bop_and_watch(sky: &Sky, mut enemy: Box<dyn Enemy>, kitty: &Kitty, seed: u64, after: usize) {
    let mut rng = Rng::new(seed);
    for _ in 0..after {
        sky.step(enemy.as_mut(), kitty, &mut rng);
    }
    assert!(enemy.alive() && enemy.stomp_box().is_some(), "a {:?} gone before step {after}", enemy.kind());
    enemy.stomped();
    let bopped_at = enemy.position();
    for i in 0..900 {
        assert!(enemy.stomp_box().is_none(), "a {:?} bopped at step {after} could be bopped again {i} steps later",
                enemy.kind());
        sky.step(enemy.as_mut(), kitty, &mut rng);
        if !enemy.alive() {
            let dropped = enemy.position().y - bopped_at.y;
            assert!(dropped > 200.0, "a {:?} bopped at step {after} left having dropped only {dropped:.0} px",
                    enemy.kind());
            return;
        }
    }
    panic!("a {:?} bopped at step {after} never fell out of sight", enemy.kind());
}

#[test]
fn a_crow_can_be_bopped_all_the_way_across() {
    let sky = Sky::open(4);
    let y = VIEW.y + 300.0;
    let mut dives = 0;
    for seed in 0..20 {
        for from_left in [true, false] {
            // With Kitty above it the crow only cruises; below it, it tells, dives and climbs back
            let above = kitty_at(VIEW.center().x, y - 200.0, 0.0);
            fly_across(&sky, sky.crow(from_left, y), &above, seed, 1200);
            let below = kitty_at(VIEW.center().x, y + 400.0, 0.0);
            if fly_across(&sky, sky.crow(from_left, y), &below, seed, 1200).dropped > 100.0 {
                dives += 1;
            }
        }
    }
    assert!(dives >= 10, "only {dives} of 40 crows dived, so the dive went largely untested");
}

#[test]
fn a_bopped_crow_tumbles_out_of_the_sky() {
    let sky = Sky::open(4);
    let y = VIEW.y + 300.0;
    let below = kitty_at(VIEW.center().x, y + 400.0, 0.0);
    for after in [20, 60, 100, 140, 180, 220] {
        bop_and_watch(&sky, sky.crow(true, y), &below, after as u64, after);
    }
}

#[test]
fn a_crow_can_be_bopped_gliding_in_to_a_perch_and_on_it() {
    let mut perches = 0;
    for seed in 0..30 {
        let Some(sky) = Sky::tree(seed) else { continue };
        // Flying in level with the limb tips, with Kitty well above so it never dives
        let y = sky.view.y + 350.0;
        let kitty = kitty_at(sky.view.center().x, sky.view.y + 120.0, 0.0);
        for from_left in [true, false] {
            let Some(at) = fly_across(&sky, sky.crow(from_left, y), &kitty, seed, 2400).perched_at else { continue };
            perches += 1;
            // The glide in lasts a second at least
            bop_and_watch(&sky, sky.crow(from_left, y), &kitty, seed, at - 20);
            bop_and_watch(&sky, sky.crow(from_left, y), &kitty, seed, at + 30);
        }
    }
    assert!(perches >= 8, "only {perches} crows perched, so perching went largely untested");
}

#[test]
fn an_eagle_can_be_bopped_whatever_it_is_doing() {
    let sky = Sky::open(5);
    let kitty = kitty_at(VIEW.center().x, VIEW.y + 700.0, 0.0);
    let (mut swoops, mut plunges) = (0, 0);
    for (seed, height) in [150.0, 250.0, 350.0, 450.0].into_iter().enumerate() {
        for from_left in [true, false] {
            // It cruises in, screeches and winds up over Kitty, swoops, pulls up and flies off
            let flight = fly_across(&sky, sky.eagle(from_left, VIEW.y + height, &kitty), &kitty, seed as u64, 1200);
            swoops += flight.screeched as usize;
            // Swooping steeply, head down, its back and tail stand higher
            plunges += (flight.tallest > 60.0) as usize;
        }
    }
    assert!(swoops >= 6, "only {swoops} of 8 eagles swooped, so the swoop went largely untested");
    assert!(plunges >= 2, "only {plunges} of 8 eagles plunged, so the plunge went largely untested");
}

#[test]
fn a_bopped_eagle_tumbles_out_of_the_sky() {
    let sky = Sky::open(5);
    let kitty = kitty_at(VIEW.center().x, VIEW.y + 700.0, 0.0);
    // From cruising in, through the wind-up and swoop, to pulling away
    for after in (10..=150).step_by(10) {
        bop_and_watch(&sky, sky.eagle(true, VIEW.y + 250.0, &kitty), &kitty, after as u64, after);
    }
}

#[test]
fn kitty_coming_down_on_a_flying_crow_or_eagle_bops_it() {
    let sky = Sky::open(5);
    let kitty = kitty_at(VIEW.center().x, VIEW.y + 700.0, 0.0);
    for eagle in [false, true] {
        // Fly it in a little way, then put Kitty's paws on its back
        let mut rng = Rng::new(7);
        let mut enemies = vec![if eagle { sky.eagle(true, VIEW.y + 300.0, &kitty) } else { sky.crow(true, VIEW.y + 300.0) }];
        for _ in 0..30 {
            sky.step(enemies[0].as_mut(), &kitty, &mut rng);
        }
        let back = enemies[0].stomp_box().expect("can be bopped in flight");
        let (x, feet) = (back.center().x, back.top() + 6.0);
        let kind = enemies[0].kind();

        assert_eq!(stomp(&kitty_at(x, back.top() - 4.0, 5.0), &mut enemies), None, "{kind:?}: bopped from above it");
        assert_eq!(stomp(&kitty_at(x, feet, -5.0), &mut enemies), None, "{kind:?}: bopped while she was rising");
        let mut standing = kitty_at(x, feet, 5.0);
        standing.standing_on = Some(Support::Ground);
        assert_eq!(stomp(&standing, &mut enemies), None, "{kind:?}: bopped while she was standing");
        assert!(enemies[0].stomp_box().is_some());

        assert_eq!(stomp(&kitty_at(x, feet, 5.0), &mut enemies), Some(0), "{kind:?}: not bopped coming down on it");
        assert!(enemies[0].stomp_box().is_none(), "{kind:?}: could be bopped twice");
        assert_eq!(stomp(&kitty_at(x, feet, 5.0), &mut enemies), None);
    }
}

#[test]
fn falling_at_full_speed_never_drops_through_a_back() {
    let sky = Sky::open(5);
    let kitty = kitty_at(VIEW.center().x, VIEW.y + 700.0, 0.0);
    let fresh = |kind| if kind == Kind::Crow { sky.crow(true, VIEW.y + 300.0) } else { sky.eagle(true, VIEW.y + 300.0, &kitty) };
    for kind in [Kind::Crow, Kind::Eagle] {
        let mut enemies = vec![fresh(kind)];
        let back = enemies[0].stomp_box().unwrap();
        // Every way her steps can fall around the back, at the fastest she ever falls
        for phase in 0..TERMINAL_SPEED as usize {
            let mut feet = back.top() - 2.0 * TERMINAL_SPEED + phase as f32;
            let mut bopped = false;
            while feet < back.bottom() + TERMINAL_SPEED && !bopped {
                bopped = stomp(&kitty_at(back.center().x, feet, TERMINAL_SPEED), &mut enemies).is_some();
                feet += TERMINAL_SPEED;
            }
            assert!(bopped, "{kind:?}: falling from {phase} px past a step dropped through its back");
            enemies[0] = fresh(kind);
        }
    }
}
