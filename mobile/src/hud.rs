//! The heads-up display along the top: hearts, lives, the level and score, the five friends, and a
//! minimap of the world's width showing the view, Kitty and where her friend is waiting, plus a
//! slim meter of how far up she has climbed. Drawn in playfield coordinates along the top, clear
//! of the pause button (top right) and of the lower screen, where thumbs slide and hold.

use macroquad::prelude::*;

use crate::consts::*;
use crate::level::tree_positions;
use crate::sprites::Assets;
use crate::story::paint::*;
use crate::story::{thousands, Friend, FRIENDS};

/// Top bar: hearts, lives, level and score.
const BAR: Rect = Rect { x: 8.0, y: 8.0, w: 584.0, h: 52.0 };
/// Second row: the friends, then the minimap, stopping short of the pause button (x 548..596).
const FRIENDS_BOX: Rect = Rect { x: 8.0, y: 66.0, w: 192.0, h: 48.0 };
const MAP_BOX: Rect = Rect { x: 206.0, y: 66.0, w: 330.0, h: 48.0 };
/// The climb meter runs down the right edge, under the pause button.
const CLIMB_X: f32 = 578.0;
const CLIMB_TOP: f32 = 166.0;
const CLIMB_BOTTOM: f32 = 520.0;
const HEART_GAP: f32 = 40.0;
const EMPTY: Color = Color::new(0.25, 0.25, 0.28, 0.6);
/// Friends not yet rescued: pale silhouettes.
const WAITING: Color = Color::new(0.62, 0.66, 0.66, 0.5);
const KITTY_MARK: Color = Color::new(1.0, 0.60, 0.22, 1.0);

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

/// `_time` restarts whenever play resumes, so the HUD's own motion runs on the game clock.
pub fn draw(assets: &Assets, hud: &HudState, _time: f32) {
    let t = get_time() as f32;
    top_bar(assets, hud, t);
    friends_row(assets, hud, t);
    minimap(assets, hud, t);
    climb_meter(assets, hud, t);
}

fn top_bar(assets: &Assets, hud: &HudState, t: f32) {
    glass(BAR, 26.0);
    let mid_y = BAR.y + BAR.h / 2.0;

    // Hearts: blinking while Kitty can't be hurt, the last one beating
    let slots = hud.max_hearts.max(hud.hearts);
    for i in 0..slots {
        let at = vec2(BAR.x + 28.0 + i as f32 * HEART_GAP, mid_y + 1.0);
        let full = i < hud.hearts;
        let (scale, tint) = if !full {
            (0.85, EMPTY)
        } else if hud.invulnerable {
            let blink = if (t * 8.0) as i32 % 2 == 0 { 1.0 } else { 0.35 };
            (1.0 + (t * 16.0).sin() * 0.1, fade(WHITE, blink))
        } else if hud.hearts == 1 {
            (1.0 + (t * 5.0).sin().max(0.0).powi(4) * 0.18, WHITE)
        } else {
            (1.0, WHITE)
        };
        assets.items.draw_ex("heart_0", at, 1.0, scale, 0.0, tint);
    }

    // Lives
    let x = BAR.x + 28.0 + slots as f32 * HEART_GAP;
    assets.kitty.draw_ex("sit_0", vec2(x, BAR.bottom() - 9.0), 1.0, 0.36, 0.0, WHITE);
    let lives = format!("x{}", hud.lives);
    shadowed(&lives, x + 20.0, mid_y + 9.0, 24.0, CREAM, Align::Left);
    let left_end = x + 20.0 + measure(&lives, 24.0) + 12.0;

    // Score, right-aligned behind a golden fish
    let score = thousands(hud.score);
    let score_w = measure(&score, 28.0);
    let score_x = BAR.right() - 16.0;
    shadowed(&score, score_x, mid_y + 10.0, 28.0, GOLD_TEXT, Align::Right);
    let fish_x = score_x - score_w - 26.0;
    assets.items.draw_ex("gold_0", vec2(fish_x, mid_y), 1.0, 0.72, (t * 2.0).sin() * 0.08, WHITE);

    // The level in the middle
    let right_end = fish_x - 26.0;
    let centre = (left_end + right_end) / 2.0;
    let width = right_end - left_end;
    let label = format!("LEVEL {}", hud.level);
    text(&label, centre, BAR.y + 22.0, 16.0, fade(ORANGE_TEXT, 0.95), Align::Center);
    let size = fit_size(hud.level_title, 20.0, width);
    shadowed(hud.level_title, centre, BAR.y + 43.0, size, CREAM, Align::Center);
}

/// The five friends: greyed until rescued, the one waiting this level ringed in gold.
fn friends_row(assets: &Assets, hud: &HudState, t: f32) {
    glass(FRIENDS_BOX, 22.0);
    let slot = (FRIENDS_BOX.w - 16.0) / FRIENDS.len() as f32;
    for (i, friend) in FRIENDS.into_iter().enumerate() {
        let centre = vec2(FRIENDS_BOX.x + 8.0 + slot * (i as f32 + 0.5), FRIENDS_BOX.y + FRIENDS_BOX.h / 2.0);
        let rescued = i < hud.friends_rescued;
        let current = friend == hud.friend && !rescued;
        let mut feet = centre + vec2(0.0, 17.0);
        if current {
            let glow = (t * 3.0).sin() * 0.5 + 0.5;
            draw_circle(centre.x, centre.y, 19.0, fade(GOLD_TEXT, 0.25 + glow * 0.2));
            draw_circle_lines(centre.x, centre.y, 19.0, 2.5, fade(GOLD_TEXT, 0.8));
            feet.y -= (t * 4.0).sin().abs() * 3.0;
        }
        let tint = if rescued || current { WHITE } else { WAITING };
        let frame = friend.help_frame();
        let size = assets.friends.get(&frame).size();
        let scale = 32.0 / size.x.max(size.y);
        assets.friends.draw_ex(&frame, feet, 1.0, scale, 0.0, tint);
        if rescued {
            // A little green tick
            let at = centre + vec2(11.0, 11.0);
            draw_circle(at.x, at.y, 7.0, Color::new(0.30, 0.70, 0.30, 1.0));
            draw_line(at.x - 3.5, at.y, at.x - 1.0, at.y + 3.0, 2.0, WHITE);
            draw_line(at.x - 1.0, at.y + 3.0, at.x + 3.5, at.y - 3.0, 2.0, WHITE);
        }
    }
}

/// The world's width in a strip: the trees, the part on screen, the friend's nest and Kitty.
fn minimap(assets: &Assets, hud: &HudState, t: f32) {
    glass(MAP_BOX, 22.0);
    let strip = Rect::new(MAP_BOX.x + 14.0, MAP_BOX.y + 8.0, MAP_BOX.w - 28.0, MAP_BOX.h - 16.0);
    let to_map = |x: f32| strip.x + (x / WORLD_W).clamp(0.0, 1.0) * strip.w;
    let ground = strip.bottom() - 3.0;

    for x in tree_positions() {
        let x = to_map(x);
        draw_line(x, strip.y + 2.0, x, ground, 3.0, Color::new(BARK.r, BARK.g, BARK.b, 0.85));
        draw_circle(x, strip.y + 4.0, 5.0, Color::new(GRASS.r, GRASS.g, GRASS.b, 0.75));
    }
    draw_line(strip.x - 4.0, ground, strip.right() + 4.0, ground, 3.0, Color::new(GRASS.r, GRASS.g, GRASS.b, 0.9));

    // The view
    let view = Rect::new(to_map(hud.view_x), strip.y - 3.0, VIEW_W / WORLD_W * strip.w, strip.h + 6.0);
    rounded_rect(view, 6.0, fade(CREAM, 0.16), fade(CREAM, 0.10));
    rounded_outline(view, 6.0, 2.0, fade(CREAM, 0.7));

    // The friend waiting in the nest
    let nest = vec2(to_map(hud.goal_x), strip.y + 15.0);
    assets.items.draw_ex("nest_0", nest, 1.0, 0.24, 0.0, WHITE);
    let frame = hud.friend.help_frame();
    let size = assets.friends.get(&frame).size();
    let bob = (t * 3.0).sin() * 1.5;
    assets.friends.draw_ex(&frame, nest - vec2(0.0, 6.0 + bob), 1.0, 18.0 / size.x.max(size.y), 0.0, WHITE);

    // Kitty
    let k = vec2(to_map(hud.kitty_x), ground - 6.0);
    draw_circle(k.x, k.y, 7.5, INK);
    draw_circle(k.x, k.y, 5.5, KITTY_MARK);
    draw_circle(k.x - 1.5, k.y - 2.0, 1.8, fade(WHITE, 0.8));
}

/// How far up toward the friend Kitty is, as a slim vertical gauge.
fn climb_meter(assets: &Assets, hud: &HudState, t: f32) {
    let track = Rect::new(CLIMB_X - 6.0, CLIMB_TOP, 12.0, CLIMB_BOTTOM - CLIMB_TOP);
    let outline = grow(track, 3.0);
    rounded_rect(outline, 9.0, Color::new(0.04, 0.07, 0.06, 0.7), Color::new(0.04, 0.07, 0.06, 0.7));
    rounded_rect(track, 6.0, Color::new(1.0, 0.96, 0.88, 0.25), Color::new(1.0, 0.96, 0.88, 0.25));
    let climb = hud.climb.clamp(0.0, 1.0);
    let filled = Rect::new(track.x, track.bottom() - track.h * climb, track.w, track.h * climb);
    if filled.h > 1.0 {
        rounded_rect(filled, 6.0, GOLD_TEXT, Color::new(0.45, 0.78, 0.35, 1.0));
    }
    for i in 1..4 {
        let y = track.bottom() - track.h * i as f32 / 4.0;
        draw_line(track.x + 2.0, y, track.right() - 2.0, y, 1.5, Color::new(1.0, 1.0, 1.0, 0.35));
    }
    // The nest on top, Kitty riding the fill
    assets.items.draw_ex("nest_0", vec2(CLIMB_X, CLIMB_TOP - 4.0), 1.0, 0.3, 0.0, WHITE);
    let frame = hud.friend.help_frame();
    let size = assets.friends.get(&frame).size();
    assets.friends.draw_ex(&frame, vec2(CLIMB_X, CLIMB_TOP - 10.0 - (t * 3.0).sin().abs() * 2.0), -1.0,
                           22.0 / size.x.max(size.y), 0.0, WHITE);
    let k = vec2(CLIMB_X, filled.y);
    draw_circle(k.x, k.y, 9.0, INK);
    draw_circle(k.x, k.y, 7.0, KITTY_MARK);
    draw_circle(k.x - 2.0, k.y - 2.5, 2.2, fade(WHITE, 0.8));
}
