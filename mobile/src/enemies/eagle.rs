//! Eagle: cruises across, glides as a warning, then swoops at Kitty with talons out before pulling
//! away. (Placeholder: flies straight across.)

use macroquad::prelude::*;

use super::{Enemy, EnemyCtx, Kind, KittyView};
use crate::sprites::Assets;

pub struct Eagle {
    pos: Vec2,
    vx: f32,
    alive: bool,
    time: f32,
}

impl Eagle {
    pub fn new(from_left: bool, y: f32, view: Rect, _kitty: &KittyView, speed_scale: f32) -> Self {
        let x = if from_left { view.left() - 80.0 } else { view.right() + 80.0 };
        Self { pos: vec2(x, y), vx: if from_left { 200.0 } else { -200.0 } * speed_scale, alive: true, time: 0.0 }
    }

    fn facing(&self) -> f32 {
        self.vx.signum()
    }

    fn frame(&self) -> String {
        format!("flap_{}", (self.time * 10.0) as usize % 5)
    }
}

impl Enemy for Eagle {
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
        assets.eagle.draw(&self.frame(), self.pos, self.facing(), WHITE);
    }
    fn strikes(&self, assets: &Assets, body: Rect) -> bool {
        assets.eagle.strikes(&self.frame(), self.pos, self.facing(), body)
    }
    fn alive(&self) -> bool {
        self.alive
    }
    fn position(&self) -> Vec2 {
        self.pos
    }
    fn kind(&self) -> Kind {
        Kind::Eagle
    }
}
