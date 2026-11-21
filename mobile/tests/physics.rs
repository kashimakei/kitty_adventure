//! Kitty against real generated limbs, stepped the way World does it (without the graphics).

use kitty_mobile::consts::*;
use kitty_mobile::kitty::{Kitty, Push, Support};
use kitty_mobile::level;
use kitty_mobile::limb::Limb;
use kitty_mobile::rng::Rng;
use kitty_mobile::world::{breeze_at, build_limbs, load_limb, step_limbs};

struct Sim {
    limbs: Vec<Limb>,
    kitty: Kitty,
    rng: Rng,
    time: f32,
    wind: f32,
}

impl Sim {
    fn new(level: usize, seed: u64) -> Self {
        let plan = level::generate(level, 0, seed);
        let mut rng = Rng::new(seed ^ 0xC0FFEE);
        let limbs = build_limbs(&plan, &mut rng);
        Self { limbs, kitty: Kitty::new(WORLD_W / 2.0), rng, time: 0.0, wind: level::difficulty(level, 0).wind }
    }

    fn step(&mut self, left: bool, right: bool, jump: bool, down: bool) {
        self.time += STEP;
        let breeze = breeze_at(self.time, self.wind);
        if jump {
            self.kitty.do_jump(&self.limbs);
        } else {
            self.kitty.stop_jump();
        }
        if down {
            self.kitty.let_go(false);
        }
        self.kitty.step(left, right, breeze);
        self.kitty.resolve(&self.limbs);
        for push in std::mem::take(&mut self.kitty.pushes) {
            match push {
                Push::Load { limb, x, weight } => load_limb(&mut self.limbs, limb, x, weight),
                Push::Kick { limb, x, speed } => self.limbs[limb].kick(x, speed),
            }
        }
        step_limbs(&mut self.limbs, breeze, self.time, &mut self.rng);
    }

    fn stand_on(&mut self, limb: usize, x: f32) -> bool {
        let Some(s) = self.limbs[limb].surface_y(x) else { return false };
        self.kitty.x = x;
        self.kitty.y = s;
        self.kitty.standing_on = Some(Support::Limb(limb));
        true
    }
}

/// Catching a low limb, hanging while it sags, then letting go: the ground always catches her.
#[test]
fn letting_go_of_a_low_limb_lands_on_the_ground() {
    let mut grabs = 0;
    for seed in 0..15 {
        let low: Vec<usize> = Sim::new(1, seed).limbs.iter().enumerate()
            .filter(|(_, l)| l.base().y > -200.0).map(|(i, _)| i).collect();
        for &id in &low {
            for k in 0..6 {
                let mut sim = Sim::new(1, seed);
                let (lo, hi) = sim.limbs[id].walk_range();
                sim.kitty = Kitty::new(lo + (hi - lo) * (k as f32 + 0.5) / 6.0);
                sim.step(false, false, true, false);
                let hung = (0..120).any(|_| {
                    sim.step(false, false, false, false);
                    sim.kitty.hanging_on.is_some()
                });
                if !hung {
                    continue;
                }
                grabs += 1;
                for _ in 0..60 {
                    sim.step(false, false, false, false);
                }
                sim.step(false, false, false, true);
                for _ in 0..240 {
                    sim.step(false, false, false, false);
                }
                // On the ground, or caught another (or the same) low limb on the way down
                let supported = sim.kitty.standing_on.is_some() || sim.kitty.hanging_on.is_some();
                assert!(sim.kitty.y <= GROUND_Y + 0.5 && supported,
                        "seed {seed} limb {id}: y {} standing {:?} hanging {:?} after letting go",
                        sim.kitty.y, sim.kitty.standing_on, sim.kitty.hanging_on);
            }
        }
    }
    assert!(grabs > 20, "only {grabs} grabs: the test isn't exercising hanging");
}

/// Walking down a fork toward the trunk steps onto its parent instead of dropping through.
#[test]
fn walking_down_a_fork_steps_onto_its_parent() {
    let mut walked = 0;
    for level in 1..=5 {
        for seed in 0..8 {
            let forks: Vec<usize> = Sim::new(level, seed).limbs.iter().enumerate()
                .filter(|(_, l)| l.attachment.is_some()).map(|(i, _)| i).collect();
            for &fork in &forks {
                let mut sim = Sim::new(level, seed);
                let parent = sim.limbs[fork].attachment.unwrap().parent;
                let (lo, hi) = sim.limbs[fork].walk_range();
                let toward_trunk_left = sim.limbs[fork].direction > 0.0;
                let start = if toward_trunk_left { lo + (hi - lo) * 0.7 } else { hi - (hi - lo) * 0.7 };
                if !sim.stand_on(fork, start) {
                    continue;
                }
                for _ in 0..200 {
                    let was_on_fork = sim.kitty.standing_on == Some(Support::Limb(fork));
                    sim.step(toward_trunk_left, !toward_trunk_left, false, false);
                    if was_on_fork && sim.kitty.standing_on != Some(Support::Limb(fork)) {
                        walked += 1;
                        let feet = [sim.kitty.x - KITTY_FOOT_REACH, sim.kitty.x, sim.kitty.x + KITTY_FOOT_REACH];
                        if let Some(s) = sim.limbs[parent].surface_under(&feet) {
                            if (s - sim.kitty.y).abs() < 25.0 {
                                assert!(sim.kitty.standing_on.is_some(),
                                        "level {level} seed {seed} fork {fork}: dropped through its parent");
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    assert!(walked > 50, "only {walked} walk-offs: the test isn't exercising forks");
}
