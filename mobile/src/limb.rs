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
#[derive(Clone)]
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
    /// Young wood that gives out under Kitty and a dog together (drawn paler, fewer leaves).
    pub thin: bool,
    pub tree: usize,
    pub trunk_half: f32,
    pub attachment: Option<Attachment>,
    pub seed: u64,
}

#[derive(Clone)]
pub struct Limb {
    pub branch: Branch,
    pub direction: f32,
    pub length: f32,
    pub thickness: f32,
    pub thin: bool,
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
    /// Give where it was measured (see `calibrate_give`), how far out from the base that was,
    /// and, for a fork, the give its parent adds at the base.
    give_measured: f32,
    give_reach: f32,
    give_base: f32,
    /// Strain left in the wood by hard, braced landings (bouncing on it), fading slowly. It counts
    /// with the next landing's jolt toward snapping, so bouncing too hard again and again breaks a
    /// limb that no single landing would (but just standing or crouching on it never does).
    fatigue: f32,
    /// Jolted since the last step.
    jolted: bool,
    /// Broken after the last step, to tell when it has just snapped.
    was_broken: bool,
    /// A sturdy limb that snapped partway along: the stub left on the tree is shorter and stiffer
    /// and can't snap again.
    pub stub: bool,
    /// A fork whose parent snapped off between the trunk and the fork: none of it is left.
    pub fallen: bool,
    /// The walkable span as it was when the limb snapped, so things resting on it keep their
    /// places on the stub (and those past the break go with the piece that fell).
    snapped_span: Option<(f32, f32)>,
}

/// Measuring a limb's give: Kitty's weight this far along the walkable span, held this many
/// steps, with the sag averaged over the last few (the limb is still swaying a little).
const GIVE_PROBE: f32 = 0.85;
const GIVE_PROBE_STEPS: usize = 90;
const GIVE_PROBE_AVERAGE: usize = 30;

/// An over-pumped sturdy limb breaks right under Kitty's paws, but the stub it leaves keeps at
/// least the first share of its segments (so the level can still be climbed past it) and at most
/// the second (so a real piece falls away).
pub const STUB_SHARE: (f32, f32) = (0.6, 0.8);
/// The stub's wood is this stiff (the springs' stiffness, of at most 1), so it barely gives, and
/// dead: it takes only this share of a kick (a landing, a push-off), so it hardly springs back.
const STUB_STIFFNESS: f32 = 1.0;
const STUB_KICK: f32 = 0.15;
/// The share of the whole limb's give a stub keeps, `.0 * s + .1` for wood of the limb's stiffness
/// `s`: about a quarter for the bendiest wood, three quarters for the stiffest. Fitted to stubs
/// settled and measured like whole limbs: most come within 0.02 of that, the worst 0.15 (where
/// the whole limb's own give is least like a beam's).
const STUB_GIVE_KEPT: (f32, f32) = (1.04, -0.28);

/// The segments a sturdy limb of `segments` can break at (see `STUB_SHARE`): the stub keeps the
/// segments up to there.
pub fn stub_segments(segments: usize) -> (usize, usize) {
    let lo = (STUB_SHARE.0 * segments as f32).ceil() as usize;
    let hi = ((STUB_SHARE.1 * segments as f32) as usize).min(segments - 2);
    (lo.min(segments - 2), hi.max(lo).min(segments - 2))
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
            // Thin wood breaks right where it leaves the bark; sturdy wood partway along (aimed
            // under Kitty's paws as she lands; see `aim_break`)
            branch.strength = spec.strength;
            branch.snap_distance = if spec.thin { spec.trunk_half } else { (stub_segments(segments).0 as f32 + 0.5) * branch.segment_len };
        }
        let mut limb = Self {
            branch,
            direction: spec.direction,
            length: spec.length,
            thickness: spec.thickness,
            thin: spec.thin,
            tree: spec.tree,
            trunk_half: spec.trunk_half,
            attachment: spec.attachment,
            anchor: spec.start,
            decorations: Vec::new(),
            wind_phase: (spec.seed % 628) as f32 / 100.0,
            just_snapped: false,
            is_goal: false,
            last_surface: Vec::new(),
            give_measured: 0.0,
            give_reach: 1.0,
            give_base: 0.0,
            fatigue: 0.0,
            jolted: false,
            was_broken: false,
            stub: false,
            fallen: false,
            snapped_span: None,
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

    /// Snapped (thin limbs at the trunk, sturdy ones partway along, leaving a stub).
    pub fn broken(&self) -> bool {
        self.branch.broken_at.is_some()
    }

    /// Snapped with nothing left to stand on: a thin limb snapped at the trunk, or a fork that fell
    /// with its parent. (A sturdy limb's stub is still there.)
    pub fn gone(&self) -> bool {
        self.broken() && !self.stub
    }

    /// Whether the wood at `x` has fallen away: the limb snapped somewhere between there and the
    /// trunk.
    pub fn fell_away(&self, x: f32) -> bool {
        self.broken() && !(self.stub && self.surface_y(x).is_some())
    }

    /// It snapped just now, or there's nothing of it left: whatever was on it falls in a panic.
    pub fn gave_way(&self) -> bool {
        self.just_snapped || self.gone()
    }

    /// 0 when unloaded, 1 at the point of snapping.
    pub fn strain(&self) -> f32 {
        if self.breakable() { (self.branch.stress + self.fatigue) / self.branch.strength } else { 0.0 }
    }

    /// A hard landing at `x`: a sudden extra load on top of the weight resting on the limb, of
    /// which `fatigue` lingers in the wood. A sturdy limb that gives way now breaks under it.
    pub fn jolt(&mut self, x: f32, force: f32, fatigue: f32) {
        self.aim_break(x);
        self.branch.apply_jolt(force);
        self.fatigue += fatigue;
        self.jolted = true;
    }

    /// Where a sturdy limb will break if it gives way: at the segment under `x`, kept within
    /// `STUB_SHARE` of the limb so the stub left is long enough to use. Thin limbs always break at
    /// the trunk.
    fn aim_break(&mut self, x: f32) {
        if self.thin || !self.breakable() || self.broken() {
            return;
        }
        let n = self.segments();
        let reach = (x - self.base().x) * self.direction;
        let under = if reach <= 0.0 {
            0
        } else {
            (0..n).find(|&i| (x - self.top_point(i + 1).x) * self.direction <= 0.0).unwrap_or(n - 1)
        };
        let (lo, hi) = stub_segments(n);
        self.branch.snap_distance = (under.clamp(lo, hi) as f32 + 0.5) * self.branch.segment_len;
    }

    /// Snap it now: a thin limb at the trunk, a sturdy one at the segment under `x` (within
    /// `STUB_SHARE`). The next `step` finishes the job, as for any snap.
    pub fn snap_at(&mut self, x: f32) {
        if self.breakable() && !self.broken() {
            self.aim_break(x);
            self.snapped_span = Some(self.walk_range());
            self.branch.snap();
        }
    }

    /// Its parent snapped off between the trunk and where it grows: it goes down with that piece.
    pub fn fall_with_parent(&mut self) {
        if !self.fallen {
            self.fallen = true;
            self.stub = false;
            self.branch.strength = 0.0;
            self.snapped_span = self.snapped_span.or(Some(self.walk_range()));
            self.branch.snap();
        }
    }

    /// Just snapped partway along: the stub left on the tree springs back stiffer, has little give
    /// left, and never snaps again.
    fn become_stub(&mut self) {
        self.stub = true;
        self.branch.strength = 0.0;
        self.fatigue = 0.0;
        // Every spring starts out as stiff as the limb's wood
        let stiffness = self.branch.constraints.first().map_or(STUB_STIFFNESS, |c| c.stiffness);
        if let Some(seg) = self.branch.broken_at {
            let columns = self.segments() + 1;
            for c in &mut self.branch.constraints {
                if c.node_a % columns <= seg && c.node_b % columns <= seg {
                    c.stiffness = STUB_STIFFNESS;
                }
            }
        }
        // As for a beam fixed at one end, how far it sags under a weight doesn't depend on the
        // wood past the weight, so the stub gives as the whole limb did at the same spot, less
        // what its stiffer wood takes out
        let (slope, offset) = STUB_GIVE_KEPT;
        self.give_measured *= (slope * stiffness + offset).clamp(0.0, 1.0);
    }

    /// Measure how far the limb sags with Kitty standing near its tip, on a copy so the level
    /// itself is untouched. `base_give` is what a fork's parent adds where the fork grows out.
    pub fn calibrate_give(&mut self, base_give: f32) {
        (self.give_measured, self.give_reach) = self.measure_give();
        self.give_base = base_give;
    }

    /// For a fork: its parent's give where it grows out, passed on (it changes if the parent is
    /// left a stub).
    pub fn set_base_give(&mut self, base_give: f32) {
        self.give_base = base_give;
    }

    /// The give measured near the end of the walkable span, and how far out from the base that was.
    fn measure_give(&self) -> (f32, f32) {
        let (lo, hi) = self.walk_range();
        let x = if self.direction > 0.0 { lo + (hi - lo) * GIVE_PROBE } else { hi - (hi - lo) * GIVE_PROBE };
        let mut probe = self.clone();
        let mut rng = Rng::new(0x61FE);
        let rest = probe.surface_y(x).unwrap_or(0.0);
        let mut sag = 0.0;
        for step in 0..GIVE_PROBE_STEPS {
            probe.branch.apply_load(x, KITTY_WEIGHT);
            probe.step(0.0, 0.0, None, &mut rng);
            if step >= GIVE_PROBE_STEPS - GIVE_PROBE_AVERAGE {
                sag += probe.surface_y(x).map_or(0.0, |y| y - rest) / GIVE_PROBE_AVERAGE as f32;
            }
        }
        (sag.max(0.0) / FULL_GIVE_SAG, ((x - self.base().x) * self.direction).max(1.0))
    }

    /// How much the limb yields under Kitty at `x`, 0 (solid, by the trunk) to 1 (a whippy
    /// tip): her push loses that much of its power into bending the wood. Like a beam fixed at
    /// one end, the sag grows with the cube of the distance from the trunk.
    pub fn give_at(&self, x: f32) -> f32 {
        let reach = ((x - self.base().x) * self.direction).max(0.0) / self.give_reach;
        (self.give_base + self.give_measured * reach.powi(3)).min(1.0)
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
        let span = (!self.broken()).then(|| self.walk_range());
        self.branch.update_physics(BRANCH_SUB_STEPS, STEP, BRANCH_GRAVITY, BRANCH_DAMPING);
        // The engine snaps a limb that's carrying too much; worn by bouncing, the next hard
        // landing breaks it sooner
        self.fatigue *= 1.0 - STEP / FATIGUE_FADE;
        let jolted = std::mem::take(&mut self.jolted);
        if jolted && self.breakable() && !self.broken() && self.branch.stress + self.fatigue > self.branch.strength {
            self.branch.snap();
        }
        if self.broken() {
            self.fatigue = 0.0;
        }
        self.just_snapped = self.broken() && !self.was_broken;
        self.was_broken = self.broken();
        if self.just_snapped {
            self.snapped_span = self.snapped_span.or(span);
            if !self.thin && !self.fallen {
                self.become_stub();
            }
        }
    }

    fn remember_surface(&mut self) {
        self.last_surface.clear();
        self.last_surface.extend(self.branch.nodes[..=self.branch.segments].iter().map(|n| n.pos.y));
    }

    /// Segments still on the tree.
    fn attached(&self) -> usize {
        if self.fallen { 0 } else { self.branch.broken_at.unwrap_or(self.branch.segments) }
    }

    /// How far the top surface under `x` moved down during the last step (negative = springing up).
    pub fn surface_velocity(&self, x: f32) -> f32 {
        for i in 0..self.attached() {
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

    /// The top surface at `x`, where the wood is still on the tree (not the scrap a thin limb
    /// leaves inside the trunk when it snaps).
    pub fn surface_y(&self, x: f32) -> Option<f32> {
        if self.gone() { None } else { self.branch.surface_y(x) }
    }

    /// Highest point of the top surface under any of `xs`.
    pub fn surface_under(&self, xs: &[f32]) -> Option<f32> {
        xs.iter().filter_map(|&x| self.surface_y(x)).reduce(f32::min)
    }

    pub fn kick(&mut self, x: f32, speed: f32) {
        let speed = if self.stub { speed * STUB_KICK } else { speed };
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

    /// The end of the wood still on the tree: the tip, or where a stub snapped off.
    pub fn end(&self) -> Vec2 {
        match self.branch.broken_at {
            Some(seg) if self.stub => self.top_point(seg),
            _ => self.tip(),
        }
    }

    pub fn segments(&self) -> usize {
        self.branch.segments
    }

    /// x-extent something can walk along: from the trunk's bark to just short of the end (of the
    /// stub, once a sturdy limb has snapped).
    pub fn walk_range(&self) -> (f32, f32) {
        let a = self.base().x + self.direction * (self.trunk_half + 10.0);
        let b = self.end().x - self.direction * 10.0;
        (a.min(b), a.max(b))
    }

    /// The walkable span as it was before the limb snapped, for placing things along it that stay
    /// put on the stub, or fall with the piece past the break.
    pub fn whole_walk_range(&self) -> (f32, f32) {
        self.snapped_span.unwrap_or_else(|| self.walk_range())
    }

    pub fn bounds(&self) -> Rect {
        let (x0, y0, x1, y1) = self.branch.bounds();
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Scatter leaf clusters and twigs along the limb. `autumn` is the share of autumn leaves.
    pub fn decorate(&mut self, rng: &mut Rng, autumn: f32) {
        let segments = self.segments();
        let thin = self.thin;
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
        let i = i.min(n);
        // At an end (the tip, or where it snapped) along the last bit of wood before it
        let end = i == n || self.branch.broken_at == Some(i);
        let (a, b) = if end { (self.top_point(i - 1), self.top_point(i)) } else { (self.top_point(i), self.top_point(i + 1)) };
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

    /// Smoothed outline of columns `from..=to`: Catmull-Rom curves through the top nodes and
    /// through the bottom nodes, sampled at matching points so they pair up across the wood.
    /// The curves pass through every node, so the top edge stays on the walkable surface.
    fn piece(&self, from: usize, to: usize, start: End, end: End) -> Piece {
        let mut piece = Piece { start, end, ..Piece::default() };
        self.sample_edge(0, from, to, &mut piece.top);
        self.sample_edge(self.branch.segments + 1, from, to, &mut piece.bottom);
        let seg = self.branch.segment_len;
        for i in from..to {
            for k in 0..SAMPLES {
                piece.along.push((i as f32 + k as f32 / SAMPLES as f32) * seg);
            }
        }
        piece.along.push(to as f32 * seg);
        piece
    }

    fn sample_edge(&self, offset: usize, from: usize, to: usize, out: &mut Vec<Vec2>) {
        let node = |i: usize| {
            let p = self.branch.nodes[offset + i].pos;
            vec2(p.x, p.y)
        };
        // Past either end, carry straight on so the curve doesn't hook
        let at = |i: isize| {
            if i < from as isize {
                2.0 * node(from) - node(from + 1)
            } else if i > to as isize {
                2.0 * node(to) - node(to - 1)
            } else {
                node(i as usize)
            }
        };
        for i in from..to {
            let (p0, p1, p2, p3) = (at(i as isize - 1), node(i), node(i + 1), at(i as isize + 2));
            for k in 0..SAMPLES {
                out.push(catmull_rom(p0, p1, p2, p3, k as f32 / SAMPLES as f32));
            }
        }
        out.push(node(to));
    }

    /// The wood in one piece, or in two once it has snapped.
    fn add_pieces(&self, id: LimbId, out: &mut Vec<(LimbId, Piece)>) {
        let n = self.segments();
        match self.branch.broken_at {
            None => out.push((id, self.piece(0, n, End::Hidden, End::Round))),
            Some(seg) => {
                if seg >= 1 {
                    out.push((id, self.piece(0, seg, End::Hidden, End::Snapped)));
                }
                if seg + 1 < n {
                    out.push((id, self.piece(seg + 1, n, End::Snapped, End::Round)));
                }
            }
        }
    }

    /// Where along the grain texture this limb starts, so neighbours don't share a pattern.
    fn grain_offset(&self) -> f32 {
        16.0 + (self.wind_phase * 41.0) % 250.0
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

// ---------------------------------------------------------------------------------------------
// The wood. Each limb is drawn from smooth curves through its nodes: its shape grown outward in
// dark bark for the outline, then a shaded, grained fill on top. A limb and its forks are drawn
// together pass by pass (every outline, then every fill), so where a fork grows out of its
// parent the two merge into one silhouette with no line between them, and a fillet rounds off
// each corner of the join.

/// Smoothed samples per physics segment.
const SAMPLES: usize = 4;
/// How far the dark outline reaches outside the wood.
const OUTLINE: f32 = 2.2;
/// A tip's rounded end reaches this many times its half-thickness past the last node.
const TIP_STRETCH: f32 = 1.3;
const TIP_STEPS: usize = 8;
/// A snapped end's splinters, as shares of how far they poke out past the break.
const SPLINTERS: [f32; 7] = [0.35, 0.95, 0.2, 1.0, 0.5, 0.8, 0.15];
const FILLET_STEPS: usize = 6;
/// Below the parent's top edge, a fork's root fades into the parent's wood over this depth.
const FORK_BLEND: f32 = 5.0;
/// How far into the trunk a limb's wood fades out, across the trunk's dark edge.
const ROOT_BLEND: f32 = 7.0;
/// Rows of vertices across the wood, top edge to underside, for the shading.
const ROWS: usize = 5;
/// Where the rows sit across the thin wood near a tip (thicker wood keeps its sunlit edge just
/// as narrow; see `row_fractions`).
const ROW_STOPS: [f32; ROWS] = [0.0, 0.16, 0.42, 0.72, 1.0];

const GRAIN_W: u16 = 512;
const GRAIN_H: u16 = 32;
/// Plain white texels along the top and left of the grain texture, for flat-coloured shapes.
const GRAIN_PLAIN_ROWS: f32 = 2.0;
const PLAIN: Vec2 = Vec2::new(0.5 / GRAIN_W as f32, 0.5 / GRAIN_H as f32);
/// Keep each mesh comfortably inside macroquad's per-draw-call limits.
const MESH_VERTICES: usize = 4000;
const MESH_INDICES: usize = 4800;

/// Bark grain running along the limbs: white, so the mesh's colours show through, with faint
/// darker streaks that follow the wood's curves and taper with it toward the tip.
pub fn make_grain_texture() -> Texture2D {
    let (w, h) = (GRAIN_W as u32, GRAIN_H as u32);
    let mut image = Image::gen_image_color(GRAIN_W, GRAIN_H, WHITE);
    let mut rng = Rng::new(0xB4C4);
    for _ in 0..160 {
        let y0 = rng.range(GRAIN_PLAIN_ROWS + 1.5, h as f32 - 1.5);
        let x0 = rng.int(8, w as i32 - 20);
        let len = rng.int(14, 80);
        let dark = rng.range(0.08, 0.22);
        let wobble = rng.range(0.0, 6.3);
        for dx in 0..len {
            let x = (x0 + dx) as u32;
            if x >= w {
                break;
            }
            let y = (y0 + (dx as f32 * 0.07 + wobble).sin() * 0.9).round().clamp(GRAIN_PLAIN_ROWS + 1.0, h as f32 - 1.0) as u32;
            // Streaks fade in and out at their ends
            let ends = (dx.min(len - 1 - dx) as f32 / 6.0).min(1.0);
            let s = 1.0 - dark * ends;
            let c = image.get_pixel(x, y);
            image.set_pixel(x, y, Color::new(c.r * s, c.g * s, c.b * s, 1.0));
        }
    }
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Linear);
    texture
}

/// How a piece of wood ends.
#[derive(Clone, Copy, PartialEq, Default)]
enum End {
    /// Out of sight: under the trunk's bark, or inside the parent's wood for a fork.
    #[default]
    Hidden,
    /// A rounded tip.
    Round,
    /// Splintered, where it snapped.
    Snapped,
}

/// One piece of wood (a whole limb, or one side of where it snapped), smoothed: matching samples
/// along its top and bottom edges from base to tip, with the normals pointing out of the wood.
#[derive(Default)]
struct Piece {
    top: Vec<Vec2>,
    bottom: Vec<Vec2>,
    top_n: Vec<Vec2>,
    bottom_n: Vec<Vec2>,
    /// How far each sample is from the limb's base, so the grain stays put on the wood.
    along: Vec<f32>,
    start: End,
    end: End,
}

impl Piece {
    fn finish(&mut self) {
        outward_normals(&self.top, &self.bottom, &mut self.top_n);
        outward_normals(&self.bottom, &self.top, &mut self.bottom_n);
    }

    /// The way the wood runs at sample `k`, base to tip.
    fn forward(&self, k: usize) -> Vec2 {
        let last = self.top.len() - 1;
        let (a, b) = if k == 0 { (0, 1) } else { (k - 1, k.min(last)) };
        (self.top[b] - self.top[a] + self.bottom[b] - self.bottom[a]).normalize_or_zero()
    }

    /// The rounded tip or splintered break closing one end, if that end shows.
    fn cap(&self, at_end: bool, reach: f32, phase: usize) -> Option<Cap> {
        let kind = if at_end { self.end } else { self.start };
        let k = if at_end { self.top.len() - 1 } else { 0 };
        let (top, bottom) = (self.top[k], self.bottom[k]);
        let out = if at_end { self.forward(k) } else { -self.forward(k) };
        let mid = (top + bottom) * 0.5;
        let r = (top - mid).length();
        if kind == End::Hidden || r < 0.05 || out == Vec2::ZERO {
            return None;
        }
        let mut chain = vec![CapPoint { pos: top, normal: self.top_n[k], f: 0.0 }];
        if kind == End::Round {
            let across = (top - mid) / r;
            let ahead = (out - across * out.dot(across)).normalize_or_zero();
            for j in 1..TIP_STEPS {
                let (s, c) = (std::f32::consts::PI * j as f32 / TIP_STEPS as f32).sin_cos();
                let pos = mid + across * (r * c) + ahead * (r * TIP_STRETCH * s);
                chain.push(CapPoint { pos, normal: Vec2::ZERO, f: (1.0 - c) * 0.5 });
            }
        } else {
            let n = SPLINTERS.len();
            for j in 0..n {
                let f = (j + 1) as f32 / (n + 1) as f32;
                let pos = top.lerp(bottom, f) + out * (SPLINTERS[(j + phase) % n] * reach);
                chain.push(CapPoint { pos, normal: Vec2::ZERO, f });
            }
        }
        chain.push(CapPoint { pos: bottom, normal: self.bottom_n[k], f: 1.0 });
        miter_normals(&mut chain, out);
        // The fan's centre sits a hair inside the wood so it overlaps the body seamlessly
        Some(Cap { centre: mid - out * 0.75, chain, along: self.along[k], out, snapped: kind == End::Snapped })
    }
}

struct CapPoint {
    pos: Vec2,
    /// Outward, scaled so offsetting by it keeps the outline an even width round corners.
    normal: Vec2,
    /// How far across the wood, 0 at the top edge to 1 underneath.
    f: f32,
}

/// The end of a piece beyond its last cross-section, from its top corner round to its bottom one.
struct Cap {
    centre: Vec2,
    chain: Vec<CapPoint>,
    along: f32,
    out: Vec2,
    snapped: bool,
}

/// A curved patch filling the corner where a fork's edge meets its parent's, so the join looks
/// grown rather than stuck on.
struct Fillet {
    corner: Vec2,
    /// Points along the curve with their outward normals.
    curve: [(Vec2, Vec2); FILLET_STEPS + 1],
    /// Colours at the curve's two ends and in the corner.
    colors: [Color; 3],
}

impl Fillet {
    /// A curve from `a` to `b` that leaves each tangent to the edge it starts on (both run
    /// straight into `corner`).
    fn new(a: Vec2, corner: Vec2, b: Vec2, colors: [Color; 3]) -> Self {
        let at = |t: f32| a * (1.0 - t) * (1.0 - t) + corner * 2.0 * t * (1.0 - t) + b * t * t;
        let slope = |t: f32| (corner - a) * 2.0 * (1.0 - t) + (b - corner) * 2.0 * t;
        // Outward is away from the corner; keep to that side all along the curve
        let side = if slope(0.5).perp().dot(at(0.5) - corner) >= 0.0 { 1.0 } else { -1.0 };
        let mut curve = [(Vec2::ZERO, Vec2::ZERO); FILLET_STEPS + 1];
        for (j, point) in curve.iter_mut().enumerate() {
            let t = j as f32 / FILLET_STEPS as f32;
            *point = (at(t), slope(t).perp().normalize_or_zero() * side);
        }
        Self { corner, curve, colors }
    }
}

/// Which way a fork leaves its parent's wood. Forks are aimed upward, but one sprouting where
/// its parent climbs more steeply drops out through the parent's underside instead, or runs
/// along inside it.
#[derive(Clone, Copy, PartialEq)]
enum Emerges {
    /// Up through the parent's top edge.
    Up,
    /// Down through the parent's underside.
    Down,
    /// Neither, or not clear of it.
    Along,
}

impl Emerges {
    fn find(fork: &Piece, parent: &Piece) -> Self {
        let up = crosses(&fork.bottom, &parent.top, true).map(|(k, _)| k);
        let down = crosses(&fork.top, &parent.bottom, false).map(|(k, _)| k);
        match (up, down) {
            (Some(u), Some(d)) if d < u => Emerges::Down,
            (Some(_), _) => Emerges::Up,
            (None, Some(_)) => Emerges::Down,
            (None, None) => Emerges::Along,
        }
    }
}

/// Where a fork grows out of its parent.
struct Join {
    /// Indices into the family's pieces.
    fork: usize,
    parent: usize,
    way: Emerges,
    /// Distances along the fork from its base: before the first, any of it lying deep inside
    /// the parent's wood is hidden; by the second it is drawn whole over the parent.
    blend: (f32, f32),
    fillets: Vec<Fillet>,
}

impl Join {
    /// How opaque the fork is at sample `k`, point `p`, drawn over its parent.
    fn opacity(&self, fork: &Piece, parent: &Piece, k: usize, p: Vec2) -> f32 {
        let along = fork.along[k] - fork.along[0];
        let t = ((along - self.blend.0) / (self.blend.1 - self.blend.0)).clamp(0.0, 1.0);
        let shown = t * t * (3.0 - 2.0 * t);
        if shown >= 1.0 {
            return 1.0;
        }
        // Fade out inside the edge it grows through, so it seems to grow out of the wood
        let depth = if self.way == Emerges::Down {
            edge_y_at(&parent.bottom, p.x).map_or(f32::MIN, |y| y - p.y)
        } else {
            edge_y_at(&parent.top, p.x).map_or(f32::MIN, |y| p.y - y)
        };
        shown.max((1.0 - depth / FORK_BLEND).clamp(0.0, 1.0))
    }
}

/// Colours down through the wood, top edge to underside.
struct Palette {
    rows: [Color; ROWS],
}

impl Palette {
    fn for_limb(limb: &Limb) -> Self {
        // Thin limbs that can snap are young wood: paler, with a hint of green
        let (light, fill, shade) = if limb.thin {
            (BARK_YOUNG_LIGHT, BARK_YOUNG, BARK_YOUNG_SHADE)
        } else {
            (BARK_LIGHT, BARK, BARK_SHADE)
        };
        Self { rows: [light, mix(light, fill, 0.5), fill, mix(fill, shade, 0.3), shade] }
    }

    /// The colour at `f` of the way from the top edge to the underside of thin wood.
    fn at(&self, f: f32) -> Color {
        let f = f.clamp(0.0, 1.0);
        let i = ROW_STOPS.iter().rposition(|&s| s <= f).unwrap_or(0).min(ROWS - 2);
        mix(self.rows[i], self.rows[i + 1], (f - ROW_STOPS[i]) / (ROW_STOPS[i + 1] - ROW_STOPS[i]))
    }
}

/// Where the rows of shading sit across wood `width` thick: a narrow sunlit edge along the top
/// (the same few pixels however thick the wood), the body, then shadow underneath.
fn row_fractions(width: f32) -> [f32; ROWS] {
    let width = width.max(0.5);
    let light = (1.8 / width).min(ROW_STOPS[1]);
    let body = (5.5 / width).clamp(light + 0.04, ROW_STOPS[2]);
    [0.0, light, body, ROW_STOPS[3], 1.0]
}

/// Wood geometry gathered into one mesh. Everything shares the grain texture (flat colours use
/// its plain corner), so the wood draws in very few batches.
struct WoodMesh {
    mesh: Mesh,
}

impl WoodMesh {
    fn new(grain: &Texture2D) -> Self {
        Self { mesh: Mesh { vertices: Vec::with_capacity(1024), indices: Vec::with_capacity(2048), texture: Some(grain.clone()) } }
    }

    /// Make room for a shape; returns the index its vertices start at.
    fn start(&mut self, vertices: usize, indices: usize) -> u16 {
        if self.mesh.vertices.len() + vertices > MESH_VERTICES || self.mesh.indices.len() + indices > MESH_INDICES {
            self.flush();
        }
        self.mesh.vertices.len() as u16
    }

    fn vertex(&mut self, p: Vec2, uv: Vec2, color: Color) {
        self.mesh.vertices.push(Vertex::new(p.x, p.y, 0.0, uv.x, uv.y, color));
    }

    fn triangle(&mut self, a: u16, b: u16, c: u16) {
        self.mesh.indices.extend_from_slice(&[a, b, c]);
    }

    fn quad(&mut self, a: u16, b: u16, c: u16, d: u16) {
        self.triangle(a, b, c);
        self.triangle(a, c, d);
    }

    /// A fan from `centre` round `rim`.
    fn fan(&mut self, centre: (Vec2, Vec2, Color), rim: impl ExactSizeIterator<Item = (Vec2, Vec2, Color)>) {
        let n = rim.len();
        if n < 2 {
            return;
        }
        let first = self.start(n + 1, 3 * (n - 1));
        self.vertex(centre.0, centre.1, centre.2);
        for (p, uv, color) in rim {
            self.vertex(p, uv, color);
        }
        for j in 0..n as u16 - 1 {
            self.triangle(first, first + 1 + j, first + 2 + j);
        }
    }

    fn flush(&mut self) {
        if !self.mesh.indices.is_empty() {
            draw_mesh(&self.mesh);
        }
        self.mesh.vertices.clear();
        self.mesh.indices.clear();
    }
}

/// After the trunks: bring each limb's wood back out over the trunk's dark edge where it leaves
/// the bark, fading into the trunk, so the limb grows out of the trunk instead of having the
/// trunk's edge drawn across it.
pub fn draw_roots(limbs: &[Limb], visible: impl Fn(&Limb) -> bool, grain: &Texture2D) {
    let mut mesh = WoodMesh::new(grain);
    for limb in limbs.iter().filter(|l| l.attachment.is_none() && l.trunk_half > 0.0 && !l.gone() && visible(l)) {
        // The root's first few columns are all the curve needs there
        let mut piece = limb.piece(0, limb.segments().min(3), End::Hidden, End::Hidden);
        piece.finish();
        let dir = limb.direction;
        let edge = limb.base().x + dir * limb.trunk_half;
        // Signed distance outward past the trunk's edge
        let out = |p: Vec2| (p.x - edge) * dir;
        // From where the wood is wholly hidden in the trunk to where it's wholly out of it, so the
        // overlay starts invisible and ends matching the wood beneath
        let reach = |k: usize| (out(piece.top[k]).min(out(piece.bottom[k])), out(piece.top[k]).max(out(piece.bottom[k])));
        let n = piece.top.len();
        let first = (0..n).rev().find(|&k| reach(k).1 <= -ROOT_BLEND).unwrap_or(0);
        let last = (first..n).find(|&k| reach(k).0 >= 0.0).unwrap_or(n - 1);
        let fade = |_: usize, p: Vec2| ((out(p) + ROOT_BLEND) / (ROOT_BLEND - 1.0)).clamp(0.0, 1.0);
        shade_piece(&mut mesh, &piece, &Palette::for_limb(limb), limb.grain_offset(), first..last + 1, Some(&fade));
    }
    mesh.flush();
}

/// Draw the wood of every visible limb. Each limb is drawn together with its forks, so they
/// merge where they join; separate limbs keep their own outlines where they cross.
pub fn draw_wood(limbs: &[Limb], visible: impl Fn(&Limb) -> bool, grain: &Texture2D) {
    let mut root = Vec::with_capacity(limbs.len());
    for (i, limb) in limbs.iter().enumerate() {
        let r = match limb.attachment {
            Some(a) if a.parent < i => root[a.parent],
            _ => i,
        };
        root.push(r);
    }
    let mut mesh = WoodMesh::new(grain);
    let mut family = Vec::new();
    for r in 0..limbs.len() {
        if root[r] != r {
            continue;
        }
        family.clear();
        family.extend((r..limbs.len()).filter(|&i| root[i] == r));
        if family.iter().any(|&i| visible(&limbs[i])) {
            draw_family(limbs, &family, &mut mesh);
        }
    }
    mesh.flush();
}

fn draw_family(limbs: &[Limb], family: &[LimbId], mesh: &mut WoodMesh) {
    let mut pieces: Vec<(LimbId, Piece)> = Vec::new();
    for &id in family {
        limbs[id].add_pieces(id, &mut pieces);
    }
    // A fork's base sits inside its parent's wood; keep its underside tucked in there too
    let mut forks = Vec::new();
    for i in 0..pieces.len() {
        let id = pieces[i].0;
        let Some(attachment) = limbs[id].attachment else { continue };
        if first_piece(&pieces, id) != Some(i) {
            continue;
        }
        if let Some(p) = parent_piece(limbs, &pieces, attachment).filter(|&p| p < i) {
            let (before, after) = pieces.split_at_mut(i);
            let way = Emerges::find(&after[0].1, &before[p].1);
            tuck_under(&mut after[0].1, &before[p].1, way, limbs[id].thickness);
            forks.push((i, p, way));
        }
    }
    for (_, piece) in &mut pieces {
        piece.finish();
    }
    let palettes: Vec<Palette> = pieces.iter().map(|(id, _)| Palette::for_limb(&limbs[*id])).collect();
    let joins: Vec<Join> = forks.iter().map(|&(fork, parent, way)| {
        join(&pieces[fork].1, &pieces[parent].1, (fork, parent), way, limbs[pieces[fork].0].thickness, &palettes)
    }).collect();
    let caps: Vec<[Option<Cap>; 2]> = pieces.iter().enumerate().map(|(i, (id, piece))| {
        let limb = &limbs[*id];
        let reach = limb.branch.segment_len * 0.55;
        let phase = (limb.wind_phase * 10.0) as usize + i * 3;
        [piece.cap(false, reach, phase), piece.cap(true, reach, phase + 2)]
    }).collect();

    // Every outline first: the wood grown outward, in dark bark
    for (i, (_, piece)) in pieces.iter().enumerate() {
        outline_piece(mesh, piece, &caps[i]);
    }
    for fillet in joins.iter().flat_map(|j| &j.fillets) {
        let rim = fillet.curve.iter().map(|&(p, n)| (p + n * OUTLINE, PLAIN, BARK_DARK));
        mesh.fan((fillet.corner, PLAIN, BARK_DARK), rim);
    }
    // Forks' wood, which the parent then covers where it lies inside it
    for join in &joins {
        let (id, piece) = &pieces[join.fork];
        shade_piece(mesh, piece, &palettes[join.fork], limbs[*id].grain_offset(), 0..piece.top.len(), None);
        shade_caps(mesh, &caps[join.fork], &palettes[join.fork], limbs[*id].grain_offset());
    }
    // Every piece's wood in order (parents before their forks). A fork is drawn again over its
    // parent, its root fading out below the parent's top edge so the two blend
    for (i, (id, piece)) in pieces.iter().enumerate() {
        let u0 = limbs[*id].grain_offset();
        match joins.iter().find(|j| j.fork == i) {
            Some(join) => {
                let parent = &pieces[join.parent].1;
                let fade = |k: usize, p: Vec2| join.opacity(piece, parent, k, p);
                shade_piece(mesh, piece, &palettes[i], u0, 0..piece.top.len(), Some(&fade));
            }
            None => shade_piece(mesh, piece, &palettes[i], u0, 0..piece.top.len(), None),
        }
        shade_caps(mesh, &caps[i], &palettes[i], u0);
    }
    for fillet in joins.iter().flat_map(|j| &j.fillets) {
        let rim = fillet.curve.iter().enumerate().map(|(j, &(p, _))| {
            (p, PLAIN, mix(fillet.colors[0], fillet.colors[1], j as f32 / FILLET_STEPS as f32))
        });
        mesh.fan((fillet.corner, PLAIN, fillet.colors[2]), rim);
    }
    // Cracks in an overloaded limb, drawn over the wood
    let strained: Vec<&Limb> = family.iter().map(|&id| &limbs[id]).filter(|l| l.strain() > 0.5 && !l.broken()).collect();
    if !strained.is_empty() {
        mesh.flush();
        for limb in strained {
            limb.draw_cracks();
        }
    }
}

/// The first of limb `id`'s pieces.
fn first_piece(pieces: &[(LimbId, Piece)], id: LimbId) -> Option<usize> {
    pieces.iter().position(|(l, _)| *l == id)
}

/// The piece of a fork's parent that holds the node the fork grows from: the stub, or the whole
/// limb, unless the parent snapped between the trunk and the fork, when the fork goes down with
/// the piece that broke off (see `Limb::add_pieces` for the order).
fn parent_piece(limbs: &[Limb], pieces: &[(LimbId, Piece)], attachment: Attachment) -> Option<usize> {
    let first = first_piece(pieces, attachment.parent)?;
    let outer = limbs[attachment.parent].branch.broken_at.is_some_and(|seg| seg < attachment.node);
    let p = if outer { first + 1 } else { first };
    pieces.get(p).is_some_and(|(l, _)| *l == attachment.parent).then_some(p)
}

/// The dark outline: the piece grown outward by `OUTLINE` all round.
fn outline_piece(mesh: &mut WoodMesh, piece: &Piece, caps: &[Option<Cap>; 2]) {
    let n = piece.top.len();
    let first = mesh.start(2 * n, 6 * (n - 1));
    for k in 0..n {
        mesh.vertex(piece.top[k] + piece.top_n[k] * OUTLINE, PLAIN, BARK_DARK);
        mesh.vertex(piece.bottom[k] + piece.bottom_n[k] * OUTLINE, PLAIN, BARK_DARK);
    }
    for k in 0..n as u16 - 1 {
        let i = first + 2 * k;
        mesh.quad(i, i + 2, i + 3, i + 1);
    }
    for cap in caps.iter().flatten() {
        let rim = cap.chain.iter().map(|c| (c.pos + c.normal * OUTLINE, PLAIN, BARK_DARK));
        mesh.fan((cap.centre, PLAIN, BARK_DARK), rim);
    }
}

/// The wood's fill over `samples`: rows of vertices across it shade it from a sunlit top edge
/// to a shadowed underside, with the grain running along it. `fade` gives a vertex's opacity.
fn shade_piece(mesh: &mut WoodMesh, piece: &Piece, palette: &Palette, u0: f32, samples: std::ops::Range<usize>,
               fade: Option<&dyn Fn(usize, Vec2) -> f32>) {
    let n = samples.len();
    if n < 2 {
        return;
    }
    let first = mesh.start(ROWS * n, 6 * (ROWS - 1) * (n - 1));
    for k in samples {
        let (top, bottom) = (piece.top[k], piece.bottom[k]);
        for (r, f) in row_fractions(top.distance(bottom)).into_iter().enumerate() {
            let p = top.lerp(bottom, f);
            let mut color = palette.rows[r];
            if let Some(fade) = fade {
                color.a = fade(k, p);
            }
            mesh.vertex(p, grain_uv(u0 + piece.along[k], f), color);
        }
    }
    for k in 0..n as u16 - 1 {
        let column = first + k * ROWS as u16;
        for r in 0..ROWS as u16 - 1 {
            let (a, b) = (column + r, column + ROWS as u16 + r);
            mesh.quad(a, b, b + 1, a + 1);
        }
    }
}

/// Fill the rounded tip, or the pale, splintered wood where it snapped.
fn shade_caps(mesh: &mut WoodMesh, caps: &[Option<Cap>; 2], palette: &Palette, u0: f32) {
    for cap in caps.iter().flatten() {
        let last = cap.chain.len() - 1;
        let rim = cap.chain.iter().enumerate().map(|(j, c)| {
            let color = if cap.snapped && j != 0 && j != last { WOOD_SPLINTER } else { palette.at(c.f) };
            (c.pos, grain_uv(u0 + cap.along + (c.pos - cap.centre).dot(cap.out), c.f), color)
        });
        let centre = if cap.snapped { mix(WOOD_SPLINTER, palette.at(0.5), 0.4) } else { palette.at(0.5) };
        mesh.fan((cap.centre, grain_uv(u0 + cap.along, 0.5), centre), rim);
    }
}

/// Keep a fork's underside inside its parent's wherever the fork lies along the parent's wood
/// (its base always does), so it can't poke out below as a stray knob or show its cut-off
/// base. One that drops out through the parent's underside is let go gradually after its
/// base, so it comes out smoothly.
fn tuck_under(fork: &mut Piece, parent: &Piece, way: Emerges, thickness: f32) {
    let base = fork.along[0];
    for k in 0..fork.top.len() {
        let Some(y) = edge_y_at(&parent.bottom, fork.bottom[k].x) else { continue };
        let mut limit = y - OUTLINE - 1.0;
        if way == Emerges::Down {
            let s = (fork.along[k] - base) / (thickness * 2.5);
            limit += thickness * s * s;
        }
        // Only while the fork's top is inside the parent; one bent right down below it is left be
        let inside = ((limit - fork.top[k].y - 2.0) / 4.0).clamp(0.0, 1.0);
        let b = fork.bottom[k].y;
        fork.bottom[k].y = b + (smooth_min(b, limit, 4.0) - b) * inside;
    }
}

/// The lesser of `a` and `b`, rounded off over `k` where they're close so curves stay smooth.
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// Where a fork comes out of its parent's wood: a fillet rounds off every corner where their
/// outlines cross (the narrow crotch on the tip side, the wide corner on the trunk side, and
/// any others an unusually bent fork makes), and the fork's root blends into the parent.
fn join(fork: &Piece, parent: &Piece, (fork_index, parent_index): (usize, usize), way: Emerges, fork_thickness: f32,
        palettes: &[Palette]) -> Join {
    let size = (fork_thickness * 0.6).clamp(5.0, 12.0);
    let (fork_colors, parent_colors) = (&palettes[fork_index].rows, &palettes[parent_index].rows);
    let mut fillets = Vec::new();
    for (fork_edge, fork_row) in [(&fork.top, 0), (&fork.bottom, ROWS - 1)] {
        for (parent_edge, parent_row) in [(&parent.top, 0), (&parent.bottom, ROWS - 1)] {
            for (fork_seg, parent_seg, corner) in intersections(fork_edge, parent_edge) {
                // The corner to fill is the one outside both: between the stretch of each edge
                // that leaves the other's wood
                let fork_ahead = !inside(parent, walk(fork_edge, fork_seg, corner, 1.5, true));
                let parent_ahead = !inside(fork, walk(parent_edge, parent_seg, corner, 1.5, true));
                let a = walk(parent_edge, parent_seg, corner, size, parent_ahead);
                let b = walk(fork_edge, fork_seg, corner, size, fork_ahead);
                let (ca, cb) = (parent_colors[parent_row], fork_colors[fork_row]);
                // A narrow crotch gets a shadowed crease
                let angle = (a - corner).angle_between(b - corner).abs();
                let crease = if angle.is_finite() { (1.0 - angle / std::f32::consts::FRAC_PI_2).clamp(0.0, 1.0) * 0.5 } else { 0.0 };
                fillets.push(Fillet::new(a, corner, b, [ca, cb, mix(mix(ca, cb, 0.5), BARK_DARK, crease)]));
            }
        }
    }
    // Past the crotch the fork is clear of the parent; one that never comes clear instead comes
    // into view along its length, over the parent, so its walkable top shows
    let crotch = match way {
        Emerges::Up => crosses(&fork.bottom, &parent.top, true),
        Emerges::Down => crosses(&fork.top, &parent.bottom, false),
        Emerges::Along => None,
    };
    let blend = match crotch {
        Some((k, _)) => {
            let at = fork.along[k + 1] - fork.along[0];
            (at, at + fork_thickness)
        }
        None => (fork_thickness * 0.5, fork_thickness * 2.5),
    };
    Join { fork: fork_index, parent: parent_index, way, blend, fillets }
}

/// Every point where polylines `a` and `b` cross: the segment of each it lies on, and the point.
fn intersections(a: &[Vec2], b: &[Vec2]) -> Vec<(usize, usize, Vec2)> {
    let mut found: Vec<(usize, usize, Vec2)> = Vec::new();
    // Only the stretch of `b` that comes anywhere near `a`
    let bounds = |line: &[Vec2]| line.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let (a_lo, a_hi) = bounds(a);
    let near: Vec<usize> = (0..b.len() - 1).filter(|&j| {
        let (lo, hi) = (b[j].min(b[j + 1]), b[j].max(b[j + 1]));
        lo.x <= a_hi.x && hi.x >= a_lo.x && lo.y <= a_hi.y && hi.y >= a_lo.y
    }).collect();
    for i in 0..a.len() - 1 {
        let (p, r) = (a[i], a[i + 1] - a[i]);
        let (lo, hi) = (a[i].min(a[i + 1]), a[i].max(a[i + 1]));
        for &j in &near {
            let (q, s) = (b[j], b[j + 1] - b[j]);
            let (q_lo, q_hi) = (b[j].min(b[j + 1]), b[j].max(b[j + 1]));
            if q_hi.x < lo.x || q_lo.x > hi.x || q_hi.y < lo.y || q_lo.y > hi.y {
                continue;
            }
            let denom = r.perp_dot(s);
            if denom.abs() < 1e-6 {
                continue;
            }
            let t = (q - p).perp_dot(s) / denom;
            let u = (q - p).perp_dot(r) / denom;
            if (0.0..1.0).contains(&t) && (0.0..1.0).contains(&u) {
                let point = p + r * t;
                if found.iter().all(|f| f.2.distance(point) > 1.0) {
                    found.push((i, j, point));
                }
            }
        }
    }
    found
}

/// Whether `p` lies within a piece's wood, between its top and bottom edges.
fn inside(piece: &Piece, p: Vec2) -> bool {
    match (edge_y_at(&piece.top, p.x), edge_y_at(&piece.bottom, p.x)) {
        (Some(top), Some(bottom)) => p.y > top && p.y < bottom,
        _ => false,
    }
}

fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let (t2, t3) = (t * t, t * t * t);
    0.5 * (2.0 * p1 + (p2 - p0) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// Unit normals along `edge`, each pointing away from the matching point on `other`.
fn outward_normals(edge: &[Vec2], other: &[Vec2], out: &mut Vec<Vec2>) {
    let n = edge.len();
    out.clear();
    for k in 0..n {
        let tangent = (edge[(k + 1).min(n - 1)] - edge[k.saturating_sub(1)]).normalize_or_zero();
        let normal = tangent.perp();
        out.push(if normal.dot(edge[k] - other[k]) < 0.0 { -normal } else { normal });
    }
}

/// Outward normals for a cap's corners, scaled (mitred) so the outline keeps its width round
/// each bend. `out` is the side the cap bulges toward.
fn miter_normals(chain: &mut [CapPoint], out: Vec2) {
    let n = chain.len();
    let span = chain[n - 1].pos - chain[0].pos;
    // Outward stays on the same side of the direction of travel all the way round
    let side = if span.perp().dot(out) >= 0.0 { 1.0 } else { -1.0 };
    let edge_normal = |a: Vec2, b: Vec2| (b - a).normalize_or_zero().perp() * side;
    for i in 1..n - 1 {
        let n1 = edge_normal(chain[i - 1].pos, chain[i].pos);
        let n2 = edge_normal(chain[i].pos, chain[i + 1].pos);
        let mid = (n1 + n2).normalize_or_zero();
        chain[i].normal = mid / mid.dot(n1).max(0.35);
    }
}

fn grain_uv(along: f32, f: f32) -> Vec2 {
    vec2(along / GRAIN_W as f32, (GRAIN_PLAIN_ROWS + f * (GRAIN_H as f32 - GRAIN_PLAIN_ROWS)) / GRAIN_H as f32)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}

/// The segment of `edge` spanning `x`, if any.
fn segment_at(edge: &[Vec2], x: f32) -> Option<usize> {
    edge.windows(2).position(|w| w[0].x != w[1].x && (x - w[0].x) * (x - w[1].x) <= 0.0)
}

/// The height of `edge` at `x`, if it spans it.
fn edge_y_at(edge: &[Vec2], x: f32) -> Option<f32> {
    segment_at(edge, x).map(|i| {
        let (a, b) = (edge[i], edge[i + 1]);
        a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x)
    })
}

/// Where `edge` first crosses `boundary`, coming up through it from below (`up`) or down through
/// it from above: the segment of `edge` it crosses on, and the crossing point.
fn crosses(edge: &[Vec2], boundary: &[Vec2], up: bool) -> Option<(usize, Vec2)> {
    let depth = |p: Vec2| edge_y_at(boundary, p.x).map(|y| if up { p.y - y } else { y - p.y });
    let mut before = depth(edge[0]);
    for k in 1..edge.len() {
        let now = depth(edge[k]);
        if let (Some(d0), Some(d1)) = (before, now) {
            if d0 > 0.0 && d1 <= 0.0 {
                return Some((k - 1, edge[k - 1].lerp(edge[k], d0 / (d0 - d1))));
            }
        }
        before = now;
    }
    None
}

/// The point `dist` along `edge` from `from` (which lies on segment `seg`), heading toward its
/// later samples or back toward its earlier ones.
fn walk(edge: &[Vec2], seg: usize, from: Vec2, dist: f32, forward: bool) -> Vec2 {
    let mut left = dist;
    let mut at = from;
    let mut visit = |next: Vec2| {
        let step = at.distance(next);
        if step >= left && step > 0.0 {
            at = at.lerp(next, left / step);
            return true;
        }
        left -= step;
        at = next;
        false
    };
    if forward {
        for &next in &edge[seg + 1..] {
            if visit(next) {
                break;
            }
        }
    } else {
        for &next in edge[..=seg].iter().rev() {
            if visit(next) {
                break;
            }
        }
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fork that goes down with its parent is joined to the piece it falls with, not the stub
    /// left on the tree, so its root still fades into the wood it grows from (at most a sample at
    /// the edge of the fade shows whole). One left on the stub stays joined to the stub.
    #[test]
    fn a_fallen_fork_blends_into_the_piece_it_fell_with() {
        let mut checked = 0;
        for level in 1..=5 {
            for seed in 0..2 {
                let (_, base, mut rng) = crate::world::build_level(&crate::level::endless(level, 0), seed);
                for fork in (0..base.len()).filter(|&i| base[i].attachment.is_some()) {
                    let attachment = base[fork].attachment.unwrap();
                    let parent = attachment.parent;
                    // Snapped just inside the fork, and as short as it can (past most forks)
                    for at_fork in [true, false] {
                        let mut limbs = base.clone();
                        if at_fork {
                            let branch = &mut limbs[parent].branch;
                            branch.snap_distance = (attachment.node as f32 - 0.5) * branch.segment_len;
                            branch.snap();
                        } else {
                            let at = limbs[parent].base().x;
                            limbs[parent].snap_at(at);
                        }
                        for step in 0..12 {
                            crate::world::step_limbs(&mut limbs, 0.0, step as f32 * STEP, &mut rng);
                        }
                        let fell = limbs[fork].fallen;
                        assert_eq!(fell, limbs[parent].branch.broken_at.unwrap() < attachment.node);
                        let mut pieces = Vec::new();
                        limbs[parent].add_pieces(parent, &mut pieces);
                        limbs[fork].add_pieces(fork, &mut pieces);
                        // The piece of the parent the fork grows out of: the one that broke off, or the stub
                        let parent_pieces: Vec<usize> = (0..pieces.len()).filter(|&i| pieces[i].0 == parent).collect();
                        let grows_from = if fell { *parent_pieces.last().unwrap() } else { parent_pieces[0] };
                        let (f, p) = (first_piece(&pieces, fork).unwrap(), parent_piece(&limbs, &pieces, attachment).unwrap());
                        let context = format!("level {level} seed {seed} fork {fork} fell {fell}");
                        assert_eq!(p, grows_from, "{context}");
                        let way = Emerges::find(&pieces[f].1, &pieces[p].1);
                        let (before, after) = pieces.split_at_mut(f);
                        tuck_under(&mut after[0].1, &before[p].1, way, limbs[fork].thickness);
                        for (_, piece) in &mut pieces {
                            piece.finish();
                        }
                        let palettes: Vec<Palette> = pieces.iter().map(|(id, _)| Palette::for_limb(&limbs[*id])).collect();
                        let join = join(&pieces[f].1, &pieces[p].1, (f, p), way, limbs[fork].thickness, &palettes);
                        let root = &pieces[f].1;
                        let opaque = (0..root.top.len())
                            .take_while(|&k| root.along[k] - root.along[0] < join.blend.1)
                            .map(|k| (k, root.top[k].lerp(root.bottom[k], 0.5)))
                            .filter(|&(k, at)| inside(&pieces[grows_from].1, at) && join.opacity(root, &pieces[p].1, k, at) >= 0.999)
                            .count();
                        assert!(opaque <= 1, "{context}: {opaque} samples of the root drawn whole inside the parent");
                        checked += fell as usize;
                    }
                }
            }
        }
        assert!(checked > 20, "only {checked} forks fell");
    }
}
