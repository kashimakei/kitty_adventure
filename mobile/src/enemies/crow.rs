//! Crow: flaps in from the side on a wavy path and dives to peck at Kitty. Can be stomped.
//! (Placeholder: flies straight across.)

use macroquad::prelude::*;

use super::{Enemy, EnemyCtx, Kind};
use crate::sprites::Assets;

pub struct Crow {
    pos: Vec2,
    vx: f32,
    alive: bool,
    time: f32,
}

impl Crow {
    pub fn new(from_left: bool, y: f32, view: Rect, speed_scale: f32) -> Self {
        let x = if from_left { view.left() - 60.0 } else { view.right() + 60.0 };
        Self { pos: vec2(x, y), vx: if from_left { 150.0 } else { -150.0 } * speed_scale, alive: true, time: 0.0 }
    }

    fn facing(&self) -> f32 {
        self.vx.signum()
    }

    fn frame(&self) -> String {
        format!("fly_{}", (self.time * 10.0) as usize % 6)
    }
}

impl Enemy for Crow {
    fn step(&mut self, ctx: &mut EnemyCtx) {
        self.pos.x += self.vx * crate::consts::STEP;
        if self.pos.x < ctx.view.left() - 200.0 || self.pos.x > ctx.view.right() + 200.0 {
            self.alive = false;
        }
    }
    fn animate(&mut self, dt: f32) {
        self.time += dt;
    }
    fn draw(&self, assets: &Assets) {
        assets.crow.draw(&self.frame(), self.pos, self.facing(), WHITE);
    }
    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        assets.crow.strikes(&self.frame(), self.pos, self.facing(), body)
    }
    fn alive(&self) -> bool {
        self.alive
    }
    fn position(&self) -> Vec2 {
        self.pos
    }
    fn kind(&self) -> Kind {
        Kind::Crow
    }
}
