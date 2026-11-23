//! Kitty Adventure: The Treetop Rescue.
//!
//! A storm scattered Kitty's five friends across Whisker Wood. The map climbs through five regions
//! of three levels each; every level is a stretch of forest three screens wide: climb the bendy
//! branches (from the shared Rust engine in `Engines/`), dodge squirrels, dogs, crows, eagles and
//! snakes, and reach the clue (or, at a region's end, the friend) at the top. Then the endless
//! climb.
//!
//! The same code runs on desktop, in the browser (wasm) and on Android, where the library is
//! loaded by the Gradle-built app (see `android/`).

pub mod audio;
pub mod camera;
pub mod campaign;
pub mod consts;
pub mod effects;
pub mod enemies;
pub mod flow;
pub mod hud;
pub mod input;
pub mod items;
pub mod kitty;
pub mod level;
pub mod limb;
pub mod look;
pub mod map;
pub mod parallax;
pub mod platform;
pub mod rng;
pub mod save;
pub mod sprites;
pub mod story;
pub mod tree;
pub mod ui;
pub mod world;

use macroquad::prelude::*;

use crate::audio::Audio;
use crate::campaign::{Goal, LevelId};
use crate::camera::View;
use crate::consts::*;
use crate::flow::ClearOutcome;
use crate::hud::HudState;
use crate::input::menu::{MenuInput, Nav};
use crate::input::{Input, Pending};
use crate::level::ENDLESS_LEVELS;
use crate::map::{MapAction, MapState, Stop};
use crate::save::SaveData;
use crate::sprites::Assets;
use crate::story::{paint, ClearCard, ClueTrack, IntroCard, Speaker, Tally, FRIENDS};
use crate::ui::Style;
use crate::world::{NestGoal, Outcome, Setup, World};

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

/// What's being played.
#[derive(Clone, Copy)]
enum Run {
    Level(LevelId),
    /// The endless climb: `level` 1..=5 of its ramp on `lap` (from 1, so every critter's out), the
    /// score so far and which climb of the run this is.
    Endless { level: usize, lap: usize, score: u32, climb: u32 },
}

impl Run {
    fn setup(self, seed: u64) -> Setup {
        match self {
            Run::Level(id) => id.def().setup(),
            Run::Endless { level, lap, .. } => Setup::endless(level, lap, seed),
        }
    }
}

/// Where a story goes once its last page is turned.
enum AfterStory {
    Map,
    /// The ending: the endless climb has opened.
    EndlessOpened,
}

enum Screen {
    Title,
    Story { pages: Vec<story::Page>, page: usize, then: AfterStory },
    Map,
    LevelIntro,
    Playing,
    Paused { focus: usize },
    TryAgain { focus: usize },
    LevelClear { tally: Tally, outcome: Option<ClearOutcome>, focus: usize },
    Clue { id: LevelId },
    EndlessOver { score: u32, best: u32, new_best: bool, climb: u32, focus: usize },
}

struct Game {
    screen: Screen,
    screen_time: f32,
    world: Option<World>,
    run: Option<Run>,
    save: SaveData,
    map: MapState,
    seed: u64,
    /// Bumped on every screen change, so input held across one can be ignored.
    screen_gen: u32,
    /// After a clear that opened a stone: Kitty's hop to it, for the next visit to the map.
    walk: Option<(Stop, Stop)>,
    /// Taps on the map's title, and when the last was (the playtest unlock).
    title_taps: (u32, f64),
}

impl Game {
    fn new() -> Self {
        let save = save::load();
        Self {
            screen: Screen::Title,
            screen_time: 0.0,
            world: None,
            run: None,
            map: MapState::new(&save),
            save,
            seed: (macroquad::miniquad::date::now() * 1000.0) as u64,
            screen_gen: 0,
            walk: None,
            title_taps: (0, 0.0),
        }
    }

    fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.screen_time = 0.0;
        self.screen_gen = self.screen_gen.wrapping_add(1);
    }

    fn persist(&mut self) {
        save::write(&mut self.save);
    }

    /// Play `run`'s level: its intro first.
    fn start(&mut self, run: Run) {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.world = Some(World::new(&run.setup(self.seed)));
        self.run = Some(run);
        self.go(Screen::LevelIntro);
    }

    /// The same level again, fresh from the bottom (snapped branches back), straight into play.
    fn retry(&mut self) {
        let Some(run) = self.run else { return };
        self.world = Some(World::new(&run.setup(self.seed)));
        self.go(Screen::Playing);
    }

    fn go_to_map(&mut self) {
        self.world = None;
        self.run = None;
        self.map.enter(&self.save, self.walk.take());
        self.go(Screen::Map);
    }

    fn new_endless_run(&mut self) {
        self.start(Run::Endless { level: 1, lap: 1, score: 0, climb: 1 });
    }

    fn intro_card(&self) -> IntroCard<'static> {
        if let Some(Run::Level(id)) = self.run {
            let def = id.def();
            let region = def.region.info();
            return IntroCard {
                    badge: format!("{} · {} OF 3", region.name.to_uppercase(), def.stage),
                    title: def.title,
                    speaker: match def.goal {
                        Goal::Clue(_) => Speaker::Kitty,
                        Goal::Rescue => Speaker::Friend(def.friend()),
                    },
                    line: def.intro,
                    watch_out: def.watch_out,
                    watch_tip: def.watch_tip,
                    is_new: def.introduces == Some(def.watch_out),
                    basics: id.0 == 0,
                    track: Some(ClueTrack { friend: def.friend(), found: flow::clues_found(&self.save, def.region), stage: def.stage }),
            };
        }
        let (level, climb) = match self.run {
            Some(Run::Endless { level, climb, .. }) => (level, climb),
            _ => (1, 1),
        };
        let info = story::level_info(level);
        IntroCard {
            badge: format!("ENDLESS · CLIMB {climb}"),
            title: "Endless Climb",
            speaker: Speaker::Friend(info.friend),
            line: info.intro,
            watch_out: info.threat,
            watch_tip: None,
            is_new: false,
            basics: false,
            track: None,
        }
    }

    fn hud(&self, world: &World) -> HudState {
        let (label, title, star_time, score) = match self.run {
            Some(Run::Level(id)) => {
                let def = id.def();
                (format!("{} {}/3", def.region.info().name.to_uppercase(), def.stage), def.title.to_string(), Some(def.star_time), world.points)
            }
            Some(Run::Endless { score, climb, .. }) => (format!("ENDLESS · CLIMB {climb}"), "Endless Climb".to_string(), None, score + world.points),
            None => (String::new(), String::new(), None, world.points),
        };
        HudState {
            hearts: world.kitty.hearts,
            max_hearts: MAX_HEARTS,
            score,
            label,
            title,
            friend: world.nest.friend(),
            rescued: FRIENDS.map(|friend| flow::rescued(&self.save, friend)),
            clue: matches!(world.nest, NestGoal::Clue { .. }),
            elapsed: world.elapsed,
            star_time,
            kitty_x: world.kitty.x,
            goal_x: world.nest_position().x,
            view_x: world.camera.x,
            climb: (-world.kitty.y / -world.top_y()).clamp(0.0, 1.0),
            invulnerable: world.kitty.invulnerable > 0,
        }
    }

    /// The level's goal has had its moment: score it, record it, and show the tally.
    fn finish_level(&mut self) {
        let result = self.world.as_ref().unwrap().result();
        match self.run {
            Some(Run::Level(id)) => {
                let tally = flow::tally(flow::par(id), &result, 0);
                let outcome = flow::record_clear(&mut self.save, id, &result, tally.level_total);
                self.persist();
                self.walk = outcome.unlocked.map(|next| (Stop::Level(id), Stop::Level(next)));
                self.go(Screen::LevelClear { tally, outcome: Some(outcome), focus: 1 });
            }
            Some(Run::Endless { level, lap, score, climb }) => {
                let tally = flow::tally(RESCUE_PAR_SECONDS, &result, score);
                self.run = Some(Run::Endless { level, lap, score: tally.score, climb });
                self.go(Screen::LevelClear { tally, outcome: None, focus: 0 });
            }
            None => self.go_to_map(),
        }
    }

    /// An endless run is over: keep its best and show how it went.
    fn end_endless(&mut self) {
        let Some(Run::Endless { score, climb, .. }) = self.run else { return };
        let score = score + self.world.as_ref().map_or(0, |w| w.points);
        let new_best = flow::record_endless(&mut self.save, score, climb);
        self.persist();
        let best = self.save.endless.best_score;
        self.go(Screen::EndlessOver { score, best, new_best, climb, focus: 0 });
    }

    /// Move on from a level-clear card.
    fn carry_on(&mut self, outcome: Option<ClearOutcome>) {
        match self.run {
            Some(Run::Level(id)) => {
                if id.def().clue().is_some() {
                    self.go(Screen::Clue { id });
                } else if outcome.is_some_and(|o| o.endless_unlocked) {
                    self.walk = Some((Stop::Level(id), Stop::Endless));
                    self.go(Screen::Story { pages: story::ending_pages(), page: 0, then: AfterStory::EndlessOpened });
                } else {
                    self.go_to_map();
                }
            }
            Some(Run::Endless { level, lap, score, climb }) => {
                let (level, lap) = if level >= ENDLESS_LEVELS { (1, lap + 1) } else { (level + 1, lap) };
                self.start(Run::Endless { level, lap, score, climb: climb + 1 });
            }
            None => self.go_to_map(),
        }
    }

    /// Playtest builds: seven quick taps on the map's title open every level.
    fn count_title_tap(&mut self, menu: &MenuInput) {
        if !cfg!(feature = "playtest") || !menu.tapped(map::TITLE) {
            return;
        }
        let now = macroquad::miniquad::date::now();
        let (count, last) = self.title_taps;
        let count = if now - last < 0.6 { count + 1 } else { 1 };
        self.title_taps = (count, now);
        if count >= 7 && !self.save.unlock_all {
            self.save.unlock_all = true;
            self.persist();
            self.map.say("Every level is open!");
        }
    }
}

/// Which button the keyboard's focus moves to, or the one tapped, of `buttons`.
fn buttons_pressed(menu: &MenuInput, buttons: &[Rect], focus: &mut usize) -> Option<usize> {
    *focus = ui::focus_step(menu.nav, *focus, buttons.len());
    buttons.iter().enumerate().position(|(i, &r)| ui::pressed(menu, r, i == *focus))
}

/// Draw `buttons` with their labels, the keyboard's focus outlined once keys have been used.
fn draw_buttons(menu: &MenuInput, buttons: &[(Rect, &str, Style)], focus: usize, keyed: bool, alpha: f32) {
    for (i, &(r, label, style)) in buttons.iter().enumerate() {
        ui::draw_button(menu, r, label, style, keyed && i == focus, alpha);
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
    // Whether the keyboard has been used for menus, so its focus shows
    let mut keyed = false;

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
        let menu = input.menu();
        keyed = (keyed || menu.nav.is_some()) && menu.tap.is_none();
        let back = controls.back || menu.nav == Some(Nav::Back);
        audio.update();
        let dt = get_frame_time().min(MAX_FRAME_DT);
        game.screen_time += dt;
        clear_background(Color::new(0.06, 0.06, 0.09, 1.0));

        match &mut game.screen {
            Screen::Title => {
                set_camera(&view.screen_camera());
                story::draw_title(&assets, game.screen_time, flow::total_stars(&game.save), game.save.endless.best_score);
                if controls.back {
                    platform::send_to_background();
                } else if controls.any_tap && game.screen_time > 0.4 {
                    if game.save.seen_intro {
                        game.go_to_map();
                    } else {
                        game.go(Screen::Story { pages: story::intro_pages(), page: 0, then: AfterStory::Map });
                    }
                }
            }
            Screen::Story { pages, page, then } => {
                set_camera(&view.screen_camera());
                story::draw_page(&assets, &pages[*page], game.screen_time);
                let skip = back && game.screen_time > 0.2;
                if (controls.any_tap && game.screen_time > 0.4) || skip {
                    *page = if skip { pages.len() } else { *page + 1 };
                    game.screen_time = 0.0;
                    if *page >= pages.len() {
                        if matches!(then, AfterStory::Map) {
                            game.save.seen_intro = true;
                            game.persist();
                        }
                        game.go_to_map();
                    }
                }
            }
            Screen::Map => {
                set_camera(&view.screen_camera());
                game.count_title_tap(&menu);
                let action = game.map.update(&menu, &game.save, dt);
                map::draw(&assets, &game.map, &game.save, &menu, game.screen_time);
                match action {
                    Some(MapAction::Play(id)) => game.start(Run::Level(id)),
                    Some(MapAction::PlayEndless) => game.new_endless_run(),
                    Some(MapAction::Back) => game.go(Screen::Title),
                    None => {}
                }
            }
            Screen::LevelIntro => {
                set_camera(&view.screen_camera());
                let card = game.intro_card();
                story::draw_level_intro(&assets, &card, game.screen_time, input.is_touch());
                if back || controls.pause {
                    game.go_to_map();
                } else if controls.any_tap && game.screen_time > story::level_intro_wait(card.basics) {
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
                if world.reached {
                    let found = match game.run {
                        Some(Run::Level(id)) => id.def().clue().map(|clue| clue.found),
                        _ => None,
                    };
                    story::draw_goal_banner(&assets, world.nest, found, world.goal_time());
                }
                let hud = game.hud(game.world.as_ref().unwrap());
                hud::draw(&assets, &hud, game.screen_time);
                input.draw();
                let world = game.world.as_ref().unwrap();
                if controls.pause || controls.back || was_away {
                    game.go(Screen::Paused { focus: 0 });
                } else if outcome == Outcome::OutOfHearts {
                    match game.run {
                        Some(Run::Endless { .. }) => game.end_endless(),
                        _ => game.go(Screen::TryAgain { focus: 0 }),
                    }
                } else if world.goal_done() {
                    game.finish_level();
                }
            }
            Screen::Paused { focus } => {
                let world = game.world.as_ref().unwrap();
                world.draw(&assets, &view, 1.0);
                set_camera(&view.screen_camera());
                story::draw_paused(&assets, game.screen_time, input.is_touch());
                let [resume, retry, map_button] = ui::pause_buttons();
                let endless = matches!(game.run, Some(Run::Endless { .. }));
                let end_run = Rect::new(retry.x, retry.y, map_button.right() - retry.x, retry.h);
                let buttons: Vec<(Rect, &str, Style)> = if endless {
                    vec![(resume, "Resume", Style::Primary), (end_run, "End run", Style::Secondary)]
                } else {
                    vec![(resume, "Resume", Style::Primary), (retry, "Retry", Style::Secondary), (map_button, "Map", Style::Secondary)]
                };
                let rects: Vec<Rect> = buttons.iter().map(|b| b.0).collect();
                let ready = game.screen_time > 0.3;
                let pressed = if ready { buttons_pressed(&menu, &rects, focus) } else { None };
                draw_buttons(&menu, &buttons, *focus, keyed, paint::appear(game.screen_time, 0.0, 0.2));
                if ready && (controls.pause || controls.back || menu.nav == Some(Nav::Back)) || pressed == Some(0) {
                    accumulator = 0.0;
                    game.go(Screen::Playing);
                } else if pressed == Some(1) && endless {
                    game.end_endless();
                } else if pressed == Some(1) {
                    accumulator = 0.0;
                    game.retry();
                } else if pressed == Some(2) {
                    game.go_to_map();
                }
            }
            Screen::TryAgain { focus } => {
                let world = game.world.as_mut().unwrap();
                world.animate(dt);
                world.draw(&assets, &view, 1.0);
                set_camera(&view.screen_camera());
                story::draw_try_again(&assets, game.screen_time);
                let [retry, map_button] = ui::try_again_buttons();
                let buttons = [(retry, "Retry", Style::Primary), (map_button, "Map", Style::Secondary)];
                let ready = game.screen_time > 0.6;
                let pressed = if ready { buttons_pressed(&menu, &[retry, map_button], focus) } else { None };
                draw_buttons(&menu, &buttons, *focus, keyed, paint::appear(game.screen_time, 0.3, 0.3));
                if pressed == Some(0) {
                    accumulator = 0.0;
                    game.retry();
                } else if pressed == Some(1) || (ready && back) {
                    game.go_to_map();
                }
            }
            Screen::LevelClear { tally, outcome, focus } => {
                let (tally, outcome) = (*tally, *outcome);
                set_camera(&view.screen_camera());
                let card = match game.run {
                    Some(Run::Level(id)) => {
                        let def = id.def();
                        let friend = def.friend();
                        let (line, speaker) = match &def.goal {
                            Goal::Clue(clue) => (clue.found, "Kitty"),
                            Goal::Rescue => (story::friend_info(friend).thanks, friend.full_name()),
                        };
                        ClearCard {
                            title: def.title,
                            nest: def.nest(),
                            line,
                            speaker,
                            stars: outcome.map(|o| (o.stars, o.new_stars)),
                            star_time: def.star_time,
                            new_best: outcome.is_some_and(|o| o.new_best),
                            run_score: false,
                        }
                    }
                    _ => {
                        let level = match game.run {
                            Some(Run::Endless { level, .. }) => level,
                            _ => 1,
                        };
                        let info = story::level_info(level);
                        ClearCard {
                            title: "Endless Climb",
                            nest: NestGoal::Rescue(info.friend),
                            line: info.thanks,
                            speaker: info.friend.full_name(),
                            stars: None,
                            star_time: 0.0,
                            new_best: false,
                            run_score: true,
                        }
                    }
                };
                story::draw_level_clear(&assets, &card, &tally, game.screen_time);
                let endless = matches!(game.run, Some(Run::Endless { .. }));
                let [retry, carry] = ui::clear_buttons();
                let buttons: Vec<(Rect, &str, Style)> = if endless {
                    vec![(Rect::new(150.0, carry.y, 300.0, carry.h), "Keep climbing", Style::Primary)]
                } else {
                    vec![(retry, "Retry", Style::Secondary), (carry, "Continue", Style::Primary)]
                };
                let rects: Vec<Rect> = buttons.iter().map(|b| b.0).collect();
                let ready = game.screen_time > 1.0;
                let pressed = if ready { buttons_pressed(&menu, &rects, focus) } else { None };
                draw_buttons(&menu, &buttons, *focus, keyed, paint::appear(game.screen_time, 0.8, 0.3));
                let continue_index = rects.len() - 1;
                if pressed == Some(continue_index) {
                    game.carry_on(outcome);
                } else if pressed == Some(0) {
                    accumulator = 0.0;
                    game.retry();
                } else if ready && back {
                    if endless { game.end_endless() } else { game.go_to_map() }
                } else if menu.tap.is_some() && game.screen_time < story::CLEAR_COUNTED {
                    // A tap off the buttons hurries the count-up along
                    game.screen_time = story::CLEAR_COUNTED;
                }
            }
            Screen::Clue { id } => {
                let id = *id;
                set_camera(&view.screen_camera());
                let def = id.def();
                let track = ClueTrack { friend: def.friend(), found: flow::clues_found(&game.save, def.region), stage: def.stage };
                if let Some(clue) = def.clue() {
                    story::draw_clue_card(&assets, def.region.info(), def.title, clue, track, game.screen_time);
                }
                if (controls.any_tap && game.screen_time > 0.9) || (back && game.screen_time > 0.3) {
                    game.go_to_map();
                }
            }
            Screen::EndlessOver { score, best, new_best, climb, focus } => {
                set_camera(&view.screen_camera());
                story::draw_endless_over(&assets, *score, *best, *new_best, *climb, game.screen_time);
                let [retry, map_button] = ui::endless_over_buttons();
                let buttons = [(retry, "Again", Style::Primary), (map_button, "Map", Style::Secondary)];
                let ready = game.screen_time > 1.0;
                let pressed = if ready { buttons_pressed(&menu, &[retry, map_button], focus) } else { None };
                draw_buttons(&menu, &buttons, *focus, keyed, paint::appear(game.screen_time, 0.8, 0.3));
                if pressed == Some(0) {
                    game.new_endless_run();
                } else if pressed == Some(1) || (ready && back) {
                    game.go_to_map();
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
