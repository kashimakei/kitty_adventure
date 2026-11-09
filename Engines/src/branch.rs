use crate::math::Vec2;
use pyo3::prelude::*;
use std::f32::consts::PI;

#[pyclass]
#[derive(Clone, Copy, Debug)]
pub struct Node {
    #[pyo3(get, set)]
    pub pos: Vec2,
    #[pyo3(get, set)]
    pub old_pos: Vec2,
    #[pyo3(get, set)]
    pub acc: Vec2,
    #[pyo3(get, set)]
    pub radius: f32,
    #[pyo3(get, set)]
    pub pinned: bool,
}

#[pymethods]
impl Node {
    #[new]
    pub fn new(pos: Vec2, radius: f32, pinned: bool) -> Self {
        Self {
            pos,
            old_pos: pos,
            acc: Vec2::zero(),
            radius,
            pinned,
        }
    }

    pub fn update(&mut self, dt: f32, friction: f32) {
        if self.pinned { return; }
        let vel = self.pos.sub_val(self.old_pos).mul_val(friction);
        self.old_pos = self.pos;
        self.pos = self.pos.add_val(vel).add_val(self.acc.mul_val(dt * dt));
        self.acc = Vec2::zero();
    }
}

#[pyclass]
#[derive(Clone, Copy, Debug)]
pub struct Constraint {
    #[pyo3(get, set)]
    pub node_a: usize,
    #[pyo3(get, set)]
    pub node_b: usize,
    #[pyo3(get, set)]
    pub target_length: f32,
    #[pyo3(get, set)]
    pub stiffness: f32,
}

#[pymethods]
impl Constraint {
    #[new]
    pub fn new(node_a: usize, node_b: usize, target_length: f32, stiffness: f32) -> Self {
        Self { node_a, node_b, target_length, stiffness }
    }
}

/// Length at which the bend amplitudes below look natural; shorter branches get proportionally
/// gentler curves so they don't crumple into zigzags.
const REFERENCE_LENGTH: f32 = 220.0;
/// Seconds of sustained overload before a branch snaps, so a hard landing alone won't break it.
const BREAK_RESPONSE: f32 = 0.3;

#[pyclass]
pub struct Branch {
    #[pyo3(get, set)]
    pub nodes: Vec<Node>,
    #[pyo3(get, set)]
    pub constraints: Vec<Constraint>,
    /// Number of segments; top contour is nodes[0..=segments], bottom contour follows.
    #[pyo3(get)]
    pub segments: usize,
    /// +1.0 grows to the right of `start`, -1.0 grows to the left.
    #[pyo3(get)]
    pub direction: f32,
    #[pyo3(get)]
    pub segment_len: f32,
    /// Total sustained load (sum of `apply_load` forces) the branch holds before snapping.
    /// 0 means unbreakable.
    #[pyo3(get, set)]
    pub strength: f32,
    /// Distance from the base where the branch snaps off.
    #[pyo3(get, set)]
    pub snap_distance: f32,
    /// Smoothed total load the branch is carrying right now; compare with `strength`.
    #[pyo3(get)]
    pub stress: f32,
    /// Segment that has snapped, if any. Everything past it has fallen free.
    #[pyo3(get)]
    pub broken_at: Option<usize>,
    /// External forces (loads, wind) accumulated between physics ticks.
    /// Applied on every sub-step of the next `update_physics`, then cleared.
    pending: Vec<Vec2>,
    /// Load accumulated by `apply_load` since the last physics tick.
    load_total: f32,
}

#[pymethods]
impl Branch {
    #[new]
    #[pyo3(signature = (start, length, base_depth, segments, stiffness, gnarl=1.0, direction=1.0, seed=0))]
    pub fn new_tree_branch(start: Vec2, length: f32, base_depth: f32, segments: usize, stiffness: f32, gnarl: f32, direction: f32, seed: u64) -> Self {
        let direction = if direction < 0.0 { -1.0 } else { 1.0 };
        let segments = segments.max(2);
        let segment_len = length / segments as f32;

        // Per-branch variety: where the wiggle sits and how much the branch angles upward
        let mut rng = Rng::new(seed);
        let phase = rng.next_f32() * 2.0 * PI;
        let tilt = -0.12 + rng.next_f32() * 0.14;

        // Top contour (the walkable surface). Bends scale with length so short branches stay smooth.
        let scale = length / REFERENCE_LENGTH;
        // Keep at least five segments per wiggle so high gnarl reads as knobbly, not zigzag
        let wiggle_cycles = (2.0 * gnarl).min(segments as f32 / 5.0);
        let top: Vec<Vec2> = (0..=segments).map(|i| {
            let t = i as f32 / segments as f32;
            let bend = scale * gnarl * (
                (t * PI * 1.5 * gnarl.min(1.0)).sin() * 30.0
                + (t * 2.0 * PI * wiggle_cycles + phase).sin() * 10.0 * t
                + t * t * 40.0
            ) + tilt * length * t;
            Vec2::new(start.x + direction * i as f32 * segment_len, start.y + bend)
        }).collect();

        // Bottom contour sits a tapering thickness below the top, measured perpendicular to the
        // branch so steep sections keep their girth instead of pinching flat.
        let bottom: Vec<Vec2> = (0..=segments).map(|i| {
            let t = i as f32 / segments as f32;
            let depth = base_depth * (1.0 - 0.75 * t).max(0.1);
            let prev = top[i.saturating_sub(1)];
            let next = top[(i + 1).min(segments)];
            let tangent = next.sub_val(prev).normalize_val();
            let mut normal = Vec2::new(-tangent.y, tangent.x);
            if normal.y < 0.0 {
                normal = normal.mul_val(-1.0);
            }
            // The pinned root columns hang straight down so the base sits square inside the trunk
            if i <= 1 {
                normal = Vec2::new(0.0, 1.0);
            }
            top[i].add_val(normal.mul_val(depth))
        }).collect();

        let mut nodes = Vec::new();
        for (i, pos) in top.iter().chain(bottom.iter()).enumerate() {
            let column = i % (segments + 1);
            nodes.push(Node::new(*pos, 3.0, column <= 1));
        }

        let mut constraints = Vec::new();
        let bottom_offset = segments + 1;

        let get_dist = |nodes: &Vec<Node>, a: usize, b: usize| -> f32 {
            nodes[a].pos.sub_val(nodes[b].pos).length_val()
        };

        for i in 0..segments {
            let t1 = i;
            let t2 = i + 1;
            let b1 = i + bottom_offset;
            let b2 = i + 1 + bottom_offset;

            constraints.push(Constraint::new(t1, t2, get_dist(&nodes, t1, t2), stiffness));
            constraints.push(Constraint::new(b1, b2, get_dist(&nodes, b1, b2), stiffness));
            constraints.push(Constraint::new(t1, b1, get_dist(&nodes, t1, b1), stiffness));
            
            constraints.push(Constraint::new(t1, b2, get_dist(&nodes, t1, b2), stiffness));
            constraints.push(Constraint::new(b1, t2, get_dist(&nodes, b1, t2), stiffness));
        }
        constraints.push(Constraint::new(segments, segments + bottom_offset, get_dist(&nodes, segments, segments + bottom_offset), stiffness));

        let pending = vec![Vec2::zero(); nodes.len()];
        Self {
            nodes, constraints, segments, direction, segment_len,
            strength: 0.0, snap_distance: 0.0, stress: 0.0, broken_at: None,
            pending, load_total: 0.0,
        }
    }

    pub fn update_constraints(&mut self, iterations: usize) {
        for _ in 0..iterations {
            for c in &self.constraints {
                let delta = self.nodes[c.node_b].pos.sub_val(self.nodes[c.node_a].pos);
                let current_len = delta.length_val();
                if current_len == 0.0 { continue; }
                
                let diff = (current_len - c.target_length) / current_len;
                let offset = delta.mul_val(diff * 0.5 * c.stiffness);

                if !self.nodes[c.node_a].pinned {
                    self.nodes[c.node_a].pos = self.nodes[c.node_a].pos.add_val(offset);
                }
                if !self.nodes[c.node_b].pinned {
                    self.nodes[c.node_b].pos = self.nodes[c.node_b].pos.sub_val(offset);
                }
            }
        }
    }

    pub fn update_physics(&mut self, sub_steps: usize, dt: f32, gravity: f32, global_damping: f32) {
        let sub_dt = dt / sub_steps as f32;
        let gravity_vec = Vec2::new(0.0, gravity);

        for _ in 0..sub_steps {
            for (node, force) in self.nodes.iter_mut().zip(&self.pending) {
                node.acc = node.acc.add_val(gravity_vec).add_val(*force);
                node.update(sub_dt, global_damping);
            }
            self.update_constraints(4);
        }
        for force in &mut self.pending {
            *force = Vec2::zero();
        }

        self.stress += (self.load_total - self.stress) * (dt / BREAK_RESPONSE).min(1.0);
        self.load_total = 0.0;
        if self.strength > 0.0 && self.broken_at.is_none() && self.stress > self.strength {
            self.snap();
        }
    }

    /// Break the branch at `snap_distance` from its base. The outer part keeps simulating and
    /// falls away; only the stub stays attached and walkable.
    pub fn snap(&mut self) {
        if self.broken_at.is_some() {
            return;
        }
        let seg = ((self.snap_distance / self.segment_len) as usize).clamp(1, self.segments - 1);
        let columns = self.segments + 1;
        self.constraints.retain(|c| {
            let (a, b) = (c.node_a % columns, c.node_b % columns);
            a.min(b) > seg || a.max(b) <= seg
        });
        // Tip-heavy kick so the free piece swings down and tumbles instead of dropping flat
        for (i, node) in self.nodes.iter_mut().enumerate() {
            let column = i % columns;
            if column > seg {
                let reach = (column - seg) as f32 / (self.segments - seg) as f32;
                node.old_pos.y -= 1.5 * reach;
            }
        }
        self.broken_at = Some(seg);
        self.stress = 0.0;
    }

    /// Zero every node's velocity, e.g. after settling so a fresh level starts perfectly still.
    pub fn rest(&mut self) {
        for node in &mut self.nodes {
            node.old_pos = node.pos;
            node.acc = Vec2::zero();
        }
    }

    /// Y of the branch's top surface at world `x`, or None if `x` is past the base or tip.
    pub fn surface_y(&self, x: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        for i in 0..self.attached_segments() {
            if let Some(t) = self.segment_t(i, x) {
                let a = self.nodes[i].pos;
                let b = self.nodes[i + 1].pos;
                let y = a.y + (b.y - a.y) * t;
                best = Some(best.map_or(y, |cur: f32| cur.min(y)));
            }
        }
        best
    }

    /// Push down (positive `force`) on the branch at world `x`, e.g. a character standing on it.
    /// The force is split between the two nearest columns and lasts for the next `update_physics`.
    pub fn apply_load(&mut self, x: f32, force: f32) {
        let columns = self.columns_at(x);
        if !columns.is_empty() {
            self.load_total += force;
        }
        for (col, w) in columns {
            let bottom = col + self.segments + 1;
            self.pending[col].y += force * w;
            self.pending[bottom].y += force * w;
        }
    }

    /// Add a uniform force to every node for the next `update_physics` (wind, shaking).
    pub fn apply_force(&mut self, fx: f32, fy: f32) {
        let f = Vec2::new(fx, fy);
        for force in &mut self.pending {
            *force = force.add_val(f);
        }
    }

    /// Kick the branch at world `x` with a vertical velocity (pixels per sub-step), e.g. a landing.
    pub fn apply_impulse(&mut self, x: f32, dy: f32) {
        for (col, w) in self.columns_at(x) {
            for idx in [col, col + self.segments + 1] {
                let node = &mut self.nodes[idx];
                if !node.pinned {
                    node.old_pos.y -= dy * w;
                }
            }
        }
    }

    /// Move the whole branch rigidly without adding velocity (camera scrolling).
    pub fn translate(&mut self, dx: f32, dy: f32) {
        let d = Vec2::new(dx, dy);
        for node in &mut self.nodes {
            node.pos = node.pos.add_val(d);
            node.old_pos = node.old_pos.add_val(d);
        }
    }

    /// Top contour from base to tip as (x, y) tuples.
    pub fn top_contour(&self) -> Vec<(f32, f32)> {
        self.nodes[..=self.segments].iter().map(|n| (n.pos.x, n.pos.y)).collect()
    }

    /// Closed outline (top contour base->tip, then bottom contour tip->base) for polygon drawing.
    pub fn outline(&self) -> Vec<(f32, f32)> {
        let top = self.nodes[..=self.segments].iter();
        let bottom = self.nodes[self.segments + 1..].iter().rev();
        top.chain(bottom).map(|n| (n.pos.x, n.pos.y)).collect()
    }

    /// One closed outline per piece: the whole branch, or the stub and the fallen part once snapped.
    pub fn outlines(&self) -> Vec<Vec<(f32, f32)>> {
        let columns = self.segments + 1;
        let piece = |from: usize, to: usize| -> Vec<(f32, f32)> {
            let top = self.nodes[from..=to].iter();
            let bottom = self.nodes[columns + from..=columns + to].iter().rev();
            top.chain(bottom).map(|n| (n.pos.x, n.pos.y)).collect()
        };
        match self.broken_at {
            None => vec![piece(0, self.segments)],
            Some(seg) => vec![piece(0, seg), piece(seg + 1, self.segments)],
        }
    }

    /// Axis-aligned bounds as (min_x, min_y, max_x, max_y).
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for n in &self.nodes {
            b.0 = b.0.min(n.pos.x);
            b.1 = b.1.min(n.pos.y);
            b.2 = b.2.max(n.pos.x);
            b.3 = b.3.max(n.pos.y);
        }
        b
    }
}

impl Branch {
    /// Fraction along top segment `i` at which it crosses world `x`, if it does.
    fn segment_t(&self, i: usize, x: f32) -> Option<f32> {
        let xa = self.nodes[i].pos.x;
        let xb = self.nodes[i + 1].pos.x;
        if xa == xb || (x - xa) * (x - xb) > 0.0 {
            return None;
        }
        Some((x - xa) / (xb - xa))
    }

    /// Segments still connected to the trunk.
    fn attached_segments(&self) -> usize {
        self.broken_at.unwrap_or(self.segments)
    }

    /// The (column, weight) pairs straddling world `x`, empty if `x` is off the attached branch.
    fn columns_at(&self, x: f32) -> Vec<(usize, f32)> {
        (0..self.attached_segments())
            .find_map(|i| self.segment_t(i, x).map(|t| vec![(i, 1.0 - t), (i + 1, t)]))
            .unwrap_or_default()
    }
}

/// Tiny xorshift so branch shapes vary per seed without pulling in a rand crate.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}
