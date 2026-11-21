//! A tree limb: one bendy branch from the shared engine, dressed with leaf clusters and twigs.
//! A limb can grow off another limb (a fork); its base then rides along as the parent bends,
//! and weight on it also presses on the parent.

use branch_engine::branch::Branch;
use branch_engine::math::Vec2 as EngineVec2;
use macroquad::prelude::*;

use crate::consts::*;
use crate::rng::Rng;
use crate::sprites::Assets;

pub type LimbId = usize;

/// Where a fork grows from: the parent limb and the top node it sprouts at.
#[derive(Clone, Copy)]
pub struct Attachment {
    pub parent: LimbId,
    pub node: usize,
}

/// A leaf cluster or twig drawn on a limb, positioned relative to one of its top nodes.
struct Decoration {
    sprite: &'static str,
    node: usize,
    offset: Vec2,
    scale: f32,
    rotation: f32,
    front: bool,
    sway_phase: f32,
}

pub struct LimbSpec {
    pub start: Vec2,
    pub direction: f32,
    pub length: f32,
    pub thickness: f32,
    pub stiffness: f32,
    pub gnarl: f32,
    pub rise: f32,
    /// Total weight it holds before snapping; 0 never snaps.
    pub strength: f32,
    pub tree: usize,
    pub trunk_half: f32,
    pub attachment: Option<Attachment>,
    pub seed: u64,
}

pub struct Limb {
    pub branch: Branch,
    pub direction: f32,
    pub length: f32,
    pub thickness: f32,
    pub tree: usize,
    pub trunk_half: f32,
    pub attachment: Option<Attachment>,
    /// Where the parent's attach node was after the last step, so the base can follow it.
    anchor: Vec2,
    decorations: Vec<Decoration>,
    wind_phase: f32,
    pub just_snapped: bool,
    /// Kitty's friend waits in a nest at the tip of the goal limb.
    pub is_goal: bool,
    /// The last tip height, to tell how fast the limb is springing up or down.
    last_surface: Vec<f32>,
}

const FOLIAGE_GREEN: [&str; 6] = ["green_0", "green_1", "green_2", "green_3", "green_4", "green_5"];
const FOLIAGE_AUTUMN: [&str; 6] = ["autumn_0", "autumn_1", "autumn_2", "autumn_3", "autumn_4", "autumn_5"];
const TWIGS: [&str; 4] = ["twig_0", "twig_1", "twig_2", "twig_3"];

impl Limb {
    pub fn new(spec: &LimbSpec) -> Self {
        let segments = ((spec.length / BRANCH_SEGMENT_LEN).round() as usize).max(8);
        let mut branch = Branch::new_tree_branch(
            EngineVec2::new(spec.start.x, spec.start.y), spec.length, spec.thickness, segments,
            spec.stiffness, spec.gnarl, spec.direction, spec.seed, spec.rise,
        );
        if spec.strength > 0.0 {
            // Breaks right where it leaves the bark
            branch.strength = spec.strength;
            branch.snap_distance = spec.trunk_half;
        }
        let mut limb = Self {
            branch,
            direction: spec.direction,
            length: spec.length,
            thickness: spec.thickness,
            tree: spec.tree,
            trunk_half: spec.trunk_half,
            attachment: spec.attachment,
            anchor: spec.start,
            decorations: Vec::new(),
            wind_phase: (spec.seed % 628) as f32 / 100.0,
            just_snapped: false,
            is_goal: false,
            last_surface: Vec::new(),
        };
        limb.remember_surface();
        limb
    }

    /// Let the limb sag under its own weight, then freeze it there so levels start still.
    pub fn settle(&mut self) {
        for _ in 0..120 {
            self.branch.update_physics(BRANCH_SUB_STEPS, STEP, BRANCH_GRAVITY, BRANCH_DAMPING);
        }
        self.branch.rest();
        self.remember_surface();
    }

    pub fn breakable(&self) -> bool {
        self.branch.strength > 0.0
    }

    pub fn broken(&self) -> bool {
        self.branch.broken_at.is_some()
    }

    /// 0 when unloaded, 1 at the point of snapping.
    pub fn strain(&self) -> f32 {
        if self.breakable() { self.branch.stress / self.branch.strength } else { 0.0 }
    }

    /// For forks: where the parent's attach node is, so the first step only moves the base by
    /// how far the parent has moved since (the base itself sits a little sunk into the wood).
    pub fn track_anchor(&mut self, anchor: Vec2) {
        self.anchor = anchor;
    }

    /// One physics step. `parent_anchor` is where the parent's attach node is now, for forks.
    pub fn step(&mut self, breeze: f32, time: f32, parent_anchor: Option<Vec2>, rng: &mut Rng) {
        self.remember_surface();
        if let Some(anchor) = parent_anchor {
            let delta = anchor - self.anchor;
            self.branch.move_pinned(delta.x, delta.y);
            self.anchor = anchor;
        }
        let mut gust = breeze * BRANCH_WIND * (time * 2.0 + self.wind_phase).sin();
        // An overloaded limb trembles before it gives way
        gust += rng.range(-1.0, 1.0) * BRANCH_TREMBLE * (self.strain() - 0.5).max(0.0);
        self.branch.apply_force(0.0, gust);
        let was_broken = self.broken();
        self.branch.update_physics(BRANCH_SUB_STEPS, STEP, BRANCH_GRAVITY, BRANCH_DAMPING);
        self.just_snapped = self.broken() && !was_broken;
    }

    fn remember_surface(&mut self) {
        self.last_surface.clear();
        self.last_surface.extend(self.branch.nodes[..=self.branch.segments].iter().map(|n| n.pos.y));
    }

    /// How far the top surface under `x` moved down during the last step (negative = springing up).
    pub fn surface_velocity(&self, x: f32) -> f32 {
        for i in 0..self.branch.segments {
            let (a, b) = (self.top_point(i), self.top_point(i + 1));
            if a.x != b.x && (x - a.x) * (x - b.x) <= 0.0 {
                let t = (x - a.x) / (b.x - a.x);
                let now = a.y + (b.y - a.y) * t;
                let before = self.last_surface[i] + (self.last_surface[i + 1] - self.last_surface[i]) * t;
                return now - before;
            }
        }
        0.0
    }

    pub fn surface_y(&self, x: f32) -> Option<f32> {
        self.branch.surface_y(x)
    }

    /// Highest point of the top surface under any of `xs`.
    pub fn surface_under(&self, xs: &[f32]) -> Option<f32> {
        xs.iter().filter_map(|&x| self.branch.surface_y(x)).reduce(f32::min)
    }

    pub fn kick(&mut self, x: f32, speed: f32) {
        self.branch.apply_impulse(x, speed);
    }

    pub fn top_point(&self, i: usize) -> Vec2 {
        let p = self.branch.nodes[i].pos;
        vec2(p.x, p.y)
    }

    fn bottom_point(&self, i: usize) -> Vec2 {
        let p = self.branch.nodes[i + self.branch.segments + 1].pos;
        vec2(p.x, p.y)
    }

    pub fn base(&self) -> Vec2 {
        self.top_point(0)
    }

    pub fn tip(&self) -> Vec2 {
        self.top_point(self.branch.segments)
    }

    pub fn segments(&self) -> usize {
        self.branch.segments
    }

    /// x-extent something can walk along: from the trunk's bark to just short of the tip.
    pub fn walk_range(&self) -> (f32, f32) {
        let a = self.base().x + self.direction * (self.trunk_half + 10.0);
        let b = self.tip().x - self.direction * 10.0;
        (a.min(b), a.max(b))
    }

    pub fn bounds(&self) -> Rect {
        let (x0, y0, x1, y1) = self.branch.bounds();
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Scatter leaf clusters and twigs along the limb. `autumn` is the share of autumn leaves.
    pub fn decorate(&mut self, rng: &mut Rng, autumn: f32) {
        let segments = self.segments();
        let thin = self.breakable();
        let pick = |rng: &mut Rng, sizes: std::ops::RangeInclusive<usize>| {
            let size = rng.int(*sizes.start() as i32, *sizes.end() as i32) as usize;
            if rng.chance(autumn) { FOLIAGE_AUTUMN[size] } else { FOLIAGE_GREEN[size] }
        };
        // A bushy clump at the tip: a big tuft with a smaller one tucked behind and one hanging
        // below, so the end of the limb reads as leafy rather than a single sticker
        let tip_size = if thin { 1..=2 } else if self.attachment.is_some() { 2..=4 } else { 3..=5 };
        let clump = [
            (vec2(-14.0, -16.0), 0.75, false),
            (vec2(8.0, -10.0), 1.0, true),
            (vec2(-4.0, self.thickness * 0.45), 0.7, true),
        ];
        for (offset, scale, front) in clump {
            let size = pick(rng, tip_size.clone());
            self.decorations.push(Decoration {
                sprite: size, node: segments, offset: offset + vec2(rng.range(-4.0, 4.0), rng.range(-3.0, 3.0)),
                scale: scale * rng.range(0.85, 1.1), rotation: rng.range(-0.2, 0.2), front,
                sway_phase: rng.range(0.0, 6.3),
            });
        }
        // Tufts along the outer part, alternating above and below; the ones above sit behind the
        // wood so the walkway stays clear
        let start = (segments as f32 * if thin { 0.5 } else { 0.3 }) as usize;
        let mut node = start.max(2);
        let mut above = rng.chance(0.5);
        while node < segments {
            let sprite = pick(rng, 0..=if thin { 1 } else { 3 });
            self.decorations.push(Decoration {
                sprite, node, offset: vec2(rng.range(-6.0, 6.0), if above { -6.0 } else { self.thickness * 0.5 }),
                scale: rng.range(0.7, 1.0), rotation: rng.range(-0.3, 0.3), front: !above && rng.chance(0.5),
                sway_phase: rng.range(0.0, 6.3),
            });
            above = !above || rng.chance(0.3);
            node += rng.int(1, 2) as usize;
        }
        // A few sprigs nearer the trunk
        for _ in 0..rng.int(1, 2) {
            let node = rng.int(1, start.max(2) as i32) as usize;
            self.decorations.push(Decoration {
                sprite: pick(rng, 0..=1), node, offset: vec2(0.0, -5.0), scale: rng.range(0.6, 0.85),
                rotation: rng.range(-0.4, 0.4), front: false, sway_phase: rng.range(0.0, 6.3),
            });
        }
        // A twig or two poking out of sturdy limbs
        if !thin {
            for _ in 0..rng.int(1, 2) {
                let node = rng.int((segments / 3) as i32, (segments * 3 / 4) as i32) as usize;
                self.decorations.push(Decoration {
                    sprite: *rng.pick(&TWIGS), node, offset: vec2(0.0, 2.0), scale: rng.range(0.55, 0.75),
                    rotation: rng.range(-0.9, -0.4), front: false, sway_phase: rng.range(0.0, 6.3),
                });
            }
        }
    }

    /// The limb's slope at top node `i`, as an angle in its own frame: x along the limb from base
    /// to tip, so a level limb is 0 whichever way it grows.
    fn angle_at(&self, i: usize) -> f32 {
        let n = self.segments();
        let (a, b) = if i >= n { (self.top_point(n - 1), self.top_point(n)) } else { (self.top_point(i), self.top_point(i + 1)) };
        let d = b - a;
        d.y.atan2(d.x * self.direction)
    }

    fn draw_decorations(&self, assets: &Assets, front: bool, time: f32, breeze: f32) {
        for d in self.decorations.iter().filter(|d| d.front == front) {
            // Offsets and rotations are in the limb's own frame (x toward the tip); mirror them
            // for limbs growing left, which also draw their sprites flipped
            let angle = self.angle_at(d.node);
            let sway = (time * 2.2 + d.sway_phase).sin() * 0.04 * (1.0 + breeze.abs() * 0.15);
            let local = Vec2::from_angle(angle).rotate(d.offset);
            let offset = vec2(local.x * self.direction, local.y);
            let rotation = (angle + d.rotation + sway) * self.direction;
            assets.foliage.draw_ex(d.sprite, self.top_point(d.node) + offset, self.direction, d.scale, rotation, WHITE);
        }
    }

    pub fn draw_back(&self, assets: &Assets, time: f32, breeze: f32) {
        self.draw_decorations(assets, false, time, breeze);
    }

    pub fn draw_front(&self, assets: &Assets, time: f32, breeze: f32) {
        self.draw_decorations(assets, true, time, breeze);
    }

    /// The wood: a filled outline of the truss, with a sunlit ridge along the walkable top.
    pub fn draw(&self) {
        let segments = self.segments();
        let fill = if self.breakable() { BARK_YOUNG } else { BARK };
        let gap = self.branch.broken_at;
        for i in 0..segments {
            if gap == Some(i) {
                continue;
            }
            let (t0, t1, b0, b1) = (self.top_point(i), self.top_point(i + 1), self.bottom_point(i), self.bottom_point(i + 1));
            draw_triangle(t0, t1, b1, fill);
            draw_triangle(t0, b1, b0, fill);
        }
        for i in 0..segments {
            if gap == Some(i) {
                continue;
            }
            let (t0, t1, b0, b1) = (self.top_point(i), self.top_point(i + 1), self.bottom_point(i), self.bottom_point(i + 1));
            draw_line(t0.x, t0.y, t1.x, t1.y, 3.0, BARK_DARK);
            draw_line(b0.x, b0.y, b1.x, b1.y, 2.0, BARK_DARK);
            draw_line(t0.x, t0.y + 2.5, t1.x, t1.y + 2.5, 2.0, BARK_LIGHT);
        }
        // Close off the tip
        let (tip, tip_bottom) = (self.tip(), self.bottom_point(segments));
        draw_line(tip.x, tip.y, tip_bottom.x, tip_bottom.y, 2.0, BARK_DARK);
        if self.strain() > 0.5 && !self.broken() {
            self.draw_cracks();
        }
    }

    /// Jagged splits where the limb will snap, growing as it nears breaking.
    fn draw_cracks(&self) {
        let seg = ((self.branch.snap_distance / self.branch.segment_len) as usize).max(1);
        let top = self.top_point(seg + 1);
        let depth = self.bottom_point(seg + 1).y - top.y;
        let reach = ((self.strain() - 0.5) * 2.0).min(1.0);
        for side in [0.1, 0.55] {
            let mut p = vec2(top.x, top.y + depth * side);
            for k in 0..3 {
                let q = p + vec2(self.direction * 4.0, if k % 2 == 0 { 5.0 } else { -3.0 } * reach);
                draw_line(p.x, p.y, q.x, q.y, 2.0, BARK_DARK);
                p = q;
            }
        }
    }
}
