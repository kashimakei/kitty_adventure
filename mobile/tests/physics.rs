//! Kitty against real generated limbs, stepped the way World does it (through `step_kitty`,
//! without the graphics): the charge jump, branches as trampolines, breaking, forgiveness, tips,
//! dropping through, and that every level can still be climbed with plain leaps.

use kitty_mobile::consts::*;
use kitty_mobile::enemies::dog;
use kitty_mobile::input::Controls;
use kitty_mobile::items::{Item, ItemKind};
use kitty_mobile::kitty::{Kitty, Support};
use kitty_mobile::level;
use kitty_mobile::limb::{stub_segments, Attachment, Limb, LimbSpec};
use kitty_mobile::rng::Rng;
use kitty_mobile::world::{breeze_at, build_limbs, load_limb, step_kitty, step_limb, KittyStep};
use macroquad::prelude::vec2;

/// Limbs further above or below Kitty than this sit still in the tests.
const NEAR: f32 = 900.0;

#[derive(Clone)]
struct Sim {
    limbs: Vec<Limb>,
    kitty: Kitty,
    rng: Rng,
    time: f32,
    wind: f32,
    /// How many limbs (the first ones) are the level's path, bottom to top.
    path: usize,
    /// The most strained the first limb has been while whole (cracks show past 0.5).
    peak_strain: f32,
}

impl Sim {
    fn new(level: usize, seed: u64) -> Self {
        let plan = level::generate(level, 0, seed);
        let mut rng = Rng::new(seed ^ 0xC0FFEE);
        let limbs = build_limbs(&plan, &mut rng);
        let path = plan.branches.iter().take_while(|b| b.on_path).count();
        Self { limbs, kitty: Kitty::new(WORLD_W / 2.0), rng, time: 0.0, wind: level::difficulty(level, 0).wind, path, peak_strain: 0.0 }
    }

    /// Only branch `id` (not a fork), to watch Kitty and one limb with nothing else to land on.
    fn alone(&self, id: usize) -> Self {
        assert!(self.limbs[id].attachment.is_none());
        Self { limbs: vec![self.limbs[id].clone()], path: 0, ..self.clone() }
    }

    /// Only fork `id` and its parent, as limbs 1 and 0.
    fn family(&self, id: usize) -> Self {
        let attachment = self.limbs[id].attachment.unwrap();
        let mut fork = self.limbs[id].clone();
        fork.attachment = Some(Attachment { parent: 0, ..attachment });
        Self { limbs: vec![self.limbs[attachment.parent].clone(), fork], path: 0, ..self.clone() }
    }

    fn step(&mut self, controls: Controls) -> KittyStep {
        self.time += STEP;
        let breeze = breeze_at(self.time, self.wind);
        let step = step_kitty(&mut self.kitty, &mut self.limbs, &controls, breeze);
        // As `step_limbs` does, but only for limbs within reach of Kitty: the rest can't touch
        // her, and leaving them be keeps these tests quick
        for i in 0..self.limbs.len() {
            if (self.limbs[i].base().y - self.kitty.y).abs() < NEAR {
                step_limb(&mut self.limbs, i, breeze, self.time, &mut self.rng);
            }
        }
        if let Some(first) = self.limbs.first().filter(|l| !l.broken()) {
            self.peak_strain = self.peak_strain.max(first.strain());
        }
        step
    }

    fn run(&mut self, steps: usize, controls: Controls) {
        for _ in 0..steps {
            self.step(controls);
        }
    }

    /// Put Kitty on `limb` at `x` and let it settle under her weight.
    fn stand_on(&mut self, limb: usize, x: f32) -> bool {
        let Some(s) = self.limbs[limb].surface_y(x) else { return false };
        self.kitty.x = x;
        self.kitty.y = s;
        self.kitty.standing_on = Some(Support::Limb(limb));
        self.run(60, Controls::default());
        self.kitty.standing_on == Some(Support::Limb(limb))
    }

    /// Snap limb `id` as short as a snap can leave it, and let the stub settle and the piece that
    /// broke off fall away.
    fn snap_short(&mut self, id: usize) {
        let base = self.limbs[id].base().x;
        self.limbs[id].snap_at(base);
        // Off the clock, so the leaps that follow fly in the same gusts as without the snap
        for _ in 0..120 {
            for i in 0..self.limbs.len() {
                step_limb(&mut self.limbs, i, 0.0, self.time, &mut self.rng);
            }
        }
    }

    /// Hold the jump for `steps` steps, standing still, then let go.
    fn charge_and_leap(&mut self, steps: u32) {
        self.step(press());
        self.run(steps.saturating_sub(1) as usize, hold());
        self.step(release());
    }

    /// Fly until she lands or catches something (or `limit` steps), steering toward `aim` if
    /// given; returns the highest point she reached.
    fn fly(&mut self, aim: Option<f32>, limit: usize) -> f32 {
        let mut top = self.kitty.y;
        for _ in 0..limit {
            let move_x = aim.map_or(0.0, |x| steer(&self.kitty, x));
            self.step(Controls { move_x, ..Controls::default() });
            top = top.min(self.kitty.y);
            if !self.kitty.airborne() {
                break;
            }
        }
        top
    }
}

fn press() -> Controls {
    Controls { press: true, charge_held: true, ..Controls::default() }
}

fn hold() -> Controls {
    Controls { charge_held: true, ..Controls::default() }
}

fn release() -> Controls {
    Controls { leap: true, ..Controls::default() }
}

fn tap() -> Controls {
    Controls { press: true, leap: true, ..Controls::default() }
}

fn walk(move_x: f32) -> Controls {
    Controls { move_x, ..Controls::default() }
}

/// Slider position that heads for `x` and settles there.
fn steer(kitty: &Kitty, x: f32) -> f32 {
    ((x - kitty.x) / KITTY_SPEED).clamp(-1.0, 1.0)
}

/// The point `f` of the way along a limb's walkable span, from the trunk out.
fn along(limb: &Limb, f: f32) -> f32 {
    let (lo, hi) = limb.walk_range();
    if limb.direction > 0.0 { lo + (hi - lo) * f } else { hi - (hi - lo) * f }
}

/// Kitty hanging halfway along limb `id` (alone), settled.
fn hanging(base: &Sim, id: usize) -> Sim {
    let mut sim = base.alone(id);
    let x = along(&sim.limbs[0], 0.5);
    sim.kitty = Kitty::new(x);
    sim.kitty.standing_on = None;
    sim.kitty.hanging_on = Some(0);
    sim.kitty.y = sim.limbs[0].surface_y(x).unwrap() - GRIP_DEPTH + KITTY_BOX.1;
    sim.run(30, Controls::default());
    assert_eq!(sim.kitty.hanging_on, Some(0));
    sim
}

/// Sturdy branches (not thin, not forks) well clear of the ground.
fn sturdy_branches(sim: &Sim) -> Vec<usize> {
    (0..sim.limbs.len())
        .filter(|&i| !sim.limbs[i].thin && sim.limbs[i].attachment.is_none() && sim.limbs[i].base().y < -300.0)
        .collect()
}

/// Runs `check` on levels 1-5 with each of `seeds`, all at once (a thread each), and collects
/// what each returns.
fn each_level<T: Send>(seeds: std::ops::Range<u64>, check: impl Fn(usize, u64) -> T + Sync) -> Vec<T> {
    let cases: Vec<(usize, u64)> = (1..=5).flat_map(|level| seeds.clone().map(move |seed| (level, seed))).collect();
    let check = &check;
    std::thread::scope(|scope| {
        let runs: Vec<_> = cases.iter().map(|&(level, seed)| scope.spawn(move || check(level, seed))).collect();
        runs.into_iter().map(|run| run.join().unwrap()).collect()
    })
}

/// A full leap from standing on limb 0, then bounce after bounce: hold the jump again as she
/// comes down, land braced, and let go at the best moment as the wood springs back. Returns each
/// flight's height above where she started and the landing that snapped the limb, if one did.
fn bounce(sim: &mut Sim, spot: f32, bounces: usize) -> (Vec<f32>, Option<usize>) {
    bounce_on(sim, 0, spot, bounces)
}

/// As `bounce`, on limb `id`: the landing that snapped it (or, for a fork, its parent).
fn bounce_on(sim: &mut Sim, id: usize, spot: f32, bounces: usize) -> (Vec<f32>, Option<usize>) {
    let start = sim.kitty.y;
    let snaps = |sim: &Sim| {
        let parent = sim.limbs[id].attachment.map(|attached| attached.parent);
        (sim.limbs[id].branch.broken_at, parent.map(|parent| sim.limbs[parent].branch.broken_at))
    };
    let intact = snaps(sim);
    let mut heights = Vec::new();
    sim.charge_and_leap(CHARGE_STEPS + 4);
    for landing in 0..=bounces {
        let mut top = sim.kitty.y;
        let mut pressed = false;
        while sim.kitty.airborne() {
            top = top.min(sim.kitty.y);
            let move_x = steer(&sim.kitty, spot);
            let falling = sim.kitty.velocity > 0.0;
            let c = Controls { press: falling && !pressed, charge_held: falling || pressed, move_x, ..Controls::default() };
            pressed |= falling;
            sim.step(c);
        }
        heights.push(start - top);
        if snaps(sim) != intact {
            return (heights, Some(landing));
        }
        if sim.kitty.standing_on != Some(Support::Limb(id)) || landing == bounces {
            break;
        }
        // Well timed: of every moment to let go over the next few steps, the one that launches
        // her fastest
        let mut ahead = sim.clone();
        let mut best = (0, 0.0);
        for wait in 0..=40 {
            let mut launch = ahead.clone();
            launch.step(release());
            if launch.kitty.velocity < best.1 {
                best = (wait, launch.kitty.velocity);
            }
            ahead.step(hold());
        }
        sim.run(best.0, hold());
        sim.step(release());
        if snaps(sim) != intact {
            return (heights, Some(landing));
        }
    }
    (heights, None)
}

/// A tap is a little hop and a full charge a proper leap, on solid ground.
#[test]
fn tap_hops_and_full_charge_leaps_on_the_ground() {
    let mut sim = Sim::new(1, 0);
    sim.limbs.clear();
    sim.step(tap());
    let hop = -sim.fly(None, 200);
    assert!((60.0..80.0).contains(&hop), "tap hop {hop:.0} px");
    assert_eq!(sim.kitty.standing_on, Some(Support::Ground));

    sim.charge_and_leap(CHARGE_STEPS + 10);
    let steps_before = sim.time;
    let leap = -sim.fly(None, 300);
    let airtime = sim.time - steps_before;
    assert!((450.0..490.0).contains(&leap), "full leap {leap:.0} px");
    assert!((1.1..1.5).contains(&airtime), "airtime {airtime:.2} s");

    // Half a charge is in between, nearer the top (the charge eases in)
    sim.charge_and_leap(CHARGE_STEPS / 2);
    let half = -sim.fly(None, 300);
    assert!(hop < half && half < leap && half > (hop + leap) / 2.0, "half charge {half:.0} px");
}

/// Charging by the trunk gets the full leap; at a bendy tip much of it goes into bending the wood.
#[test]
fn charging_by_the_trunk_beats_a_bendy_tip() {
    let mut compared = 0;
    for level in [1, 3, 5] {
        for seed in 0..3 {
            let base = Sim::new(level, seed);
            for id in sturdy_branches(&base) {
                let tip = along(&base.limbs[id], 1.0);
                if base.limbs[id].give_at(tip) < 0.3 {
                    continue;
                }
                let height = |x: f32| {
                    let mut sim = base.alone(id);
                    assert!(sim.stand_on(0, x));
                    let start = sim.kitty.y;
                    sim.charge_and_leap(CHARGE_STEPS + 4);
                    start - sim.fly(None, 60)
                };
                let (trunk, tip) = (height(along(&base.limbs[id], 0.05)), height(tip));
                assert!(trunk > 440.0, "level {level} seed {seed} limb {id}: only {trunk:.0} px by the trunk");
                assert!(tip < trunk * 0.85, "level {level} seed {seed} limb {id}: {tip:.0} px at the tip vs {trunk:.0}");
                compared += 1;
            }
        }
    }
    assert!(compared > 10, "only {compared} bendy limbs compared");
}

/// Held through the fall, landing braced and let go as the limb springs back: each bounce goes
/// higher than the last, three times running on most limbs, until the bounces level off (or
/// the limb gives out). Now and then (about one limb in 70 over seeds 0-11) one of the first
/// bounces comes off a hard landing lower than the last.
#[test]
fn well_timed_bounces_climb_higher() {
    let (mut limbs, mut third_higher, mut faltered) = (0, 0, 0);
    for level in [1, 3, 5] {
        for seed in 0..3 {
            let base = Sim::new(level, seed);
            for id in sturdy_branches(&base) {
                let spot = along(&base.limbs[id], 0.9);
                // Where it's springy, but not so soft that her push mostly goes into the bend
                if !(0.15..0.42).contains(&base.limbs[id].give_at(spot)) {
                    continue;
                }
                let mut sim = base.alone(id);
                assert!(sim.stand_on(0, spot));
                let (heights, snapped) = bounce(&mut sim, spot, 3);
                let context = format!("level {level} seed {seed} limb {id}: heights {heights:?}, snapped at {snapped:?}");
                // She stays on it, the first two bounces climb, and the third doesn't drop back much
                assert!(heights.len() == 4 || snapped.is_some(), "{context}");
                let climbed = heights.iter().take(3).collect::<Vec<_>>().windows(2).all(|w| w[1] > w[0])
                    && !matches!(heights[..], [_, _, second, third] if third <= second * 0.95);
                if !climbed {
                    println!("faltered: {context}");
                    faltered += 1;
                }
                limbs += 1;
                third_higher += (heights.windows(2).all(|w| w[1] > w[0])) as usize;
            }
        }
    }
    assert!(limbs > 30, "only {limbs} limbs bounced on");
    assert!(faltered * 30 <= limbs, "{faltered} of {limbs} limbs didn't climb the first two bounces");
    assert!(third_higher * 100 >= limbs * 85, "only {third_higher} of {limbs} limbs climbed three bounces running");
}

/// Bouncing ever higher snaps even a sturdy limb (not the goal, which never snaps), and only big
/// bounces do. It cracks first, and breaks partway along, under her paws, leaving a stub.
#[test]
fn over_pumping_snaps_a_sturdy_limb() {
    let (mut snapped, mut pumped) = (0, 0);
    for level in [1, 3, 5] {
        for seed in 0..3 {
            let base = Sim::new(level, seed);
            for id in sturdy_branches(&base).into_iter().filter(|&id| !base.limbs[id].is_goal) {
                let spot = along(&base.limbs[id], 1.0);
                if !(0.15..0.6).contains(&base.limbs[id].give_at(spot)) {
                    continue;
                }
                let mut sim = base.alone(id);
                assert!(sim.stand_on(0, spot));
                let (heights, broke) = bounce(&mut sim, spot, 12);
                if let Some(landing) = broke {
                    snapped += 1;
                    let context = format!("level {level} seed {seed} limb {id}: {heights:?}");
                    assert!(heights[landing] > 750.0, "{context} snapped landing from {:.0} px", heights[landing]);
                    assert!(sim.peak_strain > 0.5, "{context} snapped without cracking first");
                    let limb = &sim.limbs[0];
                    let (n, seg) = (limb.segments(), limb.branch.broken_at.unwrap());
                    assert!(limb.stub && !limb.breakable() && (stub_segments(n).0..=n - 2).contains(&seg),
                            "{context} snapped at segment {seg} of {n} (stub {})", limb.stub);
                }
                // Three landings in a row from way up must have broken it
                if heights.windows(3).any(|w| w.iter().all(|&h| h > 1000.0)) {
                    pumped += 1;
                    assert!(broke.is_some(), "level {level} seed {seed} limb {id} survived {heights:?}");
                }
            }
        }
    }
    assert!(snapped >= 5, "only {snapped} limbs snapped ({pumped} pumped high)");
}

/// An over-pumped sturdy limb snaps partway along its length: the stub left on the tree keeps
/// more than half of it, Kitty can stand on it anywhere and leap from it as from solid wood, and
/// it's stiff and dead, so bouncing on it gets nowhere, and it never snaps again however she
/// bounces.
#[test]
fn an_over_pumped_limb_leaves_a_stub_to_stand_and_leap_on() {
    let mut stubs = 0;
    for level in [1, 3, 5] {
        for seed in 0..3 {
            let base = Sim::new(level, seed);
            for id in sturdy_branches(&base).into_iter().filter(|&id| !base.limbs[id].is_goal) {
                let spot = along(&base.limbs[id], 1.0);
                if !(0.15..0.6).contains(&base.limbs[id].give_at(spot)) {
                    continue;
                }
                let mut sim = base.alone(id);
                assert!(sim.stand_on(0, spot));
                if bounce(&mut sim, spot, 12).1.is_none() {
                    continue;
                }
                sim.run(120, Controls::default());
                let context = format!("level {level} seed {seed} limb {id}");
                let limb = sim.limbs[0].clone();
                let seg = limb.branch.broken_at.unwrap();
                assert!(seg as f32 >= 0.55 * limb.segments() as f32, "{context}: stub of {seg} segments of {}", limb.segments());
                let (lo, hi) = limb.walk_range();
                assert!(limb.surface_y(lo).is_some() && limb.surface_y(hi).is_some(), "{context}: walk range off the stub");
                assert!(limb.give_at(along(&limb, 1.0)) < 0.2, "{context}: the stub still gives {:.2}", limb.give_at(along(&limb, 1.0)));
                for f in [0.0, 0.5, 1.0] {
                    let mut sim = sim.clone();
                    assert!(sim.stand_on(0, along(&limb, f)), "{context}: can't stand on the stub at {f}");
                }
                // A full leap from it, as from solid wood
                let mut leap = sim.clone();
                assert!(leap.stand_on(0, along(&limb, 0.5)));
                let start = leap.kitty.y;
                leap.charge_and_leap(CHARGE_STEPS + 4);
                let height = start - leap.fly(None, 100);
                assert!(height > 440.0, "{context}: only {height:.0} px from the stub");
                // Bouncing on its end gets nowhere, and never snaps it
                let end = along(&limb, 1.0);
                assert!(sim.stand_on(0, end));
                let (heights, broke) = bounce(&mut sim, end, 10);
                assert!(broke.is_none() && sim.limbs[0].branch.broken_at == Some(seg) && sim.limbs[0].stub,
                        "{context}: the stub snapped again ({heights:?})");
                assert!(heights.iter().all(|&h| h < 600.0), "{context}: bounced on the stub {heights:?}");
                stubs += 1;
            }
        }
    }
    assert!(stubs >= 5, "only {stubs} stubs");
}

/// Over-pumping costs her the climb: when a sturdy limb snaps with her middle paw past the break,
/// she goes down with the piece that falls, and the stub, springing back up as it loses that
/// piece and her weight, doesn't catch her back paw (nor can she land on or grab it) for the next
/// second. With her middle paw on the stub side she stays on the stub.
#[test]
fn snapping_past_her_paw_takes_her_down_with_the_piece() {
    let results = each_level(0..4, |level, seed| {
        let base = Sim::new(level, seed);
        let (mut fell, mut stayed) = (0, 0);
        for id in sturdy_branches(&base).into_iter().filter(|&id| !base.limbs[id].is_goal) {
            for f in [0.8, 0.9, 1.0] {
                let spot = along(&base.limbs[id], f);
                let mut sim = base.alone(id);
                assert!(sim.stand_on(0, spot));
                let (heights, broke) = bounce(&mut sim, spot, 12);
                if broke.is_none() {
                    continue;
                }
                let context = format!("level {level} seed {seed} limb {id} at {f}: {heights:?}");
                assert!(sim.limbs[0].just_snapped && sim.kitty.standing_on == Some(Support::Limb(0)), "{context}");
                let past = sim.limbs[0].fell_away(sim.kitty.x);
                for step in 0..60 {
                    sim.step(Controls::default());
                    let on = sim.kitty.standing_on == Some(Support::Limb(0)) || sim.kitty.hanging_on == Some(0) || sim.kitty.climbing_on == Some(0);
                    assert!(on != past, "{context}: middle paw past the break {past}, on the stub {on} {step} steps after the snap");
                }
                if past {
                    fell += 1;
                } else {
                    stayed += 1;
                }
            }
        }
        (fell, stayed)
    });
    let (fell, stayed) = results.iter().fold((0, 0), |(f, s), r| (f + r.0, s + r.1));
    println!("{fell} snaps took her down, {stayed} left her on the stub");
    assert!(fell >= 50, "only {fell} snaps past her middle paw");
}

/// Only the wood that gives way can't catch her: when a fork snaps under Kitty with her middle paw
/// past the break, she goes down with the piece that falls, and its stub, springing back up, doesn't
/// catch her (nor can she land on or grab it) for the next second, but the parent, whole, does
/// wherever it's under her. With her middle paw on the stub side she stays on the stub.
#[test]
fn a_fork_snapping_under_her_drops_her_onto_its_parent() {
    let results = each_level(0..4, |level, seed| {
        let base = Sim::new(level, seed);
        let (mut onto_parent, mut past_parent, mut stayed) = (0, 0, 0);
        for fork in (0..base.limbs.len()).filter(|&i| base.limbs[i].attachment.is_some()) {
            for f in [0.7, 0.85, 1.0] {
                let mut sim = base.family(fork);
                let spot = along(&sim.limbs[1], f);
                if !sim.stand_on(1, spot) {
                    continue;
                }
                let (heights, broke) = bounce_on(&mut sim, 1, spot, 14);
                // The fork snapped, not its parent
                if broke.is_none() || sim.limbs[0].broken() {
                    continue;
                }
                let context = format!("level {level} seed {seed} fork {fork} at {f}: {heights:?}");
                assert!(sim.limbs[1].just_snapped && sim.limbs[1].stub && sim.kitty.standing_on == Some(Support::Limb(1)), "{context}");
                let past = sim.limbs[1].fell_away(sim.kitty.x);
                // Under her middle paw (under only an outer one, the wind can blow her off its end)
                let parent_under = sim.limbs[0].surface_y(sim.kitty.x).is_some_and(|s| s >= sim.kitty.y);
                let mut caught = false;
                for step in 0..60 {
                    sim.step(Controls::default());
                    let k = &sim.kitty;
                    let on = |id: usize| k.standing_on == Some(Support::Limb(id)) || k.hanging_on == Some(id) || k.climbing_on == Some(id);
                    assert!(on(1) != past, "{context}: middle paw past the break {past}, on the stub {} {step} steps after the snap", on(1));
                    caught |= on(0);
                }
                if !past {
                    stayed += 1;
                } else {
                    assert!(caught || !parent_under, "{context}: fell through the parent under her to y {:.0} on {:?}", sim.kitty.y, sim.kitty.standing_on);
                    if caught {
                        onto_parent += 1;
                    } else {
                        past_parent += 1;
                    }
                }
            }
        }
        (onto_parent, past_parent, stayed)
    });
    let (onto_parent, past_parent, stayed) = results.iter().fold((0, 0, 0), |t, r| (t.0 + r.0, t.1 + r.1, t.2 + r.2));
    println!("snapped forks: {onto_parent} dropped her onto the parent, {past_parent} past it, {stayed} left her on the stub");
    assert!(onto_parent >= 10, "only {onto_parent} snapped forks dropped her onto the parent");
}

/// Nor do the forks on a stub catch her: when a limb snaps just past where a fork grows out, with
/// her middle paw past the break, the fork stays on the stub and springs up with it, but she goes
/// down with the piece that falls, past the fork, as she does past the stub.
#[test]
fn a_parent_snapping_past_a_fork_drops_her_past_the_fork() {
    let results = each_level(0..4, |level, seed| {
        let base = Sim::new(level, seed);
        let mut fell = 0;
        for fork in (0..base.limbs.len()).filter(|&i| base.limbs[i].attachment.is_some()) {
            let node = base.limbs[fork].attachment.unwrap().node;
            for dx in [2.0, 6.0, 10.0] {
                let mut sim = base.family(fork);
                let x = sim.limbs[0].top_point(node).x + dx * sim.limbs[0].direction;
                if !sim.stand_on(0, x) {
                    continue;
                }
                sim.limbs[0].snap_at(sim.kitty.x);
                // Only where the fork stays on the stub and she's past the break
                if sim.limbs[0].branch.broken_at.is_none_or(|at| at < node) || !sim.limbs[0].fell_away(sim.kitty.x) {
                    continue;
                }
                let context = format!("level {level} seed {seed} fork {fork} at {dx} px past its root");
                for step in 0..60 {
                    sim.step(Controls::default());
                    let k = &sim.kitty;
                    let on = |id: usize| k.standing_on == Some(Support::Limb(id)) || k.hanging_on == Some(id) || k.climbing_on == Some(id);
                    assert!(!on(0) && !on(1), "{context}: caught by the {} {step} steps after the snap", if on(0) { "stub" } else { "fork" });
                }
                fell += 1;
            }
        }
        fell
    });
    let fell: usize = results.iter().sum();
    println!("{fell} snaps past a fork's root took her down past the fork");
    assert!(fell >= 20, "only {fell} snaps past a fork's root");
}

/// Falling from a limb that gave way doesn't outlast catching another: hanging from a sturdy limb
/// that snaps past her paws, she falls and catches one just below it, climbs up, and a hop
/// straight back up that takes her over the stub lands her on it.
#[test]
fn after_a_catch_she_can_land_back_on_the_stub_she_fell_from() {
    let limb = |y: f32, seed: u64| {
        let mut limb = Limb::new(&LimbSpec {
            start: vec2(100.0, y), direction: 1.0, length: 210.0, thickness: 34.0, stiffness: 0.92, gnarl: 0.5, rise: 0.0,
            strength: STURDY_BRANCH_STRENGTH * KITTY_WEIGHT, thin: false, tree: 0, trunk_half: TRUNK_WIDTH / 2.0, attachment: None, seed,
        });
        limb.settle();
        limb
    };
    let mut landed = 0;
    for below in [35.0, 40.0, 45.0, 50.0, 55.0, 60.0] {
        let mut sim = Sim::new(1, 0);
        sim.limbs = vec![limb(-500.0, 1), limb(-500.0 + below, 2)];
        sim.wind = 0.0;
        let x = along(&sim.limbs[0], 0.9);
        sim.kitty = Kitty::new(x);
        sim.kitty.standing_on = None;
        sim.kitty.hanging_on = Some(0);
        sim.kitty.y = sim.limbs[0].surface_y(x).unwrap() - GRIP_DEPTH + KITTY_BOX.1;
        sim.run(30, Controls::default());
        sim.limbs[0].snap_at(x);
        let caught = (0..60).any(|_| {
            sim.step(Controls::default());
            sim.kitty.hanging_on == Some(1)
        });
        assert!(caught, "{below} px below: didn't catch the lower limb");
        sim.step(tap());
        sim.run(CLIMB_STEPS as usize + 3, Controls::default());
        assert_eq!(sim.kitty.standing_on, Some(Support::Limb(1)), "{below} px below: didn't climb up");
        // A hop for the middle of the stub
        let aim = along(&sim.limbs[0], 0.4);
        sim.step(Controls { move_x: steer(&sim.kitty, aim), ..tap() });
        let mut over = false;
        for _ in 0..120 {
            sim.step(walk(steer(&sim.kitty, aim)));
            over |= sim.limbs[0].surface_y(sim.kitty.x).is_some_and(|s| sim.kitty.y < s);
            if !sim.kitty.airborne() {
                break;
            }
        }
        if over {
            assert_eq!(sim.kitty.standing_on, Some(Support::Limb(0)), "{below} px below: hopped over the stub but passed through it");
            landed += 1;
        }
    }
    assert!(landed >= 2, "only {landed} hops got over the stub");
}

/// The goal limb, with the friend's nest, never snaps, however hard Kitty bounces on it anywhere.
#[test]
fn the_goal_limb_never_snaps() {
    let mut checked = 0;
    for level in 1..=5 {
        for seed in 0..4 {
            let base = Sim::new(level, seed);
            let goal = (0..base.limbs.len()).find(|&i| base.limbs[i].is_goal).unwrap();
            assert!(!base.limbs[goal].breakable());
            for f in [0.3, 0.6, 1.0] {
                let mut sim = base.alone(goal);
                let spot = along(&sim.limbs[0], f);
                assert!(sim.stand_on(0, spot));
                let (heights, broke) = bounce(&mut sim, spot, 15);
                assert!(broke.is_none() && !sim.limbs[0].broken(), "level {level} seed {seed} at {f}: snapped the goal {heights:?}");
                checked += 1;
            }
        }
    }
    assert!(checked >= 60);
}

/// Sturdy limbs take what play throws at them: landings from a full leap braced or not, a long
/// fall relaxed or braced (holding the jump all the way down, which is no bounce), and Kitty
/// crouching with two dogs on it.
#[test]
fn sturdy_limbs_hold_landings_and_dogs() {
    let mut checked = 0;
    for level in [1, 3, 5] {
        for seed in 0..3 {
            let base = Sim::new(level, seed);
            for id in sturdy_branches(&base) {
                for f in [0.1, 0.5, 0.95] {
                    let spot = along(&base.limbs[id], f);
                    // A full leap straight up and back down, relaxed, then held through the fall
                    for braced in [false, true] {
                        let mut sim = base.alone(id);
                        assert!(sim.stand_on(0, spot));
                        sim.charge_and_leap(CHARGE_STEPS + 4);
                        while sim.kitty.velocity < 0.0 {
                            sim.step(walk(steer(&sim.kitty, spot)));
                        }
                        sim.step(if braced { press() } else { Controls::default() });
                        let c = if braced { hold() } else { Controls::default() };
                        while sim.kitty.airborne() {
                            sim.step(Controls { move_x: steer(&sim.kitty, spot), ..c });
                        }
                        sim.run(120, c);
                        assert!(!sim.limbs[0].broken(), "level {level} seed {seed} limb {id} at {f}: a full leap (braced {braced}) snapped it");
                    }
                    // A long fall from high above, landing relaxed, then braced
                    for braced in [false, true] {
                        let mut sim = base.alone(id);
                        sim.kitty = Kitty::new(spot);
                        sim.kitty.standing_on = None;
                        sim.kitty.falling = true;
                        sim.kitty.y = sim.limbs[0].surface_y(spot).unwrap() - 1500.0;
                        let c = if braced { hold() } else { Controls::default() };
                        sim.step(if braced { press() } else { c });
                        while sim.kitty.airborne() {
                            sim.step(Controls { move_x: steer(&sim.kitty, spot), ..c });
                        }
                        assert!(sim.kitty.standing_on == Some(Support::Limb(0)) && sim.kitty.crouching() == braced);
                        sim.run(60, c);
                        assert!(!sim.limbs[0].broken(), "level {level} seed {seed} limb {id} at {f}: a long fall (braced {braced}) snapped it");
                    }
                    // Kitty crouched, and two dogs either side of her
                    let mut sim = base.alone(id);
                    assert!(sim.stand_on(0, spot));
                    for _ in 0..600 {
                        for g in [0.3, 0.7] {
                            load_limb(&mut sim.limbs, 0, along(&base.limbs[id], g), dog::WEIGHT);
                        }
                        sim.step(if sim.kitty.crouching() { hold() } else { press() });
                    }
                    assert!(!sim.limbs[0].broken(), "level {level} seed {seed} limb {id} at {f}: Kitty and two dogs snapped it");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 50);
}

/// Thin limbs hold Kitty crouching anywhere, landing from a full leap, and landing braced from a
/// fall (that's no bounce), but not Kitty and a dog. A small bounce on one that holds as she
/// lands holds while she crouches on after it.
#[test]
fn thin_limbs_hold_kitty_but_give_out_sooner() {
    let mut checked = 0;
    for level in 1..=5 {
        for seed in 0..4 {
            let base = Sim::new(level, seed);
            for id in (0..base.limbs.len()).filter(|&i| base.limbs[i].thin) {
                for f in [0.05, 0.3, 0.6, 0.9, 1.0] {
                    let spot = along(&base.limbs[id], f);
                    let mut sim = base.alone(id);
                    assert!(sim.stand_on(0, spot));
                    sim.step(press());
                    sim.run(400, hold());
                    assert!(!sim.limbs[0].broken() && sim.kitty.crouching(), "level {level} seed {seed} limb {id} at {f}: crouching snapped it");
                    // A full leap straight up and back down onto it
                    sim.step(release());
                    sim.fly(Some(spot), 300);
                    sim.run(200, Controls::default());
                    assert!(!sim.limbs[0].broken(), "level {level} seed {seed} limb {id} at {f}: landing snapped it");
                    // With a dog as well, it gives
                    for _ in 0..120 {
                        load_limb(&mut sim.limbs, 0, along(&base.limbs[id], 0.5), dog::WEIGHT);
                        sim.step(Controls::default());
                    }
                    if sim.kitty.standing_on == Some(Support::Limb(0)) {
                        assert!(sim.limbs[0].broken(), "level {level} seed {seed} limb {id} at {f}: held Kitty and a dog");
                    }
                    // Falling onto it from well above, braced all the way down
                    let mut sim = base.alone(id);
                    sim.kitty = Kitty::new(spot);
                    sim.kitty.standing_on = None;
                    sim.kitty.falling = true;
                    sim.kitty.y = sim.limbs[0].surface_y(spot).unwrap() - 450.0;
                    sim.step(press());
                    while sim.kitty.airborne() {
                        sim.step(Controls { move_x: steer(&sim.kitty, spot), ..hold() });
                    }
                    sim.run(200, hold());
                    assert!(!sim.limbs[0].broken(), "level {level} seed {seed} limb {id} at {f}: a braced landing from a fall snapped it");
                    // A part-charged hop straight up, held through the landing, then crouching on
                    let mut sim = base.alone(id);
                    assert!(sim.stand_on(0, spot));
                    sim.charge_and_leap(12);
                    while sim.kitty.velocity < 0.0 {
                        sim.step(walk(steer(&sim.kitty, spot)));
                    }
                    sim.step(press());
                    while sim.kitty.airborne() {
                        sim.step(Controls { move_x: steer(&sim.kitty, spot), ..hold() });
                    }
                    sim.run(2, hold());
                    if !sim.limbs[0].broken() && sim.kitty.crouching() {
                        sim.run(400, hold());
                        assert!(!sim.limbs[0].broken(), "level {level} seed {seed} limb {id} at {f}: crouching after a small bounce snapped it");
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 40, "only {checked} thin limb spots");
}

/// A release a few steps before landing leaps on landing, and one just after walking off an
/// edge still leaps; later ones don't.
#[test]
fn released_just_before_landing_or_after_walking_off_still_leaps() {
    let mut sim = Sim::new(1, 0);
    sim.limbs.clear();
    for early in [1, 4, LEAP_BUFFER_STEPS, LEAP_BUFFER_STEPS + 3] {
        let mut drop = sim.clone();
        drop.kitty.standing_on = None;
        drop.kitty.falling = true;
        drop.kitty.y = -400.0;
        // Find when she'll land, then release `early` steps before it
        let mut probe = drop.clone();
        let mut fall_steps = 0;
        while probe.kitty.airborne() {
            probe.step(Controls::default());
            fall_steps += 1;
        }
        drop.step(press());
        for _ in 0..fall_steps - early as usize - 2 {
            drop.step(hold());
        }
        drop.step(release());
        let mut leapt = false;
        for _ in 0..early + 2 {
            leapt |= drop.step(Controls::default()).jumped;
        }
        assert_eq!(leapt, early <= LEAP_BUFFER_STEPS, "released {early} steps before landing");
    }

    // Walking off the end of a branch with a tap a few steps later
    let mut walked = 0;
    for level in [1, 3] {
        for seed in 0..3 {
            let base = Sim::new(level, seed);
            for id in sturdy_branches(&base) {
                for late in [1, 3, COYOTE_STEPS, COYOTE_STEPS + 2] {
                    let mut sim = base.alone(id);
                    let dir = sim.limbs[0].direction;
                    assert!(sim.stand_on(0, along(&base.limbs[id], 0.9)));
                    let mut off = None;
                    for step in 0..200 {
                        sim.step(walk(dir));
                        if sim.kitty.airborne() {
                            off = Some(step);
                            break;
                        }
                    }
                    if off.is_none() || sim.kitty.panicking {
                        continue;
                    }
                    sim.run(late as usize - 1, walk(dir));
                    let leapt = sim.step(tap()).jumped;
                    assert_eq!(leapt, late <= COYOTE_STEPS, "level {level} seed {seed} limb {id}: tap {late} steps after walking off");
                    walked += 1;
                }
            }
        }
    }
    assert!(walked > 20, "only {walked} walk-offs");
}

/// Standing still or crouching at a tip as it bends in under her, or hanging from it, she never
/// slips off.
#[test]
fn standing_and_hanging_at_bending_tips_never_slips() {
    let mut checked = 0;
    for level in 1..=5 {
        for seed in 0..3 {
            let base = Sim::new(level, seed);
            for id in (0..base.limbs.len()).filter(|&i| base.limbs[i].attachment.is_none()) {
                let limb = &base.limbs[id];
                // Out past the very end: only her inner paw on the wood
                let edge = limb.tip().x + limb.direction * (KITTY_FOOT_REACH - TIP_GRIP - 1.0);
                let mut sim = base.alone(id);
                sim.kitty = Kitty::new(edge);
                let feet = [edge - KITTY_FOOT_REACH, edge, edge + KITTY_FOOT_REACH];
                let Some(s) = sim.limbs[0].surface_under(&feet) else { continue };
                sim.kitty.y = s;
                sim.kitty.standing_on = Some(Support::Limb(0));
                sim.run(200, Controls::default());
                sim.step(press());
                sim.run(200, hold());
                assert!(sim.kitty.standing_on == Some(Support::Limb(0)) || sim.limbs[0].broken(),
                        "level {level} seed {seed} limb {id}: slipped off the tip standing");
                // Hanging from the end of the walkable span
                let mut sim = base.alone(id);
                let x = along(limb, 1.0);
                let s = sim.limbs[0].surface_y(x).unwrap();
                sim.kitty = Kitty::new(x);
                sim.kitty.standing_on = None;
                sim.kitty.hanging_on = Some(0);
                sim.kitty.y = s - GRIP_DEPTH + KITTY_BOX.1;
                if sim.kitty.y >= GROUND_Y {
                    continue;
                }
                sim.run(300, Controls::default());
                if sim.kitty.standing_on == Some(Support::Ground) {
                    continue; // a low limb sagged until her feet touched the grass
                }
                assert!(sim.kitty.hanging_on == Some(0) || sim.limbs[0].broken(),
                        "level {level} seed {seed} limb {id}: slipped off the tip hanging (x {:.0}, walk {:?}, standing {:?})",
                        sim.kitty.x, sim.limbs[0].walk_range(), sim.kitty.standing_on);
                // And climbing up from there
                sim.step(tap());
                sim.run(CLIMB_STEPS as usize + 30, Controls::default());
                assert!(sim.kitty.standing_on == Some(Support::Limb(0)) || sim.limbs[0].broken(),
                        "level {level} seed {seed} limb {id}: slipped climbing up at the tip");
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "only {checked} tips");
}

/// A swipe down on a limb drops her through it to something lower, never back onto the same
/// limb; on the ground it does nothing; hanging, it lets go.
#[test]
fn dropping_through_lands_lower_never_on_the_same_limb() {
    let swipe = Controls { drop: true, ..Controls::default() };
    let mut dropped = 0;
    for level in 1..=5 {
        for seed in 0..2 {
            let base = Sim::new(level, seed);
            for id in 0..base.limbs.len() {
                for f in [0.25, 0.9] {
                    let mut sim = base.clone();
                    if !sim.stand_on(id, along(&base.limbs[id], f)) {
                        continue;
                    }
                    let (start, before) = (sim.kitty.y, sim.limbs.clone());
                    let feet = [sim.kitty.x - KITTY_FOOT_REACH, sim.kitty.x, sim.kitty.x + KITTY_FOOT_REACH];
                    sim.step(swipe);
                    for _ in 0..400 {
                        sim.step(Controls::default());
                        if !sim.kitty.airborne() {
                            break;
                        }
                    }
                    // Not back on that limb, nor caught by one level with it where she dropped (a
                    // fork's parent at the join, say)
                    let caught = match (sim.kitty.standing_on, sim.kitty.hanging_on) {
                        (Some(Support::Limb(on)), _) | (_, Some(on)) => Some(on),
                        _ => None,
                    };
                    let level_with = |on: usize| on == id || before[on].surface_under(&feet).is_some_and(|s| (s - start).abs() <= DROP_SKIP_NEAR);
                    assert!(!caught.is_some_and(level_with) && sim.kitty.y > start + DROP_SKIP_NEAR,
                            "level {level} seed {seed} limb {id} at {f}: from {start:.0} to {:.0} on {:?} hanging {:?}",
                            sim.kitty.y, sim.kitty.standing_on, sim.kitty.hanging_on);
                    dropped += 1;
                }
            }
        }
    }
    assert!(dropped > 100, "only {dropped} drops");
    // Nothing happens on the ground
    let mut sim = Sim::new(1, 0);
    sim.step(swipe);
    sim.run(10, Controls::default());
    assert_eq!(sim.kitty.standing_on, Some(Support::Ground));
}

/// A swipe or a hit while charging cancels the charge: no leap when the finger lifts.
#[test]
fn swiping_or_a_hit_cancels_a_charge() {
    let mut sim = Sim::new(1, 0);
    sim.limbs.clear();
    sim.step(press());
    sim.run(20, hold());
    // The swipe clears the hold, and the lift after it isn't a release
    let step = sim.step(Controls { drop: true, ..Controls::default() });
    assert!(!step.jumped && !sim.kitty.crouching());
    sim.run(5, Controls::default());
    assert_eq!(sim.kitty.standing_on, Some(Support::Ground));

    sim.step(press());
    sim.run(20, hold());
    assert!(sim.kitty.hurt(sim.kitty.x + 50.0));
    sim.run(5, hold());
    assert_eq!(sim.kitty.charge_level(), None);
    // Knocked back and down again, still holding: letting go now does nothing
    sim.run(60, hold());
    assert!(!sim.step(release()).jumped);
}

/// Hanging, a tap climbs at once and a held press climbs a moment later, but a press that turns
/// into a swipe lets go instead; a press late in a climb carries over into a crouch.
#[test]
fn hanging_climbs_on_a_press_and_lets_go_on_a_swipe() {
    let base = Sim::new(1, 0);
    let id = sturdy_branches(&base)[0];
    let hang = || hanging(&base, id);
    let mut sim = hang();
    sim.step(tap());
    assert_eq!(sim.kitty.climbing_on, Some(0));

    let mut sim = hang();
    sim.step(press());
    sim.run(CLIMB_HOLD_STEPS as usize - 2, hold());
    assert_eq!(sim.kitty.hanging_on, Some(0), "climbed before the hold was sure");
    sim.run(2, hold());
    assert_eq!(sim.kitty.climbing_on, Some(0), "a held press didn't climb");

    let mut sim = hang();
    sim.step(press());
    sim.run(2, hold());
    sim.step(Controls { drop: true, ..Controls::default() });
    assert!(sim.kitty.airborne() && sim.kitty.climbing_on.is_none(), "a swipe climbed instead of letting go");

    // Pressed near the top of the climb and held: she arrives crouched, and the release leaps
    let mut sim = hang();
    sim.step(tap());
    sim.run(CLIMB_STEPS as usize - 10, Controls::default());
    sim.step(press());
    sim.run(20, hold());
    assert!(sim.kitty.crouching(), "the press late in the climb didn't carry over");
    assert!(sim.step(release()).jumped);
    // Released before she's up: she leaps as soon as she is
    let mut sim = hang();
    sim.step(tap());
    sim.run(CLIMB_STEPS as usize - 10, Controls::default());
    sim.step(tap());
    let leapt = (0..12).any(|_| sim.step(Controls::default()).jumped);
    assert!(leapt, "a tap late in the climb was lost");
}

/// A press made early in a climb, and still held when she's nearly up, crouches her, and the
/// release leaps, however soon after the tap it came; a quick second tap during the climb is let
/// go first and still does nothing.
#[test]
fn a_press_held_through_a_climb_crouches_her_once_up() {
    let base = Sim::new(1, 0);
    let id = sturdy_branches(&base)[0];
    for wait in [0, 2, 5, 8, 12, 14, 16, 17, 19, 25] {
        let mut sim = hanging(&base, id);
        sim.step(tap());
        assert_eq!(sim.kitty.climbing_on, Some(0));
        sim.run(wait, Controls::default());
        sim.step(press());
        sim.run(60, hold());
        assert!(sim.kitty.crouching(), "pressed {wait} steps into the climb and held: not crouching");
        assert!(sim.step(release()).jumped, "pressed {wait} steps into the climb: the release didn't leap");
    }
    let mut sim = hanging(&base, id);
    sim.step(tap());
    sim.run(5, Controls::default());
    sim.step(tap());
    let leapt = (0..60).any(|_| sim.step(Controls::default()).jumped);
    assert!(!leapt && sim.kitty.standing_on == Some(Support::Limb(0)) && !sim.kitty.crouching(), "a double tap leapt");
}

/// Holding the jump to climb, or catching a branch with the jump still held from the air, she
/// climbs and arrives crouched; letting go then leaps.
#[test]
fn holding_on_through_a_climb_crouches_her_on_arrival() {
    let base = Sim::new(1, 0);
    let id = sturdy_branches(&base)[0];
    let mut sim = hanging(&base, id);
    sim.step(press());
    sim.run(80, hold());
    assert!(sim.kitty.standing_on == Some(Support::Limb(0)) && sim.kitty.crouching(), "held to climb: not crouching on arrival");
    assert!(sim.step(release()).jumped, "held to climb: the release didn't leap");

    // Falling just short of the branch with the jump held
    let mut sim = base.alone(id);
    let x = along(&sim.limbs[0], 0.5);
    let s = sim.limbs[0].surface_y(x).unwrap();
    sim.kitty = Kitty::new(x);
    sim.kitty.standing_on = None;
    sim.kitty.falling = true;
    sim.kitty.y = s + 60.0;
    sim.step(press());
    let grabbed = (0..30).any(|_| {
        sim.step(hold());
        sim.kitty.hanging_on.is_some() || sim.kitty.climbing_on.is_some()
    });
    assert!(grabbed, "didn't catch the branch");
    sim.run(80, hold());
    assert!(sim.kitty.standing_on == Some(Support::Limb(0)) && sim.kitty.crouching(), "caught it charged: not crouching once up");
    assert!(sim.step(release()).jumped, "caught it charged: the release didn't leap");
}

/// Catching a low limb, hanging while it sags, then letting go: the ground always catches her.
#[test]
fn letting_go_of_a_low_limb_lands_on_the_ground() {
    let mut grabs = 0;
    for seed in 0..15 {
        let base = Sim::new(1, seed);
        let low: Vec<usize> = base.limbs.iter().enumerate()
            .filter(|(_, l)| l.base().y > -200.0).map(|(i, _)| i).collect();
        for &id in &low {
            for k in 0..6 {
                let mut sim = base.clone();
                let (lo, hi) = sim.limbs[id].walk_range();
                sim.kitty = Kitty::new(lo + (hi - lo) * (k as f32 + 0.5) / 6.0);
                sim.step(tap());
                let hung = (0..120).any(|_| {
                    sim.step(Controls::default());
                    sim.kitty.hanging_on.is_some()
                });
                if !hung {
                    continue;
                }
                grabs += 1;
                sim.run(60, Controls::default());
                sim.step(Controls { drop: true, ..Controls::default() });
                sim.run(240, Controls::default());
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
            let base = Sim::new(level, seed);
            let forks: Vec<usize> = base.limbs.iter().enumerate()
                .filter(|(_, l)| l.attachment.is_some()).map(|(i, _)| i).collect();
            for &fork in &forks {
                let mut sim = base.clone();
                let parent = sim.limbs[fork].attachment.unwrap().parent;
                let toward_trunk = -sim.limbs[fork].direction;
                if !sim.stand_on(fork, along(&base.limbs[fork], 0.7)) {
                    continue;
                }
                for _ in 0..200 {
                    let was_on_fork = sim.kitty.standing_on == Some(Support::Limb(fork));
                    sim.step(walk(toward_trunk));
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

/// A fork whose parent snaps off between the trunk and the fork goes down too: Kitty standing on
/// it falls (she doesn't ride it down out of the world) and what rested on it goes. A fork left on
/// the parent's stub stays, with Kitty on it.
#[test]
fn a_fork_goes_down_with_its_parent() {
    let results = each_level(0..4, |level, seed| {
        let base = Sim::new(level, seed);
        let (mut fell, mut stayed) = (0, 0);
        for fork in (0..base.limbs.len()).filter(|&i| base.limbs[i].attachment.is_some()) {
            let attachment = base.limbs[fork].attachment.unwrap();
            let parent = attachment.parent;
            let context = format!("level {level} seed {seed} fork {fork}");
            let mut sim = base.clone();
            if !sim.stand_on(fork, along(&base.limbs[fork], 0.7)) {
                continue;
            }
            let gold = Item::new(ItemKind::Gold, fork, 0.8, 0.0);
            // Snapped just inside the fork
            let mut inside = sim.clone();
            let branch = &mut inside.limbs[parent].branch;
            branch.snap_distance = (attachment.node as f32 - 0.5) * branch.segment_len;
            branch.snap();
            inside.run(300, Controls::default());
            let limb = &inside.limbs[fork];
            assert!(limb.gone() && limb.surface_y(along(&base.limbs[fork], 0.5)).is_none(), "{context}: still there");
            assert!(gold.position(&inside.limbs, 0.0).is_none(), "{context}: the gold stayed in the air");
            assert!(inside.kitty.standing_on != Some(Support::Limb(fork)) && inside.kitty.y <= GROUND_Y + 0.5,
                    "{context}: rode it down to y {:.0} on {:?}", inside.kitty.y, inside.kitty.standing_on);
            fell += 1;
            // Snapped as short as it can, which leaves most forks on the stub
            let mut short = sim.clone();
            let at = short.limbs[parent].base().x;
            short.limbs[parent].snap_at(at);
            short.run(120, Controls::default());
            if short.limbs[parent].branch.broken_at.unwrap() >= attachment.node {
                assert!(!short.limbs[fork].broken() && short.kitty.standing_on == Some(Support::Limb(fork)),
                        "{context}: a fork on the stub went down");
                assert!(gold.position(&short.limbs, 0.0).is_some(), "{context}: the gold on a fork on the stub went");
                stayed += 1;
            }
        }
        (fell, stayed)
    });
    let (fell, stayed) = results.iter().fold((0, 0), |(f, s), r| (f + r.0, s + r.1));
    assert!(fell > 20 && stayed > 20, "only {fell} forks fell and {stayed} stayed");
}

/// Leaps from nine spots along limb `from` (or the ground under limb `to`), steered toward limb
/// `to`: which of `charges` (steps of crouching, tried in order) first lands one on a path branch
/// at or above `to` (or a fork growing from one), if any does. Each spot is tried with four aims:
/// the nearest end of the target, or across it, in case another branch overhead is in the way.
fn leap_to(base: &Sim, from: Option<usize>, to: usize, charges: &[u32]) -> Option<u32> {
    let target = &base.limbs[to];
    let (tlo, thi) = target.walk_range();
    let spots: Vec<f32> = match from {
        None => (0..9).map(|k| (tlo - 120.0 + (thi - tlo + 240.0) * k as f32 / 8.0).clamp(60.0, WORLD_W - 60.0)).collect(),
        Some(id) => (0..9).map(|k| along(&base.limbs[id], k as f32 / 8.0)).collect(),
    };
    // Kitty settled at each spot, worked out as they're needed
    let mut standing: Vec<Option<Option<Sim>>> = vec![None; spots.len()];
    let mut lands = |charge: u32| (0..spots.len()).any(|k| {
        let x = spots[k];
        let settled = standing[k].get_or_insert_with(|| {
            let mut sim = base.clone();
            match from {
                Some(id) => sim.stand_on(id, x).then_some(sim),
                None => {
                    sim.kitty = Kitty::new(x);
                    Some(sim)
                }
            }
        });
        let Some(sim) = settled else { return false };
        let mut sim = sim.clone();
        sim.step(press());
        sim.run(charge as usize - 1, hold());
        let nearest = sim.kitty.x.clamp(tlo + 20.0, thi - 20.0);
        [nearest, along(target, 0.15), along(target, 0.5), along(target, 0.85)].into_iter().any(|aim| {
            let mut leap = sim.clone();
            leap.step(release());
            leap.fly(Some(aim), 400);
            let Some(Support::Limb(on)) = leap.kitty.standing_on else { return false };
            let branch = leap.limbs[on].attachment.map_or(on, |a| a.parent);
            branch >= to && branch < leap.path
        })
    });
    charges.iter().copied().find(|&charge| lands(charge))
}

/// A full charge, then lighter ones for where another branch hangs low over the target and a full
/// leap would sail up onto that instead.
const CHARGES: [u32; 4] = [CHARGE_STEPS + 1, 22, 16, 12];

/// Every level can be climbed without bouncing: from somewhere on each path branch (and the
/// ground), one charged leap, steered toward somewhere on the next path branch, lands on it (or
/// on one further up the path, or a fork growing from one, where those are right overhead). A
/// full charge does it nearly everywhere; where another branch hangs low over the next one, a
/// full leap would sail up onto that instead, and a lighter charge drops onto the path.
#[test]
fn every_path_branch_is_one_leap_from_the_last() {
    let results = each_level(0..4, |level, seed| {
        let base = Sim::new(level, seed);
        let (mut full, mut lighter) = (0, 0);
        for to in 0..base.path {
            let charge = leap_to(&base, to.checked_sub(1), to, &CHARGES);
            assert!(charge.is_some(), "level {level} seed {seed}: no leap from path branch {} reaches {to}", to as isize - 1);
            if charge == Some(CHARGES[0]) {
                full += 1;
            } else {
                lighter += 1;
            }
        }
        (full, lighter)
    });
    let (full, lighter) = results.iter().fold((0, 0), |(f, l), r| (f + r.0, l + r.1));
    // Lighter charges are the exception
    println!("{full} path branches take a full charge, {lighter} a lighter one");
    assert!(lighter * 10 <= full, "{lighter} path branches need a lighter charge, {full} take a full one");
}

/// Snapping a sturdy path branch leaves a stub long enough that the level can still be climbed
/// without bouncing: whichever one snaps, as short as a snap can leave it, one charged leap from
/// the branch below still lands on the stub (or higher up the path), and one from the stub lands
/// on the next branch (or higher). As for the whole branches, a full charge does it nearly
/// everywhere, and a lighter one where a full leap would sail up onto something overhead.
#[test]
fn every_path_branch_is_one_leap_past_a_stub() {
    let results = each_level(0..4, |level, seed| {
        let base = Sim::new(level, seed);
        let (mut full, mut lighter) = (0, 0);
        for k in (0..base.path - 1).filter(|&k| !base.limbs[k].thin) {
            let mut sim = base.clone();
            sim.snap_short(k);
            let stub = &sim.limbs[k];
            assert!(stub.stub && stub.branch.broken_at == Some(stub_segments(stub.segments()).0));
            let up = leap_to(&sim, k.checked_sub(1), k, &CHARGES);
            assert!(up.is_some(), "level {level} seed {seed}: no leap from path branch {} reaches the stub of {k}", k as isize - 1);
            let on = leap_to(&sim, Some(k), k + 1, &CHARGES);
            assert!(on.is_some(), "level {level} seed {seed}: no leap from the stub of path branch {k} reaches {}", k + 1);
            for charge in [up, on] {
                if charge == Some(CHARGES[0]) {
                    full += 1;
                } else {
                    lighter += 1;
                }
            }
        }
        (full, lighter)
    });
    let (full, lighter) = results.iter().fold((0, 0), |(f, l), r| (f + r.0, l + r.1));
    println!("past stubs: {full} leaps take a full charge, {lighter} a lighter one");
    assert!(lighter * 10 <= full, "{lighter} leaps past stubs need a lighter charge, {full} take a full one");
}
