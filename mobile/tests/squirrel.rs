//! Squirrels lob acorns down or across at Kitty, never up at her once she's climbed past.

use kitty_mobile::consts::*;
use kitty_mobile::enemies::squirrel::Squirrel;
use kitty_mobile::enemies::{Enemy, EnemyCtx, KittyView, Projectile};
use kitty_mobile::kitty::Kitty;
use kitty_mobile::level;
use kitty_mobile::rng::Rng;
use kitty_mobile::world::build_limbs;
use macroquad::prelude::*;

/// Acorns thrown in `steps` by a squirrel partway along `limb`, with Kitty standing still at
/// `dy` px above (negative) or below the squirrel's limb and `dx` px to one side.
fn acorns_thrown(seed: u64, dx: f32, dy: f32, steps: usize) -> Option<usize> {
    let plan = level::generate(1, 0, seed);
    let difficulty = level::difficulty(1, 0);
    let mut rng = Rng::new(seed);
    let limbs = build_limbs(&plan, &mut rng);
    // A sturdy limb well clear of the ground
    let limb = (0..limbs.len()).find(|&i| limbs[i].base().y < -400.0 && !limbs[i].thin && limbs[i].attachment.is_none())?;
    let (lo, hi) = limbs[limb].walk_range();
    let x = (lo + hi) / 2.0;
    let surface = limbs[limb].surface_y(x)?;
    let mut kitty = Kitty::new(x + dx);
    kitty.y = surface + dy;
    kitty.standing_on = None;
    let view = Rect::new(x - VIEW_W / 2.0, surface - VIEW_H / 2.0, VIEW_W, VIEW_H);
    let mut squirrel = Squirrel::new(limb, 0.5);
    let (mut pushes, mut projectiles, mut sfx): (Vec<_>, Vec<Projectile>, Vec<_>) = (Vec::new(), Vec::new(), Vec::new());
    let mut thrown = 0;
    for _ in 0..steps {
        let mut ctx = EnemyCtx {
            kitty: KittyView { x: kitty.x, y: kitty.y, rect: kitty.rect(), body: kitty.body(), standing_on: None,
                               hanging_on: None, airborne: false },
            limbs: &limbs,
            pushes: &mut pushes,
            projectiles: &mut projectiles,
            view,
            rng: &mut rng,
            difficulty: &difficulty,
            sfx: &mut sfx,
        };
        squirrel.step(&mut ctx);
        thrown += projectiles.len();
        projectiles.clear();
        pushes.clear();
    }
    Some(thrown)
}

#[test]
fn squirrels_throw_at_kitty_on_their_level_or_below() {
    let mut tried = 0;
    for seed in 0..10 {
        let Some(level) = acorns_thrown(seed, 180.0, 0.0, 900) else { continue };
        let below = acorns_thrown(seed, 180.0, 150.0, 900).unwrap();
        assert!(level > 0 && below > 0, "seed {seed}: {level} acorns at Kitty level, {below} below");
        tried += 1;
    }
    assert!(tried >= 5);
}

#[test]
fn squirrels_never_throw_up_at_kitty_above_them() {
    for seed in 0..10 {
        for dy in [-60.0, -150.0, -250.0] {
            if let Some(thrown) = acorns_thrown(seed, 180.0, dy, 900) {
                assert_eq!(thrown, 0, "seed {seed}: threw {thrown} acorns up at Kitty {} px above", -dy);
            }
        }
    }
}
