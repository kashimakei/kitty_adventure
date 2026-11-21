//! Dog: lives on the ground, sniffs about, snarls then charges at Kitty on its level, barks up at
//! her overhead, and leaps onto branches low enough to reach. (Placeholder: stands still.)

use macroquad::prelude::*;

use super::{Enemy, EnemyCtx, Kind};
use crate::consts::*;
use crate::sprites::Assets;

pub struct Dog {
    x: f32,
    y: f32,
    facing: f32,
}

impl Dog {
    pub fn new(x: f32, _speed_scale: f32) -> Self {
        Self { x, y: GROUND_Y, facing: 1.0 }
    }
}

impl Enemy for Dog {
    fn step(&mut self, ctx: &mut EnemyCtx) {
        self.facing = if ctx.kitty.x >= self.x { 1.0 } else { -1.0 };
    }
    fn animate(&mut self, _dt: f32) {}
    fn draw(&self, assets: &Assets) {
        assets.dog.draw("sniff_0", vec2(self.x, self.y), self.facing, WHITE);
    }
    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        assets.dog.strikes("sniff_0", vec2(self.x, self.y), self.facing, body)
    }
    fn alive(&self) -> bool {
        true
    }
    fn position(&self) -> Vec2 {
        vec2(self.x, self.y - 40.0)
    }
    fn kind(&self) -> Kind {
        Kind::Dog
    }
}
