//! Every level can be climbed, whatever its seed (the game picks one at random each time a level
//! starts). A search over the real physics, stepped the way World does it (through `step_kitty`
//! and `step_limb`, without the graphics): from the ground, and from a few standing spots on each
//! limb Kitty gets to, she tries charged leaps of a few strengths steered across each limb within
//! reach, walks off either end, and drops through. Wherever she ends up standing (or hanging, and
//! then climbing up) she can get to. A level can be climbed if that gets her to the goal, as
//! generated, with every sturdy path branch snapped as short as a snap can leave it, with every
//! thin one snapped off, and with both, all at once.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use kitty_mobile::consts::*;
use kitty_mobile::input::Controls;
use kitty_mobile::kitty::{Kitty, Support};
use kitty_mobile::level;
use kitty_mobile::limb::Limb;
use kitty_mobile::rng::Rng;
use kitty_mobile::world::{breeze_at, build_limbs, step_kitty, step_limb};
use macroquad::prelude::*;

/// Standing spots tried along each limb, from the trunk out.
const SPOTS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
/// Steps of crouching: a full charge, then lighter ones for where a full leap would sail up onto a
/// branch overhead.
const CHARGES: [u32; 5] = [CHARGE_STEPS + 1, 22, 16, 12, 8];
/// Where along the limb she's leaping for she steers to, besides the nearest end of it.
const AIMS: [f32; 3] = [0.15, 0.5, 0.85];
/// The quick pass tries only the likeliest leaps, from this many of the spots nearest the target,
/// with these charges and aims (the nearest end and the middle), when the wind turns one way.
/// Where that doesn't get her to the goal, the thorough pass tries every one, at both turns.
const QUICK_SPOTS: usize = 2;
const QUICK_CHARGES: [u32; 2] = [CHARGE_STEPS + 1, 16];
/// Limbs out of reach aren't tried at all: higher than a full leap (about 480 px) gets her raised
/// paws, or further below or across than these.
const REACH_UP: f32 = 480.0 + KITTY_BOX.1 - GRAB_REACH;
const REACH_DOWN: f32 = 350.0;
const REACH_ACROSS: f32 = 420.0;
/// Steps standing on a spot before charging, a leap's longest flight, and how long she has to
/// stay where she landed for it to count.
const SETTLE_STEPS: usize = 40;
const FLIGHT_STEPS: usize = 300;
const STAY_STEPS: usize = 30;
/// A leap that drops this far below both where it started and the limb it's for has missed.
const MISSED: f32 = 200.0;
/// A leap's flight, for finding when the wind turns.
const LEAP_STEPS: usize = 75;
/// Limbs further than this from Kitty (or from where she's headed this step) aren't stepped.
const NEAR: f32 = 40.0;
/// Steps the stubs get to settle (and the pieces that broke off to fall away) after snapping.
const SNAP_SETTLE: usize = 120;

fn press() -> Controls {
    Controls { press: true, charge_held: true, ..Controls::default() }
}

fn hold() -> Controls {
    Controls { charge_held: true, ..Controls::default() }
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

/// Whether limbs `a` and `b` are one piece of wood: the same limb, or a fork and its parent.
fn joined(limbs: &[Limb], a: usize, b: usize) -> bool {
    let parent = |id: usize| limbs[id].attachment.map(|attached| attached.parent);
    a == b || parent(a) == Some(b) || parent(b) == Some(a)
}

/// The top of the wood still on the tree, (left, right, highest, lowest), or None if there's
/// none left. A piece that snapped off falls away out of play.
fn wood(limb: &Limb) -> Option<(f32, f32, f32, f32)> {
    if limb.gone() {
        return None;
    }
    let end = limb.branch.broken_at.unwrap_or(limb.segments());
    Some((0..=end).map(|i| limb.top_point(i)).fold((f32::MAX, f32::MIN, f32::MAX, f32::MIN), |(l, r, t, b), p| {
        (l.min(p.x), r.max(p.x), t.min(p.y), b.max(p.y))
    }))
}

/// Which of the path's branches are snapped before the search, all at once: none, the sturdy ones
/// (as short as a snap can leave them), the thin ones (which leave nothing), or both.
#[derive(Clone, Copy, PartialEq)]
enum Snapped {
    None,
    Sturdy,
    Thin,
    Both,
}

impl Snapped {
    const ALL: [Snapped; 4] = [Snapped::None, Snapped::Sturdy, Snapped::Thin, Snapped::Both];

    fn snaps(self, limb: &Limb) -> bool {
        limb.breakable() && match self {
            Snapped::None => false,
            Snapped::Sturdy => !limb.thin,
            Snapped::Thin => limb.thin,
            Snapped::Both => true,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Snapped::None => "as generated",
            Snapped::Sturdy => "with the path's sturdy branches snapped",
            Snapped::Thin => "with its thin branches snapped",
            Snapped::Both => "with both snapped",
        }
    }
}

/// A level to search: its limbs (settled, and maybe with some of the path's branches snapped),
/// and which are the goal and the path.
struct Level {
    limbs: Vec<Limb>,
    goal: usize,
    path: usize,
    wind: f32,
}

impl Level {
    fn new(level: usize, lap: usize, seed: u64, snapped: Snapped) -> Self {
        let plan = level::generate(level, lap, seed);
        let mut rng = Rng::new(seed ^ 0xC0FFEE);
        let mut limbs = build_limbs(&plan, &mut rng);
        let path = plan.branches.iter().take_while(|b| b.on_path).count();
        if snapped != Snapped::None {
            for limb in limbs.iter_mut().take(path).filter(|l| snapped.snaps(l)) {
                let base = limb.base().x;
                limb.snap_at(base);
            }
            for _ in 0..SNAP_SETTLE {
                for i in 0..limbs.len() {
                    step_limb(&mut limbs, i, 0.0, 0.0, &mut rng);
                }
            }
        }
        let goal = limbs.iter().position(|l| l.is_goal).unwrap();
        Self { limbs, goal, path, wind: level::difficulty(level, lap).wind }
    }
}

/// Two moments in the wind's cycle to leap at: as it turns rightward and as it turns leftward,
/// so over a leap it pushes her neither way on the whole. A player can wait for those; gusts
/// either way can help or hinder.
fn turns(wind: f32) -> [f32; 2] {
    let mean = |t: f32| (0..LEAP_STEPS).map(|k| breeze_at(t + k as f32 * STEP, wind)).sum::<f32>() / LEAP_STEPS as f32;
    let (mut rightward, mut leftward) = (None, None);
    for k in 0..1300 {
        let (t, next) = (k as f32 * 0.01, (k + 1) as f32 * 0.01);
        if mean(t) <= 0.0 && mean(next) > 0.0 {
            rightward = rightward.or(Some(next));
        }
        if mean(t) >= 0.0 && mean(next) < 0.0 {
            leftward = leftward.or(Some(next));
        }
    }
    [rightward.unwrap(), leftward.unwrap()]
}

#[derive(Clone)]
struct Sim {
    limbs: Vec<Limb>,
    kitty: Kitty,
    rng: Rng,
    time: f32,
    wind: f32,
}

impl Sim {
    /// As World steps, but only for the limbs Kitty is on or close to: the rest stay as they
    /// settled (that leaves out the few px the wind sways them), which keeps the search quick.
    fn step(&mut self, controls: Controls) {
        self.time += STEP;
        let breeze = breeze_at(self.time, self.wind);
        step_kitty(&mut self.kitty, &mut self.limbs, &controls, breeze);
        let on = match self.kitty.standing_on {
            Some(Support::Limb(id)) => Some(id),
            _ => self.kitty.hanging_on.or(self.kitty.climbing_on),
        };
        let r = self.kitty.rect();
        let reach = NEAR + self.kitty.velocity.abs();
        let near = Rect::new(r.x - reach, r.y - reach, r.w + 2.0 * reach, r.h + 2.0 * reach + self.kitty.velocity.max(0.0));
        for i in 0..self.limbs.len() {
            let close = wood(&self.limbs[i]).is_some_and(|(l, r, t, b)| {
                near.overlaps(&Rect::new(l, t, r - l, b - t + self.limbs[i].thickness))
            });
            if close || on.is_some_and(|on| joined(&self.limbs, i, on)) {
                step_limb(&mut self.limbs, i, breeze, self.time, &mut self.rng);
            }
        }
    }

    fn run(&mut self, steps: usize, controls: Controls) {
        for _ in 0..steps {
            self.step(controls);
        }
    }
}

/// Fly on, steering for `aim`, until something catches her. Returns the limb she ends up
/// standing on, still there a moment later, if she landed on one, or caught one and climbed up;
/// None for the ground, or once she's fallen below `floor`.
fn fly(sim: &mut Sim, aim: Option<f32>, floor: f32) -> Option<usize> {
    for _ in 0..FLIGHT_STEPS {
        if !sim.kitty.airborne() {
            break;
        }
        if sim.kitty.y > floor {
            return None;
        }
        let move_x = aim.map_or(0.0, |x| steer(&sim.kitty, x));
        sim.step(walk(move_x));
    }
    if sim.kitty.hanging_on.is_some() {
        sim.step(tap());
        sim.run(CLIMB_STEPS as usize + 5, Controls::default());
    }
    let Some(Support::Limb(id)) = sim.kitty.standing_on else { return None };
    sim.run(STAY_STEPS, Controls::default());
    (sim.kitty.standing_on == Some(Support::Limb(id))).then_some(id)
}

/// Whether a leap could get her onto (or hanging from) `limb` at all, steered as well as can be:
/// her flight from here with nothing to catch her raises her paws above its lowest point, and she
/// can be over it (at full speed with the wind) by the last moment they're still above it.
fn within_reach(sim: &Sim, limb: &Limb, floor: f32) -> bool {
    let Some((l, r, _, lowest)) = wood(limb) else { return false };
    let mut kitty = sim.kitty.clone();
    let (mut time, mut left, mut right) = (sim.time, kitty.x, kitty.x);
    let paws = KITTY_BOX.1 - GRAB_REACH;
    let mut over = false;
    for _ in 0..FLIGHT_STEPS {
        if !kitty.airborne() || kitty.y > floor {
            break;
        }
        time += STEP;
        let x = kitty.x;
        step_kitty(&mut kitty, &mut [], &Controls::default(), breeze_at(time, sim.wind));
        let drift = kitty.x - x;
        left += drift - KITTY_SPEED;
        right += drift + KITTY_SPEED;
        if kitty.y - paws < lowest {
            over = left - KITTY_FOOT_REACH <= r && right + KITTY_FOOT_REACH >= l;
        }
    }
    over
}

/// Searching one level: where Kitty has got to so far, and Kitty settled on each spot tried.
struct Search<'a> {
    level: &'a Level,
    turns: [f32; 2],
    reached: Vec<bool>,
    settled: HashMap<(usize, usize), Option<Sim>>,
    leaps: usize,
}

/// How a search went: whether she got to the goal, the highest path branch she got to, and how
/// many leaps it tried.
struct Climb {
    goal: bool,
    highest: Option<usize>,
    leaps: usize,
}

impl<'a> Search<'a> {
    fn new(level: &'a Level) -> Self {
        Self { level, turns: turns(level.wind), reached: vec![false; level.limbs.len()], settled: HashMap::new(), leaps: 0 }
    }

    fn sim(&self, x: f32) -> Sim {
        Sim { limbs: self.level.limbs.clone(), kitty: Kitty::new(x), rng: Rng::new(1), time: 0.0, wind: self.level.wind }
    }

    /// Kitty standing at spot `k` on `limb`, settled (worked out the first time it's needed).
    fn settled(&mut self, limb: usize, k: usize) -> Option<Sim> {
        if !self.settled.contains_key(&(limb, k)) {
            let mut sim = self.sim(WORLD_W / 2.0);
            let x = along(&sim.limbs[limb], SPOTS[k]);
            let settled = sim.limbs[limb].surface_y(x).and_then(|s| {
                sim.kitty.x = x;
                sim.kitty.y = s;
                sim.kitty.standing_on = Some(Support::Limb(limb));
                sim.run(SETTLE_STEPS, Controls::default());
                (sim.kitty.standing_on == Some(Support::Limb(limb))).then_some(sim)
            });
            self.settled.insert((limb, k), settled);
        }
        self.settled[&(limb, k)].clone()
    }

    /// The highest and lowest she can stand on a place, and its extent.
    fn place(&self, from: Option<usize>) -> (f32, f32, f32, f32) {
        match from {
            None => (0.0, WORLD_W, GROUND_Y, GROUND_Y),
            Some(id) => wood(&self.level.limbs[id]).unwrap(),
        }
    }

    /// Limbs she hasn't got to yet that a leap from `from` might reach, nearest in height first.
    fn targets(&self, from: Option<usize>) -> Vec<usize> {
        let (fl, fr, ft, fb) = self.place(from);
        let mut targets: Vec<(usize, f32)> = (0..self.level.limbs.len())
            .filter(|&id| !self.reached[id] && Some(id) != from)
            .filter_map(|id| wood(&self.level.limbs[id]).map(|w| (id, w)))
            .filter(|&(_, (l, r, t, b))| {
                let across = (l.max(fl) - r.min(fr)).max(0.0);
                ft - b <= REACH_UP && t - fb <= REACH_DOWN && across <= REACH_ACROSS
            })
            .map(|(id, (_, _, t, b))| (id, ((t + b) / 2.0 - (ft + fb) / 2.0).abs()))
            .collect();
        targets.sort_by(|a, b| a.1.total_cmp(&b.1));
        targets.into_iter().map(|(id, _)| id).collect()
    }

    /// Where she can stand to leap for `to` from `from`, nearest first.
    fn starts(&mut self, from: Option<usize>, to: usize, thorough: bool) -> Vec<Sim> {
        let (l, r, _, _) = wood(&self.level.limbs[to]).unwrap();
        let count = if thorough { SPOTS.len() } else { QUICK_SPOTS };
        match from {
            // On the ground she can stand anywhere: under it, and off either end
            None => [(l + r) / 2.0, l + 30.0, r - 30.0, l - 80.0, r + 80.0]
                .into_iter()
                .take(count)
                .map(|x| self.sim(x.clamp(KITTY_BOX.0 / 2.0, WORLD_W - KITTY_BOX.0 / 2.0)))
                .collect(),
            Some(id) => {
                let distance = |x: f32| (l - x).max(x - r).max(0.0);
                let mut spots: Vec<usize> = (0..SPOTS.len()).collect();
                let limb = &self.level.limbs[id];
                spots.sort_by(|&a, &b| distance(along(limb, SPOTS[a])).total_cmp(&distance(along(limb, SPOTS[b]))));
                spots.into_iter().take(count).filter_map(|k| self.settled(id, k)).collect()
            }
        }
    }

    /// Charged leaps from `from` steered for `to` (stopping once she's got there); anywhere she
    /// lands counts.
    fn leap_for(&mut self, from: Option<usize>, to: usize, thorough: bool) {
        let target = self.level.limbs[to].clone();
        let (_, _, _, lowest) = wood(&target).unwrap();
        let turns = if thorough { self.turns.to_vec() } else { vec![self.turns[0]] };
        let charges = if thorough { CHARGES.to_vec() } else { QUICK_CHARGES.to_vec() };
        for start in self.starts(from, to, thorough) {
            let floor = start.kitty.y.max(lowest) + MISSED;
            for &turn in &turns {
                for &charge in &charges {
                    let mut charged = start.clone();
                    charged.time = turn - charge as f32 * STEP;
                    charged.step(press());
                    charged.run(charge as usize - 1, hold());
                    let mut probe = charged.clone();
                    probe.step(Controls { leap: true, ..Controls::default() });
                    if !within_reach(&probe, &target, floor) {
                        continue;
                    }
                    let (lo, hi) = target.walk_range();
                    let nearest = probe.kitty.x.clamp(lo + 20.0, hi - 20.0);
                    let mut aims = vec![nearest];
                    for &f in if thorough { &AIMS[..] } else { &AIMS[1..2] } {
                        let aim = along(&target, f);
                        if aims.iter().all(|a| (a - aim).abs() > 15.0) {
                            aims.push(aim);
                        }
                    }
                    for aim in aims {
                        let mut leap = charged.clone();
                        leap.step(Controls { leap: true, move_x: steer(&leap.kitty, aim), ..Controls::default() });
                        self.leaps += 1;
                        if let Some(on) = fly(&mut leap, Some(aim), floor) {
                            self.reached[on] = true;
                        }
                        if self.reached[to] {
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Off `from` the other ways: walking off either end at full speed, and dropping through.
    fn step_off(&mut self, from: usize) {
        let direction = self.level.limbs[from].direction;
        for (k, dir) in [(SPOTS.len() - 1, direction), (0, -direction)] {
            let Some(mut sim) = self.settled(from, k) else { continue };
            for _ in 0..120 {
                if sim.kitty.airborne() {
                    break;
                }
                sim.step(walk(dir));
            }
            let far = sim.kitty.x + dir * WORLD_W;
            if let Some(on) = fly(&mut sim, Some(far), f32::INFINITY) {
                self.reached[on] = true;
            }
        }
        for k in 1..SPOTS.len() - 1 {
            let Some(mut sim) = self.settled(from, k) else { continue };
            sim.step(Controls { drop: true, ..Controls::default() });
            if let Some(on) = fly(&mut sim, None, f32::INFINITY) {
                self.reached[on] = true;
            }
        }
    }

    /// Search from the ground up, lowest place first: the quick pass, then if that doesn't get
    /// her to the goal, the thorough one.
    fn climb(mut self) -> Climb {
        let n = self.level.limbs.len();
        let height = |limbs: &[Limb], id: usize| wood(&limbs[id]).map_or(f32::MIN, |(_, _, t, b)| (t + b) / 2.0);
        'passes: for thorough in [false, true] {
            let mut searched = vec![false; n];
            let mut from = None;
            loop {
                for to in self.targets(from) {
                    if !self.reached[to] {
                        self.leap_for(from, to, thorough);
                    }
                    if self.reached[self.level.goal] {
                        break 'passes;
                    }
                }
                if let (Some(id), false) = (from, thorough) {
                    self.step_off(id);
                }
                let limbs = &self.level.limbs;
                let next = (0..n).filter(|&id| self.reached[id] && !searched[id]).max_by(|&a, &b| height(limbs, a).total_cmp(&height(limbs, b)));
                let Some(next) = next else { break };
                searched[next] = true;
                from = Some(next);
            }
        }
        let highest = (0..self.level.path).rev().find(|&k| self.reached[k]);
        Climb { goal: self.reached[self.level.goal], highest, leaps: self.leaps }
    }
}

/// Search levels 1-5, laps 0-1 and `seeds`, each with the path's branches snapped as each of
/// `configs` says, on every core (but not a level with no thin path branches with them snapped,
/// which would be the same search again). Returns the levels she can't climb, for each of
/// `Snapped::ALL`.
fn sweep(seeds: &[u64], configs: &[Snapped]) -> [Vec<String>; 4] {
    let has_thin = |level: usize, lap: usize, seed: u64| {
        level::generate(level, lap, seed).branches.iter().take_while(|b| b.on_path).any(|b| b.strength > 0.0)
    };
    let cases: Vec<(usize, usize, u64, usize)> = (1..=5)
        .flat_map(|level| (0..2).flat_map(move |lap| seeds.iter().flat_map(move |&seed| (0..Snapped::ALL.len()).map(move |k| (level, lap, seed, k)))))
        .filter(|&(level, lap, seed, k)| {
            let snapped = Snapped::ALL[k];
            configs.contains(&snapped) && (!matches!(snapped, Snapped::Thin | Snapped::Both) || has_thin(level, lap, seed))
        })
        .collect();
    let next = AtomicUsize::new(0);
    let started = Instant::now();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let results: Vec<Vec<(usize, bool, String)>> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..threads).map(|_| scope.spawn(|| {
            let mut results = Vec::new();
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(level, lap, seed, k)) = cases.get(i) else { break };
                let t = Instant::now();
                let snapped = Snapped::ALL[k];
                let plan = Level::new(level, lap, seed, snapped);
                let climb = Search::new(&plan).climb();
                let name = format!("L{level} lap {lap} seed {seed}");
                if !climb.goal {
                    println!("{name} {}: can't climb past path branch {:?} of {} ({} leaps, {:.1} s)",
                             snapped.name(), climb.highest, plan.path, climb.leaps, t.elapsed().as_secs_f32());
                }
                results.push((i, climb.goal, name));
            }
            results
        })).collect();
        runs.into_iter().map(|run| run.join().unwrap()).collect()
    });
    let mut failed: [Vec<String>; 4] = Default::default();
    for (i, climbed, name) in results.into_iter().flatten() {
        if !climbed {
            failed[cases[i].3].push(name);
        }
    }
    let counts: Vec<String> = (0..Snapped::ALL.len()).filter(|&k| configs.contains(&Snapped::ALL[k])).map(|k| {
        let total = cases.iter().filter(|case| case.3 == k).count();
        format!("{} of {total} {}", failed[k].len(), Snapped::ALL[k].name())
    }).collect();
    println!("{} seeds, {:.0} s: can't be climbed: {}", seeds.len(), started.elapsed().as_secs_f32(), counts.join(", "));
    failed
}

fn assert_climbable(failed: &[Vec<String>; 4]) {
    let failures: Vec<String> = Snapped::ALL.iter().zip(failed).filter(|(_, f)| !f.is_empty()).map(|(s, f)| format!("{} {f:?}", s.name())).collect();
    assert!(failures.is_empty(), "can't be climbed: {}", failures.join("; "));
}

/// Every level can be climbed on a handful of seeds (ones where a filler overhead catches the
/// leaps for the first path branch, or a single leap doesn't get past a stub), as generated, with
/// the path's sturdy branches snapped short, and with its thin ones snapped off as well.
#[test]
fn every_level_can_be_climbed() {
    assert_climbable(&sweep(&[4, 7, 8, 10], &[Snapped::None, Snapped::Sturdy, Snapped::Both]));
}

/// The same over many more seeds, and with only the thin ones snapped too:
/// `cargo test --release --test climbable -- --ignored --nocapture`.
#[test]
#[ignore]
fn every_level_can_be_climbed_whatever_the_seed() {
    let seeds: Vec<u64> = (0..48).collect();
    assert_climbable(&sweep(&seeds, &Snapped::ALL));
}
