//! Enemies share one interface so the world can step, draw and collide them alike. Only their
//! damaging pixels (paws, jaws, beaks, talons; see sprites.rs) hurt Kitty, and only on her body.
//! Small ones (squirrels, snakes, crows) can be stomped from above.

pub mod crow;
pub mod dog;
pub mod eagle;
pub mod snake;
pub mod squirrel;

use macroquad::prelude::*;

use crate::kitty::{Push, Support};
use crate::level::Difficulty;
use crate::limb::{Limb, LimbId};
use crate::rng::Rng;
use crate::sprites::Assets;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Dog,
    Eagle,
    Squirrel,
    Snake,
    Crow,
}

/// Sounds the world plays for things that happened this step.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sfx {
    Jump,
    Hurt,
    Snap,
    Collect,
    Heart,
    Stomp,
    Rescue,
    Bark,
    Throw,
    Caw,
    Screech,
    Hiss,
}

/// What enemies can see of Kitty.
#[derive(Clone, Copy)]
pub struct KittyView {
    pub x: f32,
    pub y: f32, // feet
    /// Kitty's movement box (100x100) and the smaller body that can be hurt.
    pub rect: Rect,
    pub body: Rect,
    pub standing_on: Option<Support>,
    pub hanging_on: Option<LimbId>,
    pub airborne: bool,
}

/// What an enemy may see and touch during its step.
pub struct EnemyCtx<'a> {
    pub kitty: KittyView,
    pub limbs: &'a [Limb],
    /// Loads and jolts on limbs (applied after the step; forks pass them on to their parents).
    pub pushes: &'a mut Vec<Push>,
    pub projectiles: &'a mut Vec<Projectile>,
    /// The visible part of the world.
    pub view: Rect,
    pub rng: &'a mut Rng,
    pub difficulty: &'a Difficulty,
    pub sfx: &'a mut Vec<Sfx>,
}

pub trait Enemy {
    /// One 60 Hz physics step.
    fn step(&mut self, ctx: &mut EnemyCtx);
    /// Advance animation by `dt` seconds (called once per drawn frame).
    fn animate(&mut self, dt: f32);
    fn draw(&self, assets: &Assets);
    /// Do this enemy's damaging pixels touch `body`?
    fn strikes(&self, assets: &Assets, body: Rect) -> bool;
    /// The area Kitty's feet must land on to stomp it, if it can be stomped right now.
    fn stomp_box(&self, _assets: &Assets) -> Option<Rect> {
        None
    }
    /// Kitty landed on it from above.
    fn stomped(&mut self) {}
    /// False once it should be removed from the world.
    fn alive(&self) -> bool;
    /// Roughly its middle, for knockback direction and spawning distance checks.
    fn position(&self) -> Vec2;
    fn kind(&self) -> Kind;
}

/// An acorn thrown by a squirrel.
pub struct Projectile {
    pub pos: Vec2,
    pub vel: Vec2,
    pub spin: f32,
    pub rotation: f32,
    pub alive: bool,
}

pub const ACORN_GRAVITY: f32 = 900.0; // px/s^2

impl Projectile {
    pub fn acorn(pos: Vec2, vel: Vec2) -> Self {
        Self { pos, vel, spin: 12.0 * vel.x.signum(), rotation: 0.0, alive: true }
    }

    pub fn step(&mut self, dt: f32, view: Rect) {
        self.vel.y += ACORN_GRAVITY * dt;
        self.pos += self.vel * dt;
        self.rotation += self.spin * dt;
        if self.pos.y > view.bottom() + 100.0 || self.pos.x < view.left() - 200.0 || self.pos.x > view.right() + 200.0 {
            self.alive = false;
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.pos.x - 9.0, self.pos.y - 9.0, 18.0, 18.0)
    }

    pub fn draw(&self, assets: &Assets) {
        let frame = ((self.rotation.abs() * 2.0) as usize) % 3;
        assets.items.draw_ex(&format!("acorn_{frame}"), self.pos, 1.0, 1.0, self.rotation, WHITE);
    }
}

/// Brings in the enemies that arrive over time: dogs trotting in along the ground, crows and
/// eagles flying in from the sides. Squirrels and snakes are placed when the level starts.
pub struct Spawner {
    dog_timer: f32,
    crow_timer: f32,
    eagle_timer: f32,
}

impl Spawner {
    pub fn new(d: &Difficulty, rng: &mut Rng) -> Self {
        Self {
            dog_timer: d.dog_interval * rng.range(0.5, 1.5),
            crow_timer: rng.pair(d.crow_interval),
            eagle_timer: rng.pair(d.eagle_interval),
        }
    }

    pub fn step(&mut self, dt: f32, enemies: &mut Vec<Box<dyn Enemy>>, kitty: &KittyView, view: Rect,
                d: &Difficulty, rng: &mut Rng) {
        let count_of = |enemies: &Vec<Box<dyn Enemy>>, kind: Kind| enemies.iter().filter(|e| e.kind() == kind).count();
        if d.dogs {
            self.dog_timer -= dt;
            if self.dog_timer <= 0.0 {
                self.dog_timer = d.dog_interval * rng.range(0.5, 1.5);
                if count_of(enemies, Kind::Dog) < d.max_dogs {
                    // Trot in somewhere on the ground that's off-screen and well away from Kitty
                    for _ in 0..12 {
                        let x = rng.range(40.0, crate::consts::WORLD_W - 40.0);
                        let off_screen = x < view.left() - 60.0 || x > view.right() + 60.0;
                        if off_screen && (x - kitty.x).abs() > 300.0 {
                            enemies.push(Box::new(dog::Dog::new(x, d.dog_speed)));
                            break;
                        }
                    }
                }
            }
        }
        if d.crows {
            self.crow_timer -= dt;
            if self.crow_timer <= 0.0 {
                self.crow_timer = rng.pair(d.crow_interval);
                if count_of(enemies, Kind::Crow) < d.max_crows {
                    let y = rng.range(view.top() + 150.0, view.top() + view.h * 0.55);
                    enemies.push(Box::new(crow::Crow::new(rng.chance(0.5), y, view, 1.0)));
                }
            }
        }
        if d.eagles {
            self.eagle_timer -= dt;
            if self.eagle_timer <= 0.0 {
                self.eagle_timer = rng.pair(d.eagle_interval);
                if count_of(enemies, Kind::Eagle) < d.max_eagles {
                    let y = rng.range(view.top() + 100.0, view.top() + view.h / 2.0);
                    enemies.push(Box::new(eagle::Eagle::new(rng.chance(0.5), y, view, kitty, d.eagle_speed)));
                }
            }
        }
    }
}
