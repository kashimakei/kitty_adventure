//! Eagles come one at a time, at most five a level, with a breather after each one has gone.

use kitty_mobile::consts::*;
use kitty_mobile::enemies::{Enemy, EnemyCtx, Kind, KittyView, Projectile, Spawner};
use kitty_mobile::kitty::Kitty;
use kitty_mobile::level;
use kitty_mobile::rng::Rng;
use macroquad::prelude::*;

/// A view high in the trees, with Kitty standing about in the middle of it.
const VIEW: Rect = Rect { x: 600.0, y: -2400.0, w: VIEW_W, h: VIEW_H };

#[test]
fn eagles_come_one_at_a_time_and_at_most_five_a_level() {
    // The story's eagle level, and laps after it that used to send two at a time, faster
    for (level, lap) in [(5, 0), (1, 1), (5, 1), (5, 3)] {
        let d = level::difficulty(level, lap);
        for seed in 0..5 {
            let mut rng = Rng::new(seed);
            let mut spawner = Spawner::new(&d, &mut rng);
            let mut kitty = Kitty::new(VIEW.center().x);
            kitty.y = VIEW.y + 800.0;
            let view = KittyView { x: kitty.x, y: kitty.y, rect: kitty.rect(), body: kitty.body(), standing_on: None,
                                   hanging_on: None, airborne: false };
            let mut enemies: Vec<Box<dyn Enemy>> = Vec::new();
            let (mut spawned, mut gone_at) = (0, None);
            // Ten minutes, far longer than a level takes
            for step in 0..60 * 600 {
                let before = enemies.len();
                spawner.step(STEP, &mut enemies, &view, VIEW, &d, &mut rng);
                // Only the eagles matter here
                enemies.retain(|e| e.kind() == Kind::Eagle);
                assert!(enemies.len() <= 1, "level {level} lap {lap} seed {seed}: two eagles at once at step {step}");
                if enemies.len() > before {
                    spawned += 1;
                    if let Some(gone) = gone_at {
                        let wait = (step - gone) as f32 * STEP;
                        assert!(wait >= d.eagle_interval.0 - STEP,
                                "level {level} lap {lap} seed {seed}: the next eagle came {wait:.2} s after the last went");
                    }
                }
                let (mut pushes, mut projectiles, mut sfx): (Vec<_>, Vec<Projectile>, Vec<_>) = (Vec::new(), Vec::new(), Vec::new());
                let mut ctx = EnemyCtx { kitty: view, limbs: &[], pushes: &mut pushes, projectiles: &mut projectiles,
                                         view: VIEW, rng: &mut rng, difficulty: &d, sfx: &mut sfx };
                for eagle in &mut enemies {
                    eagle.step(&mut ctx);
                }
                let had = !enemies.is_empty();
                enemies.retain(|e| e.alive());
                if had && enemies.is_empty() {
                    gone_at = Some(step);
                }
            }
            assert_eq!(spawned, d.eagles_per_level, "level {level} lap {lap} seed {seed}");
            assert_eq!(d.eagles_per_level, 5);
        }
    }
}
