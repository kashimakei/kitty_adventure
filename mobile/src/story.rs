//! The story: a storm scattered Kitty's five friends to the tops of the tallest trees, and Kitty
//! climbs to bring each one home. Story text, level titles and the menu screens live here; every
//! draw function works in playfield coordinates (the screen camera).
//! (Placeholder screens: plain text. To be dressed up.)

use macroquad::prelude::*;

use crate::consts::*;
use crate::sprites::Assets;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Friend {
    Pip,
    Hazel,
    Mochi,
    Sunny,
    Luna,
}

pub const FRIENDS: [Friend; 5] = [Friend::Pip, Friend::Hazel, Friend::Mochi, Friend::Sunny, Friend::Luna];

impl Friend {
    pub fn for_level(level: usize) -> Friend {
        FRIENDS[(level - 1) % FRIENDS.len()]
    }

    pub fn name(self) -> &'static str {
        match self {
            Friend::Pip => "Pip",
            Friend::Hazel => "Hazel",
            Friend::Mochi => "Mochi",
            Friend::Sunny => "Sunny",
            Friend::Luna => "Luna",
        }
    }

    /// Frame name prefix in assets/friends ("pip" -> pip_help_0, pip_cheer_0..).
    pub fn sprite(self) -> &'static str {
        match self {
            Friend::Pip => "pip",
            Friend::Hazel => "hazel",
            Friend::Mochi => "mochi",
            Friend::Sunny => "sunny",
            Friend::Luna => "luna",
        }
    }

    /// How many cheer frames its sheet row has.
    pub fn cheer_frames(self) -> usize {
        if self == Friend::Pip { 4 } else { 3 }
    }
}

pub struct LevelInfo {
    pub title: &'static str,
    pub friend: Friend,
    pub intro: &'static str,
    pub thanks: &'static str,
}

pub fn level_info(level: usize) -> LevelInfo {
    let friend = Friend::for_level(level);
    LevelInfo { title: "The Woods", friend, intro: "A friend is stuck at the top!", thanks: "Thank you, Kitty!" }
}

pub enum PageArt {
    Kitty,
    Friend(Friend),
    AllFriends,
}

pub struct Page {
    pub heading: &'static str,
    pub text: &'static str,
    pub art: PageArt,
}

pub fn intro_pages() -> Vec<Page> {
    vec![Page { heading: "The Treetop Rescue", text: "Kitty's friends are stuck in the trees. Climb up and bring them home!", art: PageArt::Kitty }]
}

pub fn ending_pages() -> Vec<Page> {
    vec![Page { heading: "Home at last", text: "Everyone is safe.", art: PageArt::AllFriends }]
}

/// Points earned on a level, shown when it's cleared.
#[derive(Clone, Copy, Default)]
pub struct Tally {
    pub rescue: u32,
    pub time_bonus: u32,
    pub no_damage: u32,
    pub collected: u32,
    pub level_total: u32,
    pub score: u32,
}

fn centered(text: &str, y: f32, size: f32, color: Color) {
    let w = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, (VIEW_W - w) / 2.0, y, size, color);
}

fn backdrop() {
    draw_rectangle(0.0, 0.0, VIEW_W, VIEW_H, Color::new(0.1, 0.12, 0.18, 1.0));
}

pub fn draw_title(_assets: &Assets, _time: f32, high_score: u32) {
    backdrop();
    centered("Kitty Adventure", 400.0, 64.0, ORANGE);
    centered("Tap to start", 700.0, 36.0, WHITE);
    centered(&format!("High score {high_score}"), 780.0, 28.0, LIGHTGRAY);
}

pub fn draw_page(_assets: &Assets, page: &Page, _time: f32) {
    backdrop();
    centered(page.heading, 300.0, 48.0, ORANGE);
    centered(page.text, 500.0, 24.0, WHITE);
    centered("Tap to continue", 1000.0, 28.0, LIGHTGRAY);
}

pub fn draw_level_intro(_assets: &Assets, level: usize, info: &LevelInfo, _time: f32) {
    backdrop();
    centered(&format!("Level {level}: {}", info.title), 400.0, 44.0, ORANGE);
    centered(info.intro, 520.0, 26.0, WHITE);
    centered("Tap to climb!", 1000.0, 28.0, LIGHTGRAY);
}

/// Shown over the world while the rescued friend cheers.
pub fn draw_rescue_banner(_assets: &Assets, info: &LevelInfo, _time: f32) {
    centered(&format!("You found {}!", info.friend.name()), 300.0, 44.0, YELLOW);
}

pub fn draw_level_clear(_assets: &Assets, _info: &LevelInfo, tally: &Tally, _time: f32) {
    backdrop();
    centered("Level clear!", 360.0, 52.0, YELLOW);
    centered(&format!("Level points {}", tally.level_total), 520.0, 32.0, WHITE);
    centered(&format!("Score {}", tally.score), 580.0, 32.0, WHITE);
    centered("Tap to continue", 1000.0, 28.0, LIGHTGRAY);
}

pub fn draw_game_over(_assets: &Assets, score: u32, high_score: u32, _time: f32) {
    backdrop();
    centered("Game over", 420.0, 60.0, RED);
    centered(&format!("Score {score}   High {high_score}"), 540.0, 30.0, WHITE);
    centered("Tap to try again", 1000.0, 28.0, LIGHTGRAY);
}

pub fn draw_life_lost(lives_left: u32, _time: f32) {
    draw_rectangle(0.0, 0.0, VIEW_W, VIEW_H, Color::new(0.0, 0.0, 0.0, 0.45));
    centered(&format!("Ouch! {lives_left} lives left"), 560.0, 44.0, WHITE);
}

pub fn draw_paused(_time: f32) {
    draw_rectangle(0.0, 0.0, VIEW_W, VIEW_H, Color::new(0.0, 0.0, 0.0, 0.5));
    centered("Paused", 560.0, 60.0, WHITE);
    centered("Tap to resume", 640.0, 30.0, LIGHTGRAY);
}
