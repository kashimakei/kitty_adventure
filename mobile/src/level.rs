//! Level layout and difficulty, ported from the pygame game's level_utils.py. Each of the five
//! story levels is one stretch of forest three screens wide, with a friend to rescue at the top.

use crate::consts::*;
use crate::rng::Rng;

pub const STORY_LEVELS: usize = 5;

/// Everything that makes a level harder. Branch sizes stay natural at every level; the ratchet is
/// wider spacing, bendier wood, stronger wind, and more (and faster) enemies.
#[derive(Clone)]
pub struct Difficulty {
    pub gap_extra: f32,
    pub stiffness: f32,
    pub wind: f32,
    pub autumn: f32, // share of autumn leaves: the woods turn as Kitty climbs higher
    pub squirrels: usize,
    pub snakes: usize,
    pub dogs: bool,
    pub max_dogs: usize,
    pub dog_interval: f32,
    pub dog_speed: f32,
    pub crows: bool,
    pub max_crows: usize,
    pub crow_interval: (f32, f32),
    pub eagles: bool,
    pub max_eagles: usize,
    pub eagle_interval: (f32, f32),
    pub eagle_speed: f32,
}

/// `level` is 1..=5; each pass through the story after the first (`lap`) gets harder.
pub fn difficulty(level: usize, lap: usize) -> Difficulty {
    // The pygame game's ramp ran over ten levels; the story's five levels take every other step
    let old_level = (1 + (level - 1) * 2 + lap * 2).min(12) as f32;
    let climb = old_level - 1.0;
    let eagle_wait = (1.0 - 0.08 * (old_level - 4.0).max(0.0)).max(0.35);
    Difficulty {
        gap_extra: 2.0 * old_level * 10.0,
        stiffness: (0.95 - 0.035 * climb).max(0.5),
        wind: 3.0 + 0.6 * climb,
        autumn: (0.25 + 0.15 * (level - 1) as f32).min(0.9),
        squirrels: 1 + level + lap,
        snakes: if level >= 3 { level - 1 + lap } else { 0 },
        dogs: level >= 2 || lap > 0,
        max_dogs: (1 + (old_level as usize + 1) / 4).min(4),
        dog_interval: 5.0 * (1.0 - 0.07 * climb).max(0.4),
        dog_speed: 1.0 + 0.08 * climb,
        crows: level >= 4 || lap > 0,
        max_crows: if level >= 5 { 2 } else { 1 },
        crow_interval: (4.0 * eagle_wait.max(0.6), 8.0 * eagle_wait.max(0.6)),
        eagles: level >= 5 || lap > 0,
        max_eagles: if old_level >= 8.0 { 2 } else { 1 },
        eagle_interval: (5.0 * eagle_wait, 10.0 * eagle_wait),
        eagle_speed: 1.0 + 0.06 * (old_level - 4.0).max(0.0),
    }
}

/// x-centres of the trees: one hugging each edge of the world, the rest evenly spaced between.
pub fn tree_positions() -> Vec<f32> {
    let edge = TRUNK_WIDTH / 2.0 + 5.0;
    let span = WORLD_W - 2.0 * edge;
    let count = (span / TREE_SPACING).round() as usize + 1;
    (0..count).map(|i| edge + i as f32 * span / (count - 1) as f32).collect()
}

/// (tree index, direction) pairs: edge trees grow inward, the rest both ways.
fn branch_slots(trees: usize) -> Vec<(usize, f32)> {
    let mut slots = Vec::new();
    for i in 0..trees {
        if i > 0 {
            slots.push((i, -1.0));
        }
        if i + 1 < trees {
            slots.push((i, 1.0));
        }
    }
    slots
}

fn span(tree_xs: &[f32], (tree, dir): (usize, f32), length: f32) -> (f32, f32) {
    let x = tree_xs[tree];
    (x.min(x + dir * length), x.max(x + dir * length))
}

fn gap_between(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0.max(b.0) - a.1.min(b.1)).max(0.0)
}

pub struct BranchPlan {
    pub tree: usize,
    pub direction: f32,
    pub y: f32,
    pub length: f32,
    pub thickness: f32,
    pub stiffness: f32,
    pub gnarl: f32,
    pub strength: f32,
    pub on_path: bool,
    pub goal: bool,
}

pub struct ForkPlan {
    pub parent: usize,     // index into branches
    pub node_frac: f32,    // where along the parent it sprouts
    pub length: f32,
    pub rise: f32,
    pub thickness: f32,
    pub stiffness: f32,
    pub gnarl: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ItemKind {
    Fish,
    Gold,
    Heart,
}

/// A pickup resting on a limb: `limb` indexes branches then forks (forks follow the branches).
pub struct ItemPlan {
    pub kind: ItemKind,
    pub limb: usize,
    pub frac: f32,
}

pub struct LevelPlan {
    pub tree_xs: Vec<f32>,
    pub branches: Vec<BranchPlan>,
    pub forks: Vec<ForkPlan>,
    pub items: Vec<ItemPlan>,
    /// Limbs (same indexing as items) where squirrels and snakes start.
    pub squirrel_spots: Vec<(usize, f32)>,
    pub snake_spots: Vec<(usize, f32)>,
    pub top_y: f32,
}

pub fn generate(level: usize, lap: usize, seed: u64) -> LevelPlan {
    let mut rng = Rng::new(seed);
    let d = difficulty(level, lap);
    let tree_xs = tree_positions();
    let slots = branch_slots(tree_xs.len());
    // Branches stop short of the neighbouring trunk instead of poking through it
    let max_length = tree_xs[1] - tree_xs[0] - TRUNK_WIDTH / 2.0 - 25.0;
    let path_len = 10 + level.min(3);

    // Gaps first, so each path branch knows how far it is to the one above
    let gaps: Vec<f32> = (0..path_len)
        .map(|i| {
            let progress = i as f32 / path_len as f32;
            let lo = 120.0 + d.gap_extra * progress;
            let hi = 150.0 + d.gap_extra * progress;
            rng.range(lo, hi).min(MAX_BRANCH_GAP)
        })
        .collect();

    // The main path: every branch is within a jump of the one below, and it heads out to one
    // side screen, across to the other, then wherever
    let screen_centres: Vec<f32> = (0..3).map(|k| WORLD_W * (k as f32 + 0.5) / 3.0).collect();
    let first_side = if rng.chance(0.5) { screen_centres[0] } else { screen_centres[2] };
    let other_side = if first_side == screen_centres[0] { screen_centres[2] } else { screen_centres[0] };
    let mut targets = vec![first_side, other_side];
    for _ in 0..path_len {
        targets.push(*rng.pick(&screen_centres));
    }
    let mut prev_span = (WORLD_W / 2.0 - 50.0, WORLD_W / 2.0 + 50.0);
    let mut last_slot = None;
    let mut y = 10.0;
    let mut branches = Vec::new();
    for (i, &gap) in gaps.iter().enumerate() {
        y -= gap;
        let target = targets[i / 4];
        let length = rng.pair(BRANCH_LENGTH).min(max_length);
        let reachable: Vec<(usize, f32)> = slots.iter().copied()
            .filter(|&s| Some(s) != last_slot && gap_between(span(&tree_xs, s, length), prev_span) <= MAX_BRANCH_HOP)
            .collect();
        let slot = if rng.chance(0.7) {
            *reachable.iter().min_by(|a, b| {
                let ca = { let s = span(&tree_xs, **a, length); (s.0 + s.1) / 2.0 };
                let cb = { let s = span(&tree_xs, **b, length); (s.0 + s.1) / 2.0 };
                (ca - target).abs().total_cmp(&(cb - target).abs())
            }).unwrap()
        } else {
            *rng.pick(&reachable)
        };
        let is_last = i + 1 == path_len;
        // A thin branch can snap, so only make one where Kitty could climb past it without it
        let thin = !is_last && rng.chance(THIN_BRANCH_CHANCE) && gap + gaps[i + 1] <= MAX_BRANCH_GAP;
        branches.push(new_branch(&mut rng, &d, slot, y, length, thin, true, is_last));
        prev_span = span(&tree_xs, slot, length);
        last_slot = Some(slot);
    }

    // Fillers: extra branches wherever there's room, at heights along the path
    let top_path_y = branches.last().unwrap().y;
    for i in 0..path_len {
        for _ in 0..FILLERS_PER_PATH_BRANCH {
            let fy = (branches[i].y + rng.range(-70.0, 70.0)).clamp(top_path_y + 60.0, -120.0);
            let slot = *rng.pick(&slots);
            let length = rng.pair(BRANCH_LENGTH).min(max_length);
            let thin = rng.chance(THIN_BRANCH_CHANCE);
            let filler = new_branch(&mut rng, &d, slot, fy, length, thin, false, false);
            let room = branches.iter().all(|b: &BranchPlan| {
                gap_between(span(&tree_xs, (filler.tree, filler.direction), filler.length),
                            span(&tree_xs, (b.tree, b.direction), b.length)) >= 40.0
                    || (b.y - filler.y).abs() >= 120.0
            });
            if room {
                branches.push(filler);
            }
        }
    }

    // Forks sprout upward from sturdy branches, leaving room above for Kitty
    let mut forks = Vec::new();
    for (i, b) in branches.iter().enumerate() {
        if b.strength > 0.0 || b.goal || b.length < 180.0 || !rng.chance(FORK_CHANCE) {
            continue;
        }
        forks.push(ForkPlan {
            parent: i,
            node_frac: rng.range(0.45, 0.7),
            length: rng.pair(FORK_LENGTH),
            rise: rng.pair(FORK_RISE),
            thickness: b.thickness * 0.55,
            stiffness: (b.stiffness - 0.05).max(0.5),
            gnarl: rng.range(0.4, 0.8),
        });
    }

    // Pickups: fish along the path, golden fish on fork tips and risky thin branches, hearts
    // tucked on fillers
    let mut items = Vec::new();
    for (i, b) in branches.iter().enumerate() {
        if b.goal {
            continue;
        }
        // Fixed slots along the branch so pickups never pile up on each other
        let gold = b.strength > 0.0 && rng.chance(0.6);
        if b.on_path {
            let slots: &[f32] = if gold || rng.chance(0.5) { &[0.4] } else { &[0.35, 0.75] };
            for &slot in slots {
                items.push(ItemPlan { kind: ItemKind::Fish, limb: i, frac: slot + rng.range(-0.04, 0.04) });
            }
        }
        if gold {
            items.push(ItemPlan { kind: ItemKind::Gold, limb: i, frac: 0.88 });
        }
    }
    for (k, _) in forks.iter().enumerate() {
        items.push(ItemPlan { kind: ItemKind::Gold, limb: branches.len() + k, frac: 0.8 });
    }
    let mut fillers: Vec<usize> = (0..branches.len()).filter(|&i| !branches[i].on_path && branches[i].strength == 0.0).collect();
    for _ in 0..(1 + level / 3) {
        if !fillers.is_empty() {
            let limb = fillers.swap_remove(rng.int(0, fillers.len() as i32 - 1) as usize);
            items.push(ItemPlan { kind: ItemKind::Heart, limb, frac: rng.range(0.5, 0.85) });
        }
    }

    // Enemy perches: squirrels and snakes start on sturdy branches above the first third
    let low_cut = top_path_y / 3.0;
    let mut perches: Vec<usize> = (0..branches.len())
        .filter(|&i| branches[i].y < low_cut && !branches[i].goal && branches[i].strength == 0.0)
        .collect();
    let mut take = |rng: &mut Rng, n: usize| {
        let mut spots = Vec::new();
        for _ in 0..n {
            if perches.is_empty() {
                break;
            }
            let k = rng.int(0, perches.len() as i32 - 1) as usize;
            spots.push((perches.swap_remove(k), rng.range(0.45, 0.85)));
        }
        spots
    };
    let squirrel_spots = take(&mut rng, d.squirrels);
    let snake_spots = take(&mut rng, d.snakes);

    let top_y = branches.iter().map(|b| b.y).fold(0.0, f32::min) - 180.0;
    LevelPlan { tree_xs, branches, forks, items, squirrel_spots, snake_spots, top_y }
}

#[allow(clippy::too_many_arguments)]
fn new_branch(rng: &mut Rng, d: &Difficulty, (tree, direction): (usize, f32), y: f32, length: f32,
              thin: bool, on_path: bool, goal: bool) -> BranchPlan {
    let mut stiffness = d.stiffness + rng.range(-0.03, 0.03);
    if thin {
        // Thin wood already bends a lot, so it only gets half the extra floppiness
        stiffness = 1.0 - (1.0 - stiffness) * 0.5;
    }
    BranchPlan {
        tree,
        direction,
        y,
        length,
        thickness: rng.pair(if thin { BRANCH_THIN_DEPTH } else { BRANCH_STURDY_DEPTH }),
        stiffness,
        gnarl: rng.range(0.4, 1.0),
        strength: if thin { rng.pair(THIN_BRANCH_STRENGTH) * KITTY_WEIGHT } else { 0.0 },
        on_path,
        goal,
    }
}
