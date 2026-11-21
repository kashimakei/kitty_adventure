//! Tuning, ported from the pygame game's constants.py. Kitty and branch values are per physics
//! step, which runs at a fixed 60 Hz exactly like the pygame frame loop did.

use macroquad::color::Color;

/// Logical playfield; scaled to fit any screen with letterboxing.
pub const VIEW_W: f32 = 600.0;
pub const VIEW_H: f32 = 1200.0;
/// Physics steps at a fixed 60 Hz whatever the display's refresh rate.
pub const STEP: f32 = 1.0 / 60.0;
pub const MAX_FRAME_DT: f32 = 0.1;

/// World: three screens wide; the ground's top is at y = 0 and everything above is negative.
pub const WORLD_SCREENS: f32 = 3.0;
pub const WORLD_W: f32 = VIEW_W * WORLD_SCREENS;
pub const GROUND_Y: f32 = 0.0;
pub const GROUND_DEPTH: f32 = 160.0;

// Camera: Kitty roams the middle of the screen before it pans; the bottom strip holds the
// touch buttons, so the camera keeps Kitty above it.
pub const CAMERA_DEADZONE_X: f32 = 0.35;
pub const CAMERA_TOP_MARGIN: f32 = 0.33;
pub const CAMERA_BOTTOM_MARGIN: f32 = 0.66;
pub const CAMERA_GROUND_SHOWN: f32 = 170.0; // px of ground visible at the bottom when on it
pub const CAMERA_FOLLOW: f32 = 0.12;        // fraction of the gap closed per step

// Kitty (per 60 Hz step, as in kitty.py)
pub const KITTY_BOX: (f32, f32) = (100.0, 100.0);
pub const KITTY_SPEED: f32 = 5.0;
pub const KITTY_GRAVITY: f32 = 0.5;
pub const KITTY_JUMP_ACCEL: f32 = -1.0;
pub const KITTY_JUMP_STEPS: u32 = 30; // how long holding jump keeps accelerating upward
pub const KITTY_MIN_JUMP_STEPS: u32 = 10; // a quick tap on a phone still makes a proper hop
pub const STEP_ACROSS_UP: f32 = 30.0; // how far up Kitty steps when walking from one limb onto another
pub const KITTY_FOOT_REACH: f32 = 27.0;
pub const KITTY_WEIGHT: f32 = 38000.0;
pub const LANDING_TOLERANCE: f32 = 12.0;
pub const LANDING_KICK: f32 = 0.06;
pub const JUMP_KICK: f32 = 1.2;
pub const PANIC_SPEED: f32 = 20.0;
pub const WIND_DRIFT: f32 = 0.3; // px per step per unit of breeze while airborne
/// Jumping while the branch underfoot springs upward adds this much of its speed to the jump.
pub const SPRING_BOOST: f32 = 1.6;
pub const SPRING_BOOST_MAX: f32 = 7.0;

// Hanging & climbing
pub const GRAB_REACH: f32 = 22.0;
pub const GRIP_DEPTH: f32 = 4.0;
pub const HANG_SHIMMY: f32 = 2.0;
pub const CLIMB_STEPS: u32 = 33;
pub const REGRAB_STEPS: u32 = 24;

// Health
pub const MAX_HEARTS: u32 = 3;
pub const START_LIVES: u32 = 3;
pub const INVULNERABLE_STEPS: u32 = 90;
pub const KNOCKBACK_SPEED: f32 = 7.0;
pub const KNOCKBACK_HOP: f32 = -7.0;
pub const STOMP_BOUNCE: f32 = -9.0;

// Score
pub const SCORE_FISH: u32 = 10;
pub const SCORE_GOLD: u32 = 50;
pub const SCORE_SPARE_HEART: u32 = 25;
pub const SCORE_STOMP: u32 = 100;
pub const SCORE_RESCUE: u32 = 1000;
pub const SCORE_NO_DAMAGE: u32 = 500;
pub const SCORE_PER_SECOND_LEFT: u32 = 10;
pub const RESCUE_PAR_SECONDS: f32 = 90.0;
pub const SCORE_HEIGHT_STEP: f32 = 10.0; // 1 point per this many px of new height

// Trees & branches
pub const TRUNK_WIDTH: f32 = 50.0;
pub const TREE_SPACING: f32 = 270.0;
pub const BRANCH_LENGTH: (f32, f32) = (170.0, 220.0);
pub const BRANCH_STURDY_DEPTH: (f32, f32) = (30.0, 38.0);
pub const BRANCH_THIN_DEPTH: (f32, f32) = (22.0, 26.0);
pub const THIN_BRANCH_CHANCE: f32 = 0.3;
pub const THIN_BRANCH_STRENGTH: (f32, f32) = (1.15, 1.3);
pub const MAX_BRANCH_GAP: f32 = 360.0;
pub const MAX_BRANCH_HOP: f32 = 150.0;
pub const FILLERS_PER_PATH_BRANCH: usize = 2;
pub const BRANCH_SEGMENT_LEN: f32 = 18.0;
pub const BRANCH_SUB_STEPS: usize = 8;
pub const BRANCH_GRAVITY: f32 = 800.0;
pub const BRANCH_DAMPING: f32 = 0.995;
pub const BRANCH_WIND: f32 = 120.0;
pub const BRANCH_TREMBLE: f32 = 6000.0;
/// Forks: shorter, thinner limbs that sprout upward from sturdy branches.
pub const FORK_CHANCE: f32 = 0.55;
pub const FORK_LENGTH: (f32, f32) = (80.0, 120.0);
pub const FORK_RISE: (f32, f32) = (-0.75, -0.45);
pub const FORK_LOAD_SHARE: f32 = 0.8;

// Colours
pub const TRUNK_COLOR: Color = Color::new(0.36, 0.23, 0.13, 1.0);
pub const BARK: Color = Color::new(0.47, 0.29, 0.15, 1.0);
pub const BARK_DARK: Color = Color::new(0.27, 0.16, 0.08, 1.0);
pub const BARK_LIGHT: Color = Color::new(0.63, 0.42, 0.24, 1.0);
pub const BARK_YOUNG: Color = Color::new(0.64, 0.47, 0.27, 1.0);
/// Limbs are shaded from a sunlit top edge down to these on their undersides.
pub const BARK_SHADE: Color = Color::new(0.37, 0.22, 0.11, 1.0);
pub const BARK_YOUNG_LIGHT: Color = Color::new(0.77, 0.66, 0.41, 1.0);
pub const BARK_YOUNG_SHADE: Color = Color::new(0.50, 0.36, 0.19, 1.0);
/// Fresh pale wood showing where a limb snapped.
pub const WOOD_SPLINTER: Color = Color::new(0.90, 0.78, 0.55, 1.0);
pub const GRASS: Color = Color::new(0.36, 0.55, 0.20, 1.0);
pub const GRASS_DARK: Color = Color::new(0.25, 0.42, 0.14, 1.0);
pub const SOIL: Color = Color::new(0.33, 0.23, 0.14, 1.0);
