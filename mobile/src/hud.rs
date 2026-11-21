//! The heads-up display along the top: hearts, lives, score, the level, and a minimap of the
//! world's width showing Kitty and where her friend is waiting. Drawn in playfield coordinates.
//! (Placeholder: plain text.)

use macroquad::prelude::*;

use crate::consts::*;
use crate::sprites::Assets;
use crate::story::Friend;

pub struct HudState {
    pub hearts: u32,
    pub max_hearts: u32,
    pub lives: u32,
    pub score: u32,
    pub level: usize,
    pub level_title: &'static str,
    pub friend: Friend,
    pub friends_rescued: usize,
    /// World x of Kitty, the goal nest, and the left edge of the view.
    pub kitty_x: f32,
    pub goal_x: f32,
    pub view_x: f32,
    /// 0 at the ground, 1 at the goal's height.
    pub climb: f32,
    pub invulnerable: bool,
}

pub fn draw(_assets: &Assets, hud: &HudState, _time: f32) {
    draw_rectangle(0.0, 0.0, VIEW_W, 60.0, Color::new(1.0, 1.0, 1.0, 0.7));
    draw_text(&format!("Hearts {}/{}  Lives {}  Score {}", hud.hearts, hud.max_hearts, hud.lives, hud.score),
              12.0, 38.0, 28.0, BLACK);
    let bar = Rect::new(150.0, 66.0, 300.0, 10.0);
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, Color::new(0.3, 0.2, 0.1, 0.8));
    let to_bar = |x: f32| bar.x + x / WORLD_W * bar.w;
    draw_circle(to_bar(hud.goal_x), bar.y + 5.0, 6.0, GOLD);
    draw_circle(to_bar(hud.kitty_x), bar.y + 5.0, 5.0, ORANGE);
}
