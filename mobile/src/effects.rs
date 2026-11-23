//! Little bits of life: sparkles and score pop-ups when something is collected, and leaves that
//! drift down in the breeze or get shaken loose when Kitty lands on a branch.

use std::f32::consts::TAU;

use macroquad::prelude::*;

use crate::rng::Rng;
use crate::sprites::Assets;

const FALLING: [&str; 6] = ["falling_0", "falling_1", "falling_2", "falling_3", "falling_4", "falling_5"];

enum Effect {
    Sparkle { pos: Vec2, age: f32 },
    Popup { pos: Vec2, text: String, color: Color, age: f32 },
    Leaf { pos: Vec2, vel: Vec2, rotation: f32, spin: f32, sprite: &'static str, sway: f32, age: f32 },
}

#[derive(Default)]
pub struct Effects {
    list: Vec<Effect>,
}

impl Effects {
    pub fn sparkle(&mut self, pos: Vec2) {
        self.list.push(Effect::Sparkle { pos, age: 0.0 });
    }

    pub fn popup(&mut self, pos: Vec2, text: impl Into<String>, color: Color) {
        self.list.push(Effect::Popup { pos, text: text.into(), color, age: 0.0 });
    }

    /// A few leaves shaken loose around `pos`.
    pub fn shake_leaves(&mut self, pos: Vec2, count: usize, rng: &mut Rng) {
        for _ in 0..count {
            self.leaf(pos + vec2(rng.range(-30.0, 30.0), rng.range(-20.0, 5.0)), rng);
        }
    }

    pub fn leaf(&mut self, pos: Vec2, rng: &mut Rng) {
        self.list.push(Effect::Leaf {
            pos,
            vel: vec2(rng.range(-20.0, 20.0), rng.range(20.0, 50.0)),
            rotation: rng.range(0.0, TAU),
            spin: rng.range(-2.0, 2.0),
            sprite: *rng.pick(&FALLING),
            sway: rng.range(0.0, TAU),
            age: 0.0,
        });
    }

    /// Ambient leaves drifting into view from above.
    pub fn drift(&mut self, view: Rect, rng: &mut Rng) {
        if rng.chance(0.04) {
            self.leaf(vec2(rng.range(view.left(), view.right()), view.top() - 20.0), rng);
        }
    }

    pub fn update(&mut self, dt: f32, breeze: f32, view: Rect) {
        for effect in &mut self.list {
            match effect {
                Effect::Sparkle { age, .. } => *age += dt,
                Effect::Popup { pos, age, .. } => {
                    *age += dt;
                    pos.y -= 40.0 * dt;
                }
                Effect::Leaf { pos, vel, rotation, spin, sway, age, .. } => {
                    *age += dt;
                    *sway += dt * 2.5;
                    pos.x += (vel.x + breeze * 12.0 + sway.sin() * 25.0) * dt;
                    pos.y += vel.y * dt;
                    *rotation += *spin * dt;
                }
            }
        }
        self.list.retain(|effect| match effect {
            Effect::Sparkle { age, .. } => *age < 0.5,
            Effect::Popup { age, .. } => *age < 1.0,
            Effect::Leaf { pos, age, .. } => *age < 12.0 && pos.y < view.bottom() + 40.0,
        });
    }

    /// Falling leaves take `leaf_tint`, as the leaves on the limbs do.
    pub fn draw(&self, assets: &Assets, leaf_tint: Color) {
        for effect in &self.list {
            match effect {
                Effect::Sparkle { pos, age } => {
                    let t = age / 0.5;
                    let alpha = 1.0 - t;
                    assets.items.draw_ex("star_0", *pos, 1.0, 0.6 + t * 0.9, t * 2.0, Color::new(1.0, 1.0, 1.0, alpha));
                }
                Effect::Popup { pos, text, color, age } => {
                    let alpha = (1.0 - age).clamp(0.0, 1.0);
                    let size = 30.0;
                    let w = measure_text(text, None, size as u16, 1.0).width;
                    draw_text(text, pos.x - w / 2.0 + 2.0, pos.y + 2.0, size, Color::new(0.0, 0.0, 0.0, alpha * 0.6));
                    draw_text(text, pos.x - w / 2.0, pos.y, size, Color::new(color.r, color.g, color.b, alpha));
                }
                Effect::Leaf { pos, rotation, sprite, age, .. } => {
                    let alpha = (12.0 - age).clamp(0.0, 1.0);
                    let color = Color::new(leaf_tint.r, leaf_tint.g, leaf_tint.b, alpha);
                    assets.foliage.draw_ex(sprite, *pos, 1.0, 1.0, *rotation, color);
                }
            }
        }
    }
}
