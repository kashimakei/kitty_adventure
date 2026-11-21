//! A squirrel: scampers along its branch and throws acorns at Kitty. Can be stomped.
//! (Placeholder: sits still on its branch.)

use macroquad::prelude::*;

use super::{Enemy, EnemyCtx, Kind};
use crate::limb::LimbId;
use crate::sprites::Assets;

pub struct Squirrel {
    limb: LimbId,
    frac: f32,
    pos: Vec2,
    facing: f32,
    alive: bool,
}

impl Squirrel {
    pub fn new(limb: LimbId, frac: f32) -> Self {
        Self { limb, frac, pos: vec2(-9999.0, -9999.0), facing: 1.0, alive: true }
    }
}

impl Enemy for Squirrel {
    fn step(&mut self, ctx: &mut EnemyCtx) {
        let limb = &ctx.limbs[self.limb];
        let (lo, hi) = limb.walk_range();
        let x = lo + (hi - lo) * self.frac;
        match limb.surface_y(x) {
            Some(y) => self.pos = vec2(x, y),
            None => self.alive = false,
        }
        self.facing = if ctx.kitty.x >= self.pos.x { 1.0 } else { -1.0 };
    }
    fn animate(&mut self, _dt: f32) {}
    fn draw(&self, assets: &Assets) {
        assets.squirrel.draw("idle_0", self.pos, self.facing, WHITE);
    }
    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        assets.squirrel.strikes("idle_0", self.pos, self.facing, body)
    }
    fn stomp_box(&self, assets: &Assets) -> Option<Rect> {
        let b = assets.squirrel.bounds("idle_0", self.pos, self.facing);
        Some(Rect::new(b.x + b.w * 0.15, b.y, b.w * 0.7, b.h * 0.45))
    }
    fn stomped(&mut self) {
        self.alive = false;
    }
    fn alive(&self) -> bool {
        self.alive
    }
    fn position(&self) -> Vec2 {
        self.pos - vec2(0.0, 25.0)
    }
    fn kind(&self) -> Kind {
        Kind::Squirrel
    }
}
