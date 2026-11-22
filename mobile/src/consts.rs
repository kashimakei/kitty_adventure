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

// Camera: Kitty roams the middle of the screen before it pans; on a phone, thumbs slide and hold
// over the bottom strip, so the camera keeps Kitty above it.
pub const CAMERA_DEADZONE_X: f32 = 0.35;
pub const CAMERA_TOP_MARGIN: f32 = 0.33;
pub const CAMERA_BOTTOM_MARGIN: f32 = 0.66;
pub const CAMERA_GROUND_SHOWN: f32 = 170.0; // px of ground visible at the bottom when on it
pub const CAMERA_FOLLOW: f32 = 0.12;        // fraction of the gap closed per step

// Kitty (per 60 Hz step, as in kitty.py)
pub const KITTY_BOX: (f32, f32) = (100.0, 100.0);
/// Top running speed, with the slider (or an arrow key) all the way over.
pub const KITTY_SPEED: f32 = 5.0;
/// A cat's arc rather than a rocket's: she slows as she rises, floats a moment at the top (gravity
/// halves below `APEX_FLOAT_SPEED`), then drops a little quicker, up to `TERMINAL_SPEED`. Terminal
/// speed is high so the height of a fall still shows when she lands.
pub const RISE_GRAVITY: f32 = 0.6;
pub const FALL_GRAVITY: f32 = 0.85;
pub const APEX_FLOAT_SPEED: f32 = 2.0;
pub const TERMINAL_SPEED: f32 = 40.0;
/// Leg power (upward px/step) for a tap and for a full charge: about 70 px and 470 px high on
/// solid ground. Charging eases from one to the other over `CHARGE_STEPS`.
pub const LEAP_SPEED: (f32, f32) = (9.3, 24.0);
pub const CHARGE_STEPS: u32 = 30;
/// Crouched and charging, she can still creep along (and turn to aim) at this share of speed.
pub const CHARGE_CREEP: f32 = 0.35;
/// Crouching presses down a little harder than standing.
pub const CHARGE_LOAD: f32 = 1.1;
/// Forgiveness: a release this many steps before landing leaps on landing; a release this many
/// steps after walking off an edge still leaps; a press in the last steps of a climb carries over.
pub const LEAP_BUFFER_STEPS: u32 = 8;
pub const COYOTE_STEPS: u32 = 6;
pub const CLIMB_CARRY_STEPS: u32 = 15;
/// Hanging, a held press climbs after this many steps (a tap climbs on release), so a swipe down
/// to let go isn't mistaken for a climb.
pub const CLIMB_HOLD_STEPS: u32 = 6;
pub const STEP_ACROSS_UP: f32 = 30.0; // how far up Kitty steps when walking from one limb onto another
pub const KITTY_FOOT_REACH: f32 = 27.0;
pub const KITTY_WEIGHT: f32 = 38000.0;
pub const LANDING_TOLERANCE: f32 = 12.0;
/// Landing, her legs take her weight over this many steps rather than all at once.
pub const LANDING_STEPS: u32 = 8;
pub const PANIC_SPEED: f32 = 32.0; // falling faster than a full leap comes down
pub const WIND_DRIFT: f32 = 0.3; // px per step per unit of breeze while airborne

// Branches as trampolines. Pushing off loses `LEAP_LOSS` x the limb's give into bending it (the
// limb whips down by `PUSH_KICK` x the lost speed), and she keeps `SURFACE_CARRY` of the speed
// the wood under her paws is moving up (or down) at. Landing braced (still holding the jump)
// drives her whole impact into the limb, so it throws her back up; landing relaxed, her legs
// soak up most of it.
pub const LEAP_LOSS: f32 = 0.4;
pub const PUSH_KICK: f32 = 0.3;
pub const SURFACE_CARRY: f32 = 1.0;
pub const LANDING_KICK: f32 = 0.3;
pub const RELAXED_LANDING: f32 = 0.4;
/// A braced landing at terminal speed on solid wood jolts a limb with this many times Kitty's
/// weight (scaled by the square of the speed); give softens it by up to `IMPACT_SOFTEN`. A share
/// of a braced landing's jolt lingers as fatigue in the wood, fading over `FATIGUE_FADE` seconds.
pub const IMPACT_LOAD: f32 = 3.0;
pub const IMPACT_SOFTEN: f32 = 0.5;
pub const FATIGUE_SHARE: f32 = 0.12;
pub const FATIGUE_FADE: f32 = 7.0;
/// Sag under Kitty's weight (px) that counts as a limb's full give.
pub const FULL_GIVE_SAG: f32 = 60.0;
/// The wood under a charging Kitty rising faster than this (px/step): a release now would ride
/// the rebound, so her ring glows.
pub const SWEET_SPOT_SPEED: f32 = 1.5;
/// Kicks bigger than this (px per sub-step) shake leaves loose.
pub const SHAKE_KICK: f32 = 0.6;

// Dropping through a limb: it, and anything within `DROP_SKIP_NEAR` px of where she started,
// can't catch her until she's `DROP_CLEAR` px lower or `DROP_STEPS` have passed. That's far enough
// that her raised paws are past it too, so she doesn't catch it as it goes by. A fork or parent
// joined to it can't catch her less than `DROP_JOINED_NEAR` px below where she started.
pub const DROP_SPEED: f32 = 1.0;
pub const DROP_SKIP_NEAR: f32 = 16.0;
pub const DROP_JOINED_NEAR: f32 = 40.0;
pub const DROP_CLEAR: f32 = KITTY_BOX.1;
pub const DROP_STEPS: u32 = 40;
/// Standing at a tip that bends in under her, her inner paw stays this far inside it.
pub const TIP_GRIP: f32 = 2.0;

// Hanging & climbing
pub const GRAB_REACH: f32 = 22.0;
pub const GRAB_KICK: f32 = 0.36;
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
/// Sturdy branches and forks (x Kitty's weight): Kitty and two dogs stand on one safely, but
/// bouncing on it too hard snaps it.
pub const STURDY_BRANCH_STRENGTH: f32 = 3.0;
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
