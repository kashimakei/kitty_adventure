//! One level in play: the trees and their limbs, pickups, enemies, Kitty and the camera.

use std::f32::consts::TAU;

use macroquad::prelude::*;

use crate::camera::{Camera, View};
use crate::consts::*;
use crate::effects::Effects;
use crate::enemies::{self, Enemy, EnemyCtx, KittyView, Projectile, Sfx, Spawner};
use crate::input::Controls;
use crate::items::{Item, ItemKind};
use crate::kitty::{Kitty, Push, Support};
use crate::level::{self, Difficulty, LevelPlan};
use crate::limb::{self, Attachment, Limb, LimbId, LimbSpec};
use crate::rng::Rng;
use crate::sprites::Assets;
use crate::story::{self, Friend};
use crate::tree::{self, Tree};

/// What happened during a step that the game flow cares about.
#[derive(PartialEq)]
pub enum Outcome {
    Playing,
    Rescued,
    OutOfHearts,
}

pub struct World {
    pub level: usize,
    pub lap: usize,
    pub difficulty: Difficulty,
    pub friend: Friend,
    trees: Vec<Tree>,
    bark: Texture2D,
    /// Bark grain running along the limbs.
    grain: Texture2D,
    pub limbs: Vec<Limb>,
    pub goal: LimbId,
    items: Vec<Item>,
    enemies: Vec<Box<dyn Enemy>>,
    projectiles: Vec<Projectile>,
    spawner: Spawner,
    pub kitty: Kitty,
    pub camera: Camera,
    /// Where the camera and Kitty were before the last step, so frames drawn between steps (the
    /// screen may refresh at 90 or 120 Hz) can blend between the two instead of stuttering.
    prev_camera: Vec2,
    prev_kitty: Vec2,
    effects: Effects,
    rng: Rng,
    pub time: f32,
    pub breeze: f32,
    /// Points earned on this level so far (the game adds them to the total when it's cleared).
    pub points: u32,
    pub collected_points: u32,
    pub hits_taken: u32,
    pub elapsed: f32,
    best_height: f32,
    pub rescued: bool,
    rescue_time: f32,
    pub sfx: Vec<Sfx>,
    start_x: f32,
    top_y: f32,
}

impl World {
    pub fn new(level: usize, lap: usize, seed: u64) -> Self {
        let plan = level::generate(level, lap, seed);
        let difficulty = level::difficulty(level, lap);
        let mut rng = Rng::new(seed ^ 0xC0FFEE);
        let mut limbs = build_limbs(&plan, &mut rng);
        for limb in &mut limbs {
            limb.decorate(&mut rng, difficulty.autumn);
        }
        let goal = limbs.iter().position(|l| l.is_goal).unwrap_or(0);

        let trees = plan.tree_xs.iter().map(|&x| Tree::new(x, plan.top_y, &mut rng)).collect();
        let items = plan.items.iter().map(|i| Item::new(i.kind, i.limb, i.frac, rng.range(0.0, TAU))).collect();
        let mut enemies: Vec<Box<dyn Enemy>> = Vec::new();
        for &(limb, frac) in &plan.squirrel_spots {
            enemies.push(Box::new(enemies::squirrel::Squirrel::new(limb, frac)));
        }
        for &(limb, frac) in &plan.snake_spots {
            enemies.push(Box::new(enemies::snake::Snake::new(limb, frac)));
        }
        let spawner = Spawner::new(&difficulty, &mut rng);
        let start_x = WORLD_W / 2.0;
        let kitty = Kitty::new(start_x);
        let mut camera = Camera::new(start_x);
        camera.snap_to(kitty.rect());
        let bark = tree::make_bark_texture(&mut rng);
        Self {
            level,
            lap,
            friend: Friend::for_level(level),
            difficulty,
            trees,
            bark,
            grain: limb::make_grain_texture(),
            limbs,
            goal,
            items,
            enemies,
            projectiles: Vec::new(),
            spawner,
            prev_camera: vec2(camera.x, camera.y),
            prev_kitty: vec2(kitty.x, kitty.y),
            kitty,
            camera,
            effects: Effects::default(),
            rng,
            time: 0.0,
            breeze: 0.0,
            points: 0,
            collected_points: 0,
            hits_taken: 0,
            elapsed: 0.0,
            best_height: 0.0,
            rescued: false,
            rescue_time: 0.0,
            sfx: Vec::new(),
            start_x,
            top_y: plan.top_y,
        }
    }

    /// Where the friend's nest sits, near the goal limb's tip.
    pub fn nest_position(&self) -> Vec2 {
        let limb = &self.limbs[self.goal];
        let (lo, hi) = limb.walk_range();
        let x = if limb.direction > 0.0 { lo + (hi - lo) * 0.85 } else { hi - (hi - lo) * 0.85 };
        vec2(x, limb.surface_y(x).unwrap_or(limb.tip().y))
    }

    fn kitty_view(&self) -> KittyView {
        KittyView {
            x: self.kitty.x,
            y: self.kitty.y,
            rect: self.kitty.rect(),
            body: self.kitty.body(),
            standing_on: self.kitty.standing_on,
            hanging_on: self.kitty.hanging_on,
            airborne: self.kitty.airborne(),
        }
    }

    /// Apply loads, kicks and jolts; a hard kick shakes a few leaves loose.
    fn apply(&mut self, pushes: Vec<Push>) {
        for at in apply_pushes(&mut self.limbs, pushes) {
            self.effects.shake_leaves(at, 2, &mut self.rng);
        }
    }

    /// Kitty lost all her hearts: back to the start with full hearts.
    pub fn respawn_kitty(&mut self) {
        self.kitty.respawn(self.start_x);
        self.camera.snap_to(self.kitty.rect());
        self.prev_camera = vec2(self.camera.x, self.camera.y);
        self.prev_kitty = vec2(self.kitty.x, self.kitty.y);
        self.projectiles.clear();
        // Give her room to breathe
        let start = vec2(self.start_x, GROUND_Y);
        self.enemies.retain(|e| e.position().distance(start) > 350.0);
    }

    pub fn step(&mut self, controls: &Controls) -> Outcome {
        self.prev_camera = vec2(self.camera.x, self.camera.y);
        self.prev_kitty = vec2(self.kitty.x, self.kitty.y);
        self.time += STEP;
        self.breeze = breeze_at(self.time, self.difficulty.wind);

        if self.rescued {
            // The friend cheers while Kitty waits, still riding (and weighing down) the branch;
            // the world keeps swaying
            self.rescue_time += STEP;
            self.kitty.resolve(&self.limbs);
            let pushes = std::mem::take(&mut self.kitty.pushes);
            self.apply(pushes);
            self.step_limbs();
            self.effects.drift(self.camera.view_rect(), &mut self.rng);
            return Outcome::Playing;
        }
        self.elapsed += STEP;

        let kitty = step_kitty(&mut self.kitty, &mut self.limbs, controls, self.breeze);
        if kitty.jumped {
            self.sfx.push(Sfx::Jump);
        }
        for at in kitty.shakes {
            self.effects.shake_leaves(at, 2, &mut self.rng);
        }

        self.step_limbs();

        // Pickups
        let body = self.kitty.body();
        for item in &mut self.items {
            if item.taken {
                continue;
            }
            match item.rect(&self.limbs, self.time) {
                None => item.taken = true, // fell away with a snapped limb
                Some(rect) if rect.overlaps(&body) => {
                    item.taken = true;
                    let at = rect.center();
                    self.effects.sparkle(at);
                    let (points, sfx) = match item.kind {
                        ItemKind::Fish => (SCORE_FISH, Sfx::Collect),
                        ItemKind::Gold => (SCORE_GOLD, Sfx::Collect),
                        ItemKind::Heart if self.kitty.hearts < MAX_HEARTS => {
                            self.kitty.hearts += 1;
                            self.effects.popup(at, "+1 heart", Color::new(1.0, 0.4, 0.45, 1.0));
                            (0, Sfx::Heart)
                        }
                        ItemKind::Heart => (SCORE_SPARE_HEART, Sfx::Heart),
                    };
                    if points > 0 {
                        self.points += points;
                        self.collected_points += points;
                        self.effects.popup(at, format!("+{points}"), Color::new(1.0, 0.9, 0.3, 1.0));
                    }
                    self.sfx.push(sfx);
                }
                Some(_) => {}
            }
        }

        // Enemies
        let view = self.camera.view_rect();
        let kitty_view = self.kitty_view();
        let mut pushes = Vec::new();
        {
            let mut ctx = EnemyCtx {
                kitty: kitty_view,
                limbs: &self.limbs,
                pushes: &mut pushes,
                projectiles: &mut self.projectiles,
                view,
                rng: &mut self.rng,
                difficulty: &self.difficulty,
                sfx: &mut self.sfx,
            };
            for enemy in &mut self.enemies {
                enemy.step(&mut ctx);
            }
        }
        self.apply(pushes);
        self.enemies.retain(|e| e.alive());
        self.spawner.step(STEP, &mut self.enemies, &kitty_view, view, &self.difficulty, &mut self.rng);
        for acorn in &mut self.projectiles {
            acorn.step(STEP, view);
        }
        self.projectiles.retain(|p| p.alive);

        // Score for new height reached
        let height = -self.kitty.y;
        if height > self.best_height + SCORE_HEIGHT_STEP {
            let steps = ((height - self.best_height) / SCORE_HEIGHT_STEP) as u32;
            self.best_height += steps as f32 * SCORE_HEIGHT_STEP;
            self.points += steps;
        }

        // Reaching the friend's nest
        if self.kitty.standing_on == Some(Support::Limb(self.goal)) && (self.kitty.x - self.nest_position().x).abs() < 70.0 {
            self.rescued = true;
            // She sits and waits now, whatever was held
            self.kitty.forget_input();
            self.sfx.push(Sfx::Rescue);
            self.effects.sparkle(self.nest_position() - vec2(0.0, 40.0));
            return Outcome::Rescued;
        }

        self.camera.follow(self.kitty.rect());
        self.effects.drift(self.camera.view_rect(), &mut self.rng);
        if self.kitty.hearts == 0 {
            return Outcome::OutOfHearts;
        }
        Outcome::Playing
    }

    fn step_limbs(&mut self) {
        step_limbs(&mut self.limbs, self.breeze, self.time, &mut self.rng);
        let mut snapped = false;
        for limb in self.limbs.iter().filter(|l| l.just_snapped) {
            // One crack, even if a fork goes down with its parent
            if !snapped {
                self.sfx.push(Sfx::Snap);
                snapped = true;
            }
            self.effects.shake_leaves(limb.tip(), 6, &mut self.rng);
        }
    }

    /// Stomps first (feet landing on an enemy), then anything that strikes Kitty's body.
    /// Hits need the frames' damage masks, so the game calls this with the assets after `step`.
    pub fn collide_with_assets(&mut self, assets: &Assets) {
        if self.rescued {
            return;
        }
        if let Some(i) = stomp(&self.kitty, &mut self.enemies) {
            let (at, points) = (self.enemies[i].position(), self.enemies[i].stomp_points());
            self.points += points;
            self.effects.sparkle(at);
            self.effects.popup(at, format!("+{points}"), Color::new(1.0, 0.9, 0.3, 1.0));
            self.sfx.push(Sfx::Stomp);
            self.kitty.bounce();
            return;
        }
        let body = self.kitty.body();
        let mut hit_from = None;
        for enemy in &self.enemies {
            if enemy.strikes(assets, body) {
                hit_from = Some(enemy.position().x);
                break;
            }
        }
        if hit_from.is_none() {
            for acorn in &mut self.projectiles {
                if acorn.rect().overlaps(&body) {
                    acorn.alive = false;
                    hit_from = Some(acorn.pos.x);
                    break;
                }
            }
        }
        if let Some(x) = hit_from {
            if self.kitty.hurt(x) {
                self.hits_taken += 1;
                self.sfx.push(Sfx::Hurt);
            }
        }
    }

    /// Advance animations by a drawn frame's time.
    pub fn animate(&mut self, dt: f32) {
        self.kitty.animate(dt, &mut self.rng);
        for enemy in &mut self.enemies {
            enemy.animate(dt);
        }
        let view = self.camera.view_rect();
        self.effects.update(dt, self.breeze, view);
    }

    /// `blend` is how far the clock is between the last step and the next (0..1).
    pub fn draw(&self, assets: &Assets, view: &View, blend: f32) {
        let blend = blend.clamp(0.0, 1.0);
        let at = self.prev_camera.lerp(vec2(self.camera.x, self.camera.y), blend);
        let camera = Camera { x: at.x, y: at.y };
        let kitty_offset = self.prev_kitty.lerp(vec2(self.kitty.x, self.kitty.y), blend) - vec2(self.kitty.x, self.kitty.y);
        // The sky and the painted scenery behind the trees, sliding at their own paces
        set_camera(&view.screen_camera());
        let climbed = camera.height();
        let height = (climbed / (-self.top_y).max(1.0)).clamp(0.0, 1.0);
        assets.scenery.draw(camera.x, climbed, height, self.time);
        set_camera(&camera.world_camera(view));

        let view_rect = camera.view_rect();
        let visible = |r: Rect| r.overlaps(&Rect::new(view_rect.x - 120.0, view_rect.y - 120.0, view_rect.w + 240.0, view_rect.h + 240.0));
        // Leaves behind the wood, the wood (each limb merged with its forks), then trunks over
        // the limbs' roots (with the wood brought back out over their edges), then leaves in front
        for limb in self.limbs.iter().filter(|l| visible(l.bounds())) {
            limb.draw_back(assets, self.time, self.breeze);
        }
        limb::draw_wood(&self.limbs, |l| visible(l.bounds()), &self.grain);
        for tree in &self.trees {
            tree.draw(assets, &self.bark, view_rect);
        }
        limb::draw_roots(&self.limbs, |l| visible(l.bounds()), &self.grain);
        for limb in self.limbs.iter().filter(|l| visible(l.bounds())) {
            limb.draw_front(assets, self.time, self.breeze);
        }
        // Ground
        draw_rectangle(-50.0, GROUND_Y, WORLD_W + 100.0, GROUND_DEPTH, SOIL);
        draw_rectangle(-50.0, GROUND_Y, WORLD_W + 100.0, 18.0, GRASS);
        draw_rectangle(-50.0, GROUND_Y + 14.0, WORLD_W + 100.0, 6.0, GRASS_DARK);

        // The friend waiting in the nest
        let nest = self.nest_position();
        assets.items.draw("nest_0", nest + vec2(0.0, 14.0), 1.0, WHITE);
        if self.rescued {
            let frame = self.friend.cheer_frame((self.rescue_time * 6.0) as usize);
            let hop = ((self.rescue_time * 6.0).sin().abs()) * 18.0;
            assets.friends.draw(&frame, nest - vec2(0.0, 4.0 + hop), 1.0, WHITE);
        } else {
            let bob = (self.time * 2.0).sin() * 2.0;
            assets.friends.draw(&self.friend.help_frame(), nest - vec2(0.0, 4.0 + bob), -1.0, WHITE);
        }

        for item in &self.items {
            item.draw(assets, &self.limbs, self.time);
        }
        // Anything mid-strike draws over Kitty so a lunging jaw or beak isn't hidden behind her
        for enemy in self.enemies.iter().filter(|e| !e.in_front()) {
            enemy.draw(assets);
        }
        for acorn in &self.projectiles {
            acorn.draw(assets);
        }
        // The charge ring wraps round her: its far half behind her, its near half in front
        self.kitty.draw_ring(kitty_offset, self.time, false);
        self.kitty.draw(assets, kitty_offset);
        self.kitty.draw_ring(kitty_offset, self.time, true);
        for enemy in self.enemies.iter().filter(|e| e.in_front()) {
            enemy.draw(assets);
        }
        self.effects.draw(assets);

        if self.rescued {
            set_camera(&view.screen_camera());
            story::draw_rescue_banner(assets, &story::level_info(self.level), self.rescue_time);
        }
    }

    /// The rescue has played long enough to move on.
    pub fn rescue_done(&self) -> bool {
        self.rescued && self.rescue_time > 3.0
    }

    pub fn top_y(&self) -> f32 {
        self.top_y
    }
}

/// The level's limbs: branches first, then forks, which need their parent's shape to know where
/// they sprout. Settled, so nothing springs about when play starts.
pub fn build_limbs(plan: &LevelPlan, rng: &mut Rng) -> Vec<Limb> {
    let half = TRUNK_WIDTH / 2.0;

    // Branches first, then forks, which need their parent's shape to know where they sprout
    let mut limbs: Vec<Limb> = plan.branches.iter().map(|b| {
        let mut limb = Limb::new(&LimbSpec {
            start: vec2(plan.tree_xs[b.tree], b.y),
            direction: b.direction,
            length: b.length,
            thickness: b.thickness,
            stiffness: b.stiffness,
            gnarl: b.gnarl,
            rise: 0.0,
            // The plan only sets a strength for thin branches; sturdy ones are far stronger but
            // can still be snapped by bouncing on them too hard. The goal, with the friend's nest,
            // never snaps.
            strength: if b.goal {
                0.0
            } else if b.strength > 0.0 {
                b.strength
            } else {
                STURDY_BRANCH_STRENGTH * KITTY_WEIGHT
            },
            thin: b.strength > 0.0,
            tree: b.tree,
            trunk_half: half,
            attachment: None,
            seed: rng.next_u64(),
        });
        limb.is_goal = b.goal;
        limb.settle();
        limb
    }).collect();
    for fork in &plan.forks {
        let parent = &limbs[fork.parent];
        let node = ((parent.segments() as f32 * fork.node_frac) as usize).clamp(2, parent.segments() - 1);
        // Sprout from inside the parent's wood so the join is hidden
        let start = parent.top_point(node) + vec2(0.0, parent.thickness * 0.25);
        let mut limb = Limb::new(&LimbSpec {
            start,
            direction: parent.direction,
            length: fork.length,
            thickness: fork.thickness,
            stiffness: fork.stiffness,
            gnarl: fork.gnarl,
            rise: fork.rise,
            strength: STURDY_BRANCH_STRENGTH * KITTY_WEIGHT,
            thin: false,
            tree: parent.tree,
            trunk_half: 0.0,
            attachment: Some(Attachment { parent: fork.parent, node }),
            seed: rng.next_u64(),
        });
        limb.track_anchor(parent.top_point(node));
        limb.settle();
        limbs.push(limb);
    }
    // How much each limb gives under Kitty, measured against its own sag; forks also ride on
    // their parent's bend
    for i in 0..limbs.len() {
        let base_give = limbs[i].attachment.map_or(0.0, |a| {
            limbs[a.parent].give_at(limbs[a.parent].top_point(a.node).x) * FORK_LOAD_SHARE
        });
        limbs[i].calibrate_give(base_give);
    }
    limbs
}

/// One physics step for every limb. Parents come before their forks, so forks follow where the
/// parent is now. A fork goes down with its parent if that snapped off between the trunk and
/// the fork; one left on a stub stays, its base giving no more than the stub does now.
pub fn step_limbs(limbs: &mut [Limb], breeze: f32, time: f32, rng: &mut Rng) {
    for i in 0..limbs.len() {
        step_limb(limbs, i, breeze, time, rng);
    }
}

/// One physics step for limb `i` (after its parent, if it's a fork).
pub fn step_limb(limbs: &mut [Limb], i: LimbId, breeze: f32, time: f32, rng: &mut Rng) {
    let mut anchor = None;
    if let Some(a) = limbs[i].attachment {
        let parent = &limbs[a.parent];
        let at = parent.top_point(a.node);
        let fell = parent.fallen || parent.branch.broken_at.is_some_and(|seg| seg < a.node);
        let base_give = (parent.just_snapped && !fell).then(|| parent.give_at(at.x) * FORK_LOAD_SHARE);
        if fell {
            limbs[i].fall_with_parent();
        }
        if let Some(give) = base_give {
            limbs[i].set_base_give(give);
        }
        anchor = Some(at);
    }
    limbs[i].step(breeze, time, anchor, rng);
}

/// What a step of Kitty did that the world makes sounds and leaves fly for.
#[derive(Default)]
pub struct KittyStep {
    pub jumped: bool,
    pub landed: Option<Support>,
    /// Where a limb took a kick hard enough to shake leaves loose.
    pub shakes: Vec<Vec2>,
}

/// Kitty coming down with her feet on an enemy that can be stomped right now, on a branch or in
/// the air: it's stomped, and its index returned. One per step.
pub fn stomp(kitty: &Kitty, enemies: &mut [Box<dyn Enemy>]) -> Option<usize> {
    if kitty.velocity <= 1.0 || !kitty.airborne() {
        return None;
    }
    let feet = kitty.stomp_feet();
    let i = enemies.iter().position(|e| e.stomp_box().is_some_and(|target| target.overlaps(&feet)))?;
    enemies[i].stomped();
    Some(i)
}

/// One step of Kitty: her jump and drop input, moving, then riding, catching or landing on
/// whatever is under her, with her weight and kicks passed on to the limbs (which the caller
/// steps afterwards). World and the physics tests both go through here.
pub fn step_kitty(kitty: &mut Kitty, limbs: &mut [Limb], controls: &Controls, breeze: f32) -> KittyStep {
    kitty.input(controls, limbs);
    kitty.step(controls.move_x, breeze);
    let landed = kitty.resolve(limbs);
    let shakes = apply_pushes(limbs, std::mem::take(&mut kitty.pushes));
    KittyStep { jumped: kitty.jumped, landed, shakes }
}

/// Apply loads, kicks and jolts (a fork passes some of its load and jolts on to its parent).
/// Returns where a kick was hard enough to shake leaves loose.
pub fn apply_pushes(limbs: &mut [Limb], pushes: Vec<Push>) -> Vec<Vec2> {
    let mut shakes = Vec::new();
    for push in pushes {
        match push {
            Push::Load { limb, x, weight } => load_limb(limbs, limb, x, weight),
            Push::Kick { limb, x, speed } => {
                limbs[limb].kick(x, speed);
                if speed > SHAKE_KICK {
                    if let Some(y) = limbs[limb].surface_y(x) {
                        shakes.push(vec2(x, y));
                    }
                }
            }
            Push::Jolt { limb, x, force, fatigue } => jolt_limb(limbs, limb, x, force, fatigue),
        }
    }
    shakes
}

/// A hard landing's jolt at `x`; like weight, a fork passes most of it on to its parent (where
/// it grows out).
pub fn jolt_limb(limbs: &mut [Limb], limb: LimbId, x: f32, force: f32, fatigue: f32) {
    limbs[limb].jolt(x, force, fatigue);
    if let Some(Attachment { parent, node }) = limbs[limb].attachment {
        let px = limbs[parent].top_point(node).x;
        limbs[parent].jolt(px, force * FORK_LOAD_SHARE, fatigue * FORK_LOAD_SHARE);
    }
}

/// Weight on a limb; a fork passes most of it on to its parent.
pub fn load_limb(limbs: &mut [Limb], limb: LimbId, x: f32, weight: f32) {
    limbs[limb].branch.apply_load(x, weight);
    if let Some(Attachment { parent, node }) = limbs[limb].attachment {
        let px = limbs[parent].top_point(node).x;
        limbs[parent].branch.apply_load(px, weight * FORK_LOAD_SHARE);
    }
}

/// The wind at `time`: a steady breeze with slow gusts that grow with the level's wind.
pub fn breeze_at(time: f32, wind: f32) -> f32 {
    1.5 + wind * ((0.5 * time).sin() + 0.3 * (1.7 * time + 1.0).sin())
}
