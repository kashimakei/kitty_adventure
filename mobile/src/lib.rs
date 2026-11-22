//! Kitty Adventure: The Treetop Rescue.
//!
//! A storm scattered Kitty's five friends to the tops of the tallest trees. Each level is a
//! stretch of forest three screens wide: climb the bendy branches (from the shared Rust engine in
//! `Engines/`), dodge squirrels, snakes, crows, dogs and eagles, and reach the friend waiting in a
//! nest at the top.
//!
//! The same code runs on desktop, in the browser (wasm) and on Android, where the library is
//! loaded by the Gradle-built app (see `android/`).

pub mod audio;
pub mod camera;
pub mod consts;
pub mod effects;
pub mod enemies;
pub mod hud;
pub mod input;
pub mod items;
pub mod kitty;
pub mod level;
pub mod limb;
pub mod parallax;
pub mod platform;
pub mod rng;
pub mod save;
pub mod sprites;
pub mod story;
pub mod tree;
pub mod world;

use macroquad::prelude::*;

use crate::audio::Audio;
use crate::camera::View;
use crate::consts::*;
use crate::hud::HudState;
use crate::input::{Input, Pending};
use crate::level::STORY_LEVELS;
use crate::sprites::Assets;
use crate::story::Tally;
use crate::world::{Outcome, World};

pub fn window_conf() -> Conf {
    Conf {
        window_title: "Kitty Adventure".to_owned(),
        window_width: 450,
        window_height: 900,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

enum Screen {
    Title,
    Story { pages: Vec<story::Page>, page: usize, ending: bool },
    LevelIntro,
    Playing,
    Paused,
    LifeLost { timer: f32 },
    LevelClear { tally: Tally },
    GameOver,
}

struct Game {
    screen: Screen,
    screen_time: f32,
    world: Option<World>,
    level: usize,
    lap: usize,
    lives: u32,
    score: u32,
    high_score: u32,
    /// The high score before the current game, for the game-over screen.
    previous_best: u32,
    friends_rescued: usize,
    seed: u64,
    /// Bumped on every screen change, so input held across one can be ignored.
    screen_gen: u32,
}

impl Game {
    fn new() -> Self {
        Self {
            screen: Screen::Title,
            screen_time: 0.0,
            world: None,
            level: 1,
            lap: 0,
            lives: START_LIVES,
            score: 0,
            high_score: save::load_high_score(),
            previous_best: 0,
            friends_rescued: 0,
            seed: (macroquad::miniquad::date::now() * 1000.0) as u64,
            screen_gen: 0,
        }
    }

    fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.screen_time = 0.0;
        self.screen_gen = self.screen_gen.wrapping_add(1);
    }

    fn start_level(&mut self) {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.world = Some(World::new(self.level, self.lap, self.seed));
        self.go(Screen::LevelIntro);
    }

    fn new_game(&mut self) {
        self.level = 1;
        self.lap = 0;
        self.lives = START_LIVES;
        self.score = 0;
        self.friends_rescued = 0;
        self.go(Screen::Story { pages: story::intro_pages(), page: 0, ending: false });
    }

    fn finish_game(&mut self) {
        self.previous_best = self.high_score;
        self.high_score = self.high_score.max(self.score);
        save::save_high_score(self.high_score);
    }

    fn hud(&self, world: &World) -> HudState {
        HudState {
            hearts: world.kitty.hearts,
            max_hearts: MAX_HEARTS,
            lives: self.lives,
            score: self.score + world.points,
            level: self.level,
            level_title: story::level_info(self.level).title,
            friend: world.friend,
            friends_rescued: self.friends_rescued,
            kitty_x: world.kitty.x,
            goal_x: world.nest_position().x,
            view_x: world.camera.x,
            climb: (-world.kitty.y / -world.top_y()).clamp(0.0, 1.0),
            invulnerable: world.kitty.invulnerable > 0,
        }
    }

    /// Tally a cleared level and add it to the score.
    fn tally(&mut self) -> Tally {
        let world = self.world.as_ref().unwrap();
        let time_bonus = ((RESCUE_PAR_SECONDS - world.elapsed).max(0.0) as u32) * SCORE_PER_SECOND_LEFT;
        let no_damage = if world.hits_taken == 0 { SCORE_NO_DAMAGE } else { 0 };
        let level_total = world.points + SCORE_RESCUE + time_bonus + no_damage;
        self.score += level_total;
        Tally {
            rescue: SCORE_RESCUE,
            time_bonus,
            no_damage,
            collected: world.collected_points,
            level_total,
            score: self.score,
        }
    }
}

pub async fn run() {
    // Desktop runs from mobile/ with the staged files in assets/; the web and Android builds
    // serve them from their root
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    macroquad::file::set_pc_assets_folder("assets");
    // Input reads touches as touches; mirrored as a mouse too, every tap would count twice
    simulate_mouse_with_touch(false);
    let mut input = Input::new();
    // Sounds load in the background alongside the sprites
    let audio = Audio::load();
    audio.start_music();
    let assets = Assets::load().await;
    let mut game = Game::new();
    let mut accumulator = 0.0;
    let mut pending = Pending::default();
    let mut screen_gen = game.screen_gen;

    loop {
        let view = View::fit();
        // On a new screen, whatever was held for the last one (the tap that dismissed a menu)
        // does nothing until it's let go
        if game.screen_gen != screen_gen {
            screen_gen = game.screen_gen;
            input.ignore_current();
            pending = Pending::default();
            if let Some(world) = game.world.as_mut() {
                world.kitty.forget_input();
            }
        }
        // Also notices the app being put away (Home, a call, a system dialog, a hidden browser
        // tab), so play can pause instead of carrying on without the player
        let (controls, was_away) = input.poll(&view);
        audio.update();
        let dt = get_frame_time().min(MAX_FRAME_DT);
        game.screen_time += dt;
        clear_background(Color::new(0.06, 0.06, 0.09, 1.0));

        match &mut game.screen {
            Screen::Title => {
                set_camera(&view.screen_camera());
                story::draw_title(&assets, game.screen_time, game.high_score);
                if controls.back {
                    platform::send_to_background();
                } else if controls.any_tap && game.screen_time > 0.4 {
                    game.new_game();
                }
            }
            Screen::Story { pages, page, ending } => {
                set_camera(&view.screen_camera());
                story::draw_page(&assets, &pages[*page], game.screen_time);
                if controls.any_tap && game.screen_time > 0.4 {
                    *page += 1;
                    game.screen_time = 0.0;
                    if *page >= pages.len() {
                        if *ending {
                            game.finish_game();
                            // Another lap through the woods, harder this time
                            game.lap += 1;
                            game.level = 1;
                            game.friends_rescued = 0;
                            game.start_level();
                        } else {
                            game.start_level();
                        }
                    }
                }
            }
            Screen::LevelIntro => {
                set_camera(&view.screen_camera());
                story::draw_level_intro(&assets, game.level, &story::level_info(game.level), game.screen_time);
                if controls.any_tap && game.screen_time > 0.5 {
                    accumulator = 0.0;
                    game.go(Screen::Playing);
                }
            }
            Screen::Playing => {
                // Fixed-rate physics; a long pause can't fling the branches. Some frames run no
                // step at all, so presses and releases wait for the next one instead of being
                // dropped.
                pending.add(&controls);
                accumulator += dt;
                let mut outcome = Outcome::Playing;
                let world = game.world.as_mut().unwrap();
                while accumulator >= STEP {
                    let step_controls = pending.take(&controls);
                    accumulator -= STEP;
                    outcome = world.step(&step_controls);
                    world.collide_with_assets(&assets);
                    for sfx in world.sfx.drain(..) {
                        audio.play(sfx);
                    }
                    if outcome != Outcome::Playing {
                        break;
                    }
                }
                world.animate(dt);
                world.draw(&assets, &view, accumulator / STEP);
                set_camera(&view.screen_camera());
                let hud = game.hud(game.world.as_ref().unwrap());
                hud::draw(&assets, &hud, game.screen_time);
                input.draw();
                let world = game.world.as_ref().unwrap();
                if controls.pause || controls.back || was_away {
                    game.go(Screen::Paused);
                } else if outcome == Outcome::OutOfHearts {
                    game.lives = game.lives.saturating_sub(1);
                    if game.lives == 0 {
                        game.score += world.points;
                        game.finish_game();
                        game.go(Screen::GameOver);
                    } else {
                        game.go(Screen::LifeLost { timer: 0.0 });
                    }
                } else if world.rescue_done() {
                    game.friends_rescued += 1;
                    let tally = game.tally();
                    game.go(Screen::LevelClear { tally });
                }
            }
            Screen::Paused => {
                let world = game.world.as_ref().unwrap();
                world.draw(&assets, &view, 1.0);
                set_camera(&view.screen_camera());
                story::draw_paused(game.screen_time);
                if (controls.pause || controls.back || controls.any_tap) && game.screen_time > 0.3 {
                    accumulator = 0.0;
                    game.go(Screen::Playing);
                }
            }
            Screen::LifeLost { timer } => {
                *timer += dt;
                let timer = *timer;
                let world = game.world.as_mut().unwrap();
                world.animate(dt);
                world.draw(&assets, &view, 1.0);
                set_camera(&view.screen_camera());
                story::draw_life_lost(game.lives, timer);
                if timer > 1.6 {
                    game.world.as_mut().unwrap().respawn_kitty();
                    accumulator = 0.0;
                    game.go(Screen::Playing);
                }
            }
            Screen::LevelClear { tally } => {
                let tally = *tally;
                set_camera(&view.screen_camera());
                story::draw_level_clear(&assets, &story::level_info(game.level), &tally, game.screen_time);
                if controls.any_tap && game.screen_time > 0.8 {
                    if game.level >= STORY_LEVELS {
                        game.go(Screen::Story { pages: story::ending_pages(), page: 0, ending: true });
                    } else {
                        game.level += 1;
                        game.start_level();
                    }
                }
            }
            Screen::GameOver => {
                set_camera(&view.screen_camera());
                story::draw_game_over(&assets, game.score, game.previous_best, game.screen_time);
                if (controls.any_tap || controls.back) && game.screen_time > 1.0 {
                    game.go(Screen::Title);
                }
            }
        }

        set_default_camera();
        next_frame().await;
    }
}

/// Android entry point: the Java activity loads libkitty_mobile.so and miniquad calls this.
#[cfg(target_os = "android")]
#[macroquad::main(window_conf)]
async fn main() {
    run().await;
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn quad_main() {
    main();
}
