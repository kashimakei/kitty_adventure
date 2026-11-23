//! The story: one blustery night the wind swept Kitty's five friends up to the tops of the
//! tallest trees, and Kitty climbs to bring each one home. Story text, level titles and the menu
//! screens live here, along with the drawing helpers (`paint`) they share with the HUD. Every
//! draw function works in playfield coordinates (the screen camera).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::*;

use crate::campaign::{Clue, ClueObject, Region};
use crate::consts::*;
use crate::kitty::{RING_FILL, RING_FULL, RING_SWEET, RING_TRACK};
use crate::save::Stars;
use crate::sprites::{Assets, Sprites};
use crate::world::NestGoal;
use paint::*;

/// Level-clear tally timing (seconds): when the first row appears, the gap between rows, and how
/// long each number takes to count up.
const TALLY_START: f32 = 0.7;
const TALLY_ROW: f32 = 0.45;
const TALLY_COUNT: f32 = 0.4;

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
        FRIENDS[level.saturating_sub(1) % FRIENDS.len()]
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

    /// "Pip the bunny", for name tags.
    pub fn full_name(self) -> &'static str {
        match self {
            Friend::Pip => "Pip the bunny",
            Friend::Hazel => "Hazel the hedgehog",
            Friend::Mochi => "Mochi the hamster",
            Friend::Sunny => "Sunny the duckling",
            Friend::Luna => "Luna the kitten",
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

    /// The cheer frames that read as happy, in order. Pip's row opens with a worried-looking
    /// leap and Hazel's has a stray scrap of leaf sliced out as frame 1, so those are left out.
    fn cheer_cycle(self) -> &'static [usize] {
        match self {
            Friend::Pip => &[1, 2, 3],
            Friend::Hazel => &[0, 2, 3],
            _ => &[0, 1, 2],
        }
    }

    /// How many cheer frames it cycles through (see `cheer_frame`).
    pub fn cheer_frames(self) -> usize {
        self.cheer_cycle().len()
    }

    /// The name of cheer frame `i`, wrapping around.
    pub fn cheer_frame(self, i: usize) -> String {
        let cycle = self.cheer_cycle();
        format!("{}_cheer_{}", self.sprite(), cycle[i % cycle.len()])
    }

    /// Frame `i` of it tumbling frightened on the wind, wrapping around. Mochi's first pose is
    /// nearly upright, as if sitting, and Sunny's middle one squeezes her eyes shut like her
    /// laughing cheer, so those are left out.
    pub fn storm_frame(self, i: usize) -> String {
        let cycle: &[usize] = match self {
            Friend::Mochi => &[1, 2],
            Friend::Sunny => &[0, 2],
            _ => &[0, 1, 2],
        };
        format!("{}_storm_{}", self.sprite(), cycle[i % cycle.len()])
    }

    pub fn help_frame(self) -> String {
        format!("{}_help_0", self.sprite())
    }
}

/// The new danger each story level brings (later laps bring them all).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Threat {
    Squirrels,
    Dogs,
    Snakes,
    Crows,
    Eagles,
}

impl Threat {
    pub fn name(self) -> &'static str {
        match self {
            Threat::Squirrels => "Squirrels",
            Threat::Dogs => "Dogs",
            Threat::Snakes => "Snakes",
            Threat::Crows => "Crows",
            Threat::Eagles => "Eagles",
        }
    }

    pub fn tip(self) -> &'static str {
        match self {
            Threat::Squirrels => "Cheeky squirrels toss acorns from the branches. Dodge them, or bop the squirrel from above!",
            Threat::Dogs => "Dogs patrol the ground and leap up at low branches. Bounce out of reach: hold as you land, let go as the ring glows blue!",
            Threat::Snakes => "Snakes lie coiled on branches and strike when you come close. Hop over them, or bop them on the head!",
            Threat::Crows => "Crows flap overhead and dive beak-first! Step aside when one hovers, or bop it from above, even in flight!",
            Threat::Eagles => "Eagles sweep in fast from the edge of the sky. Dodge their dive, or pounce on an eagle's back to bop it!",
        }
    }

    /// A frame that shows it off, for the level intro's tip card.
    pub(crate) fn icon(self, assets: &Assets) -> (&Sprites, &'static str) {
        match self {
            Threat::Squirrels => (&assets.squirrel, "idle_0"),
            Threat::Dogs => (&assets.dog, "sniff_0"),
            Threat::Snakes => (&assets.snake, "coiled_0"),
            Threat::Crows => (&assets.crow, "fly_0"),
            Threat::Eagles => (&assets.eagle, "flap_0"),
        }
    }
}

/// What each friend says, and the critter their old story level brought (the endless climb uses
/// these).
pub struct LevelInfo {
    pub friend: Friend,
    /// What the friend calls down from the nest.
    pub intro: &'static str,
    pub thanks: &'static str,
    pub threat: Threat,
}

/// What the friend waiting on an endless climb's `level` (1..=5, round and round) says.
pub fn level_info(level: usize) -> LevelInfo {
    friend_info(Friend::for_level(level))
}

pub fn friend_info(friend: Friend) -> LevelInfo {
    let (intro, thanks, threat) = match friend {
        Friend::Pip => (
            "Kitty! Up here! The wind blew me sooo high, and my ears are all in a tangle!",
            "Hooray! You hop just like a bunny, Kitty! Thank you!",
            Threat::Squirrels,
        ),
        Friend::Hazel => (
            "Hello down there! I'm a bit too prickly to hug, but I would really love a rescue!",
            "You're the bravest cat in the whole forest! Thank you, Kitty!",
            Threat::Dogs,
        ),
        Friend::Mochi => (
            "I packed my cheeks with seeds for the trip... I just didn't plan on the trip being up a tree!",
            "Thank you! Want a sunflower seed? I saved you the biggest one!",
            Threat::Snakes,
        ),
        Friend::Sunny => (
            "Peep peep! My wings are still too fluffy to fly. Can you come and get me, Kitty?",
            "Peep peep hooray! I knew you'd come. Thank you, Kitty!",
            Threat::Crows,
        ),
        Friend::Luna => (
            "Kitty, is that you? I can see the whole forest from up here... and it's a long way down!",
            "We're all together again! Thank you, Kitty, best friend ever!",
            Threat::Eagles,
        ),
    };
    LevelInfo { friend, intro, thanks, threat }
}

pub enum PageArt {
    /// The friends tumbling through the treetops on the wind: frightened the first time, giggling
    /// when the wind comes back at the end.
    Storm { scared: bool },
    /// Everyone stuck up in the trees, calling for help.
    AllFriends,
    /// Kitty at the foot of the trees, ready to climb.
    Kitty,
    /// One friend cheering in a nest.
    Friend(Friend),
    /// Everyone home, cheering around Kitty.
    Home,
    /// The party under the stars.
    Party,
}

pub struct Page {
    pub heading: &'static str,
    pub text: &'static str,
    pub art: PageArt,
    /// Which page this is (from 0) of how many, for the page dots.
    pub number: usize,
    pub count: usize,
}

fn numbered<const N: usize>(pages: [(&'static str, &'static str, PageArt); N]) -> Vec<Page> {
    pages.into_iter().enumerate()
        .map(|(number, (heading, text, art))| Page { heading, text, art, number, count: N })
        .collect()
}

pub fn intro_pages() -> Vec<Page> {
    numbered([
        ("A Windy Night",
         "One blustery night, a great big wind went whooshing through Whisker Wood. It tickled the trees and \
          rattled the leaves... and it swept Kitty's five best friends right up into the treetops!",
         PageArt::Storm { scared: true }),
        ("Stuck Up High",
         "In the morning, five little voices were calling from all over Whisker Wood: Pip, Hazel, Mochi, Sunny \
          and Luna, scattered far and wide and much too small to climb down by themselves.",
         PageArt::AllFriends),
        ("Kitty to the Rescue",
         "Kitty stretched her paws, fluffed up her tail and looked up, up, up. \"Hold on, everyone. I'll find \
          every one of you!\"",
         PageArt::Kitty),
    ])
}

pub fn ending_pages() -> Vec<Page> {
    numbered([
        ("Everyone Home",
         "With one last leap, Kitty helped Luna down from the very tallest tree. All five friends were safe \
          and sound at last!",
         PageArt::Home),
        ("A Starry Party",
         "That night they had a party under the stars, with acorn cake, berry juice and the coziest cuddle \
          pile in all of Whisker Wood.",
         PageArt::Party),
        ("The Wind's Game",
         "But listen... the wind is whistling through the branches again! \"Catch us if you can!\" giggle the \
          friends, tumbling into the treetops. A new game has begun: the Endless Climb. How high can Kitty go?",
         PageArt::Storm { scared: false }),
    ])
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

// ---------------------------------------------------------------------------------------------
// Screens

/// The title screen; `stars` (won, out of) and the endless best show once there are any.
pub fn draw_title(assets: &Assets, time: f32, stars: (usize, usize), endless_best: u32) {
    let t = ambient_time();
    backdrop(assets, Mood::Dusk);

    // The title drops in, then floats
    let drop = 1.0 - ease_out_back(time / 0.8);
    let y = 210.0 - drop * 260.0 + wave(t, 0.9, 5.0, 0.0);
    let size = fit_size("Kitty", 150.0, 520.0);
    outlined("Kitty", 300.0, y, size, ORANGE_TEXT, INK, Align::Center);
    let size = fit_size("Adventure", 92.0, 540.0);
    outlined("Adventure", 300.0, y + 96.0, size, GOLD_TEXT, INK, Align::Center);
    let ribbon_alpha = appear(time, 0.5, 0.5);
    ribbon("The Treetop Rescue", vec2(300.0, y + 160.0), 30.0, ribbon_alpha, Ribbon::Leaf);

    title_scene(assets, t, time);

    if time > 0.9 {
        tap_prompt("Tap to start", 990.0, t);
    }
    let score_alpha = appear(time, 1.0, 0.6);
    if stars.0 > 0 || endless_best > 0 {
        let label = if endless_best > 0 {
            format!("{} of {}   Best  {}", stars.0, stars.1, thousands(endless_best))
        } else {
            format!("{} of {} stars", stars.0, stars.1)
        };
        let w = measure(&label, 28.0) + 90.0;
        let r = Rect::new(300.0 - w / 2.0, 1060.0, w, 56.0);
        panel(r, 28.0, score_alpha);
        crate::ui::star(vec2(r.x + 36.0, r.y + 28.0), 16.0, fade(GOLD_TEXT, score_alpha), fade(INK, score_alpha));
        shadowed(&label, r.x + 62.0, r.y + 38.0, 28.0, fade(GOLD_TEXT, score_alpha), Align::Left);
    } else {
        shadowed("Help Kitty bring her friends home!", 300.0, 1092.0, 22.0, fade(CREAM, score_alpha * 0.9),
                 Align::Center);
    }
}

/// Kitty on a big branch while her friends peek out of the leaves.
fn title_scene(assets: &Assets, t: f32, time: f32) {
    let branch_y = |x: f32| 790.0 + (x * 0.012).sin() * 8.0 + (x - 300.0) * 0.03;
    let rise = 1.0 - appear(time, 0.2, 0.8);
    let lift = rise * 500.0;

    // The branch grows from a trunk off the right edge
    draw_trunk(632.0, -40.0 + lift, VIEW_H, 120.0, 1.0);
    // Friends behind the leaves, each popping up in turn (never quite out of sight)
    let spots = [(42.0, 0.0), (132.0, 1.3), (410.0, 2.6), (492.0, 0.7), (572.0, 1.9)];
    for (&(x, phase), friend) in spots.iter().zip(FRIENDS) {
        let peek = ((t * 0.9 + phase).sin() * 0.5 + 0.5).powf(0.6);
        let at = vec2(x, branch_y(x) + 6.0 - peek * 46.0 + lift);
        let facing = if x < 300.0 { 1.0 } else { -1.0 };
        let name = if peek > 0.85 { friend.cheer_frame(0) } else { friend.help_frame() };
        assets.friends.draw_ex(&name, at, facing, 0.9, 0.0, WHITE);
    }
    draw_branch(-30.0, 610.0, branch_y, lift, 42.0);
    for (i, &(x, _)) in spots.iter().enumerate() {
        let sway = (t * 1.6 + i as f32).sin() * 0.06;
        let name = if i % 2 == 0 { "green_5" } else { "green_4" };
        assets.foliage.draw_ex(name, vec2(x, branch_y(x) + 4.0 + lift), 1.0, 1.35, sway, WHITE);
    }

    // Kitty's little routine: wave, sit, groom, look around
    let cycle = t % 7.0;
    let (frame, tilt) = match cycle {
        c if c < 1.6 => ("wave_0", (c * 7.0).sin() * 0.05),
        c if c < 3.2 => ("sit_0", 0.0),
        c if c < 4.8 => (if (c * 4.0) as i32 % 2 == 0 { "groom_0" } else { "groom_1" }, 0.0),
        c if c < 5.6 => ("clean_1", 0.0),
        _ => ("sit_alert_0", 0.0),
    };
    let x = 270.0;
    let at = vec2(x, branch_y(x) - 10.0 + lift);
    shadow_ellipse(at + vec2(0.0, 4.0), 70.0, 12.0, 0.25);
    assets.kitty.draw_ex(frame, at, 1.0, 1.9, tilt, WHITE);
}

pub fn draw_page(assets: &Assets, page: &Page, time: f32) {
    let t = ambient_time();
    let mood = match page.art {
        PageArt::Storm { .. } => Mood::Night,
        PageArt::Party => Mood::Party,
        _ => Mood::Day,
    };
    backdrop(assets, mood);

    // The picture
    let art_in = appear(time, 0.0, 0.6);
    let frame = Rect::new(40.0, 80.0 + (1.0 - art_in) * 40.0, 520.0, 520.0);
    picture_frame(frame, mood, art_in);
    clip(Some(grow(frame, -6.0)));
    page_art(assets, &page.art, frame, t, time, art_in);
    clip(None);
    rounded_outline(frame, 30.0, 6.0, fade(rgb(0.98, 0.90, 0.72), art_in));

    // The words, on a card sized to fit them
    let text_in = appear(time, 0.2, 0.6);
    let (size, spacing, width) = (27.0, 1.45, 470.0);
    let lines = wrap(page.text, size, width).len() as f32;
    let card = Rect::new(36.0, 640.0 + (1.0 - text_in) * 40.0, 528.0, 150.0 + lines * size * spacing);
    panel(card, 28.0, text_in);
    let heading = fit_size(page.heading, 46.0, card.w - 50.0);
    outlined(page.heading, 300.0, card.y + 68.0, heading, fade(GOLD_TEXT, text_in), fade(INK, text_in),
             Align::Center);
    paragraph(page.text, 300.0, card.y + 108.0, size, width, spacing, fade(CREAM, text_in));

    page_dots(page.number, page.count, 1045.0, t);
    if time > 0.5 {
        let last = page.number + 1 == page.count;
        tap_prompt(if last { "Tap to begin" } else { "Tap to continue" }, 1115.0, t);
    }
}

/// The picture-book frame each story page draws its art in.
pub(crate) fn picture_frame(r: Rect, mood: Mood, alpha: f32) {
    drop_shadow(r, 30.0, alpha);
    let (top, bottom) = match mood {
        Mood::Night => (rgb(0.10, 0.12, 0.30), rgb(0.18, 0.24, 0.40)),
        Mood::Party => (rgb(0.20, 0.16, 0.36), rgb(0.42, 0.30, 0.42)),
        _ => (rgb(0.55, 0.80, 0.95), rgb(0.80, 0.93, 0.88)),
    };
    rounded_rect(r, 30.0, fade(top, alpha), fade(bottom, alpha));
    // Rolling hills along the bottom
    let ground = if matches!(mood, Mood::Night) { rgb(0.12, 0.22, 0.18) } else { rgb(0.42, 0.66, 0.30) };
    let hill = Rect::new(r.x, r.bottom() - 120.0, r.w, 120.0);
    for (i, (dx, w)) in [(-0.1, 0.7), (0.45, 0.75)].into_iter().enumerate() {
        let shade = if i == 0 { 0.85 } else { 1.0 };
        let c = Color::new(ground.r * shade, ground.g * shade, ground.b * shade, alpha);
        let cx = hill.x + hill.w * (dx + w / 2.0);
        draw_ellipse(cx, hill.bottom() - 10.0, hill.w * w / 2.0 + 30.0, 70.0 + i as f32 * 14.0, 0.0, c);
    }
    rounded_rect(Rect::new(r.x, r.bottom() - 60.0, r.w, 60.0), 30.0, fade(ground, alpha), fade(ground, alpha));
}

fn page_art(assets: &Assets, art: &PageArt, r: Rect, t: f32, time: f32, alpha: f32) {
    let centre = r.center();
    let ground = r.bottom() - 46.0;
    let white = fade(WHITE, alpha);
    match art {
        PageArt::Storm { scared } => {
            // A moon, gusts, leaves and five friends tumbling across on the wind
            draw_circle(r.right() - 100.0, r.y + 100.0, 60.0, fade(rgb(0.30, 0.34, 0.60), alpha * 0.5));
            draw_circle(r.right() - 100.0, r.y + 100.0, 42.0, fade(rgb(1.0, 0.96, 0.80), alpha));
            for i in 0..5 {
                let y = r.y + 110.0 + i as f32 * 62.0;
                let x = r.x + ((t * 220.0 + i as f32 * 170.0) % (r.w + 300.0)) - 150.0;
                gust(vec2(x, y), 140.0, fade(WHITE, alpha * 0.35), r);
            }
            // Treetops bending in the gale
            for (i, x) in [r.x + 40.0, r.x + 150.0, r.right() - 60.0].into_iter().enumerate() {
                let bend = (t * 1.8 + i as f32 * 1.7).sin() * 0.12 + 0.18;
                let base = vec2(x, r.bottom() + 10.0);
                let height = 230.0 + i as f32 * 40.0;
                let top = base + vec2(bend * height, -height);
                draw_line(base.x, base.y, top.x, top.y, 22.0, fade(rgb(0.10, 0.08, 0.10), alpha));
                for (j, (dx, dy)) in [(-30.0, 10.0), (28.0, 6.0), (0.0, -24.0), (-6.0, 40.0)].into_iter().enumerate() {
                    let at = top + vec2(dx + bend * 40.0, dy);
                    let spin = bend + (t * 3.0 + j as f32).sin() * 0.1;
                    assets.foliage.draw_ex("green_5", at, 1.0, 1.4, spin, fade(rgb(0.30, 0.38, 0.55), alpha));
                }
            }
            for (i, friend) in FRIENDS.into_iter().enumerate() {
                let speed = 70.0 + i as f32 * 9.0;
                let x = r.x + ((t * speed + i as f32 * 120.0) % (r.w + 160.0)) - 80.0;
                let y = r.y + 150.0 + i as f32 * 52.0 + (t * 2.2 + i as f32).sin() * 26.0;
                // Frightened, they tumble head over heels through their storm poses (which turn
                // over on their own, so they need less extra spin); later they ride it giggling
                let (frame, spin) = if *scared {
                    (friend.storm_frame((t * 3.0 + i as f32 * 0.7) as usize), (t * 1.6 + i as f32 * 2.0).sin() * 0.3)
                } else {
                    (friend.cheer_frame(1), (t * 1.6 + i as f32 * 2.0).sin() * 0.6)
                };
                assets.friends.draw_ex(&frame, vec2(x, y), 1.0, 0.75, spin, white);
            }
            leaves_in(assets, r, t, 16, 2.6, alpha);
        }
        PageArt::AllFriends => {
            // Each friend in a nest of their own on branches of three tall trees, calling down
            let trunks = [r.x + 30.0, centre.x, r.right() - 30.0];
            for (i, &x) in trunks.iter().enumerate() {
                let top = if i == 1 { r.y + r.h * 0.26 } else { r.y - 20.0 };
                draw_trunk(x, top, r.bottom(), if i == 1 { 56.0 } else { 70.0 }, alpha);
            }
            for (i, dx) in [-60.0, 0.0, 60.0, -30.0, 30.0].into_iter().enumerate() {
                let y = r.y + r.h * 0.26 - if i < 3 { 10.0 } else { 50.0 };
                let sway = (t * 1.2 + i as f32).sin() * 0.06;
                assets.foliage.draw_ex("green_5", vec2(centre.x + dx, y), 1.0, 1.3, sway, white);
            }
            // (nest x, nest y, which trunk its branch grows from)
            let spots = [(0.20, 0.30, 0), (0.50, 0.20, 1), (0.80, 0.34, 2), (0.30, 0.60, 1), (0.72, 0.62, 1)];
            for (i, (&(fx, fy, trunk), friend)) in spots.iter().zip(FRIENDS).enumerate() {
                let at = vec2(r.x + r.w * fx, r.y + r.h * fy + 40.0);
                if i != 1 {
                    let from = trunks[trunk];
                    let (x0, x1) = if from < at.x { (from, at.x + 50.0) } else { (at.x - 50.0, from) };
                    draw_branch(x0, x1, |x| at.y + 6.0 + (x - at.x).abs() * 0.12, 0.0, 18.0);
                }
                let pop = appear(time, 0.2 + i as f32 * 0.15, 0.4);
                let bob = wave(t, 0.4, 3.0, i as f32);
                let facing = if fx < 0.5 { 1.0 } else { -1.0 };
                friend_in_nest(assets, &friend.help_frame(), at, 0.85 * pop, bob, facing, alpha);
            }
        }
        PageArt::Kitty => {
            // Kitty at the foot of a huge trunk, looking up at the friends far above
            let trunk = Rect::new(r.right() - 190.0, r.y, 90.0, r.h - 40.0);
            draw_trunk(trunk.center().x, r.y - 20.0, r.bottom(), trunk.w, alpha);
            draw_branch(trunk.x - 170.0, trunk.x + 10.0, |x| r.y + 150.0 + (x - trunk.x) * 0.08, 0.0, 22.0);
            draw_branch(trunk.right() - 10.0, r.right() + 20.0, |x| r.y + 260.0 - (x - trunk.right()) * 0.1, 0.0, 20.0);
            for (i, x) in [trunk.x - 150.0, trunk.x - 90.0, trunk.x - 30.0].into_iter().enumerate() {
                let sway = (t * 1.3 + i as f32).sin() * 0.08;
                assets.foliage.draw_ex(if i == 1 { "green_4" } else { "green_5" }, vec2(x, r.y + 150.0), 1.0, 1.2,
                                       sway, white);
            }
            for (i, x) in [trunk.x - 30.0, trunk.center().x, trunk.right() + 30.0].into_iter().enumerate() {
                let sway = (t * 1.1 + i as f32).sin() * 0.06;
                assets.foliage.draw_ex("green_5", vec2(x, r.y + 20.0 + i as f32 % 2.0 * 16.0), 1.0, 1.6, sway, white);
            }
            let tiny = |f: Friend, at: Vec2, i: f32| {
                let bob = wave(t, 2.5, 3.0, i);
                assets.friends.draw_ex(&f.help_frame(), at - vec2(0.0, bob), 1.0, 0.45, 0.0, white);
            };
            tiny(Friend::Pip, vec2(trunk.x - 120.0, r.y + 128.0), 0.0);
            tiny(Friend::Sunny, vec2(trunk.right() + 60.0, r.y + 250.0), 1.0);
            let kitty_frame = if (t % 4.0) < 2.6 { "sit_alert_0" } else { "wave_0" };
            let at = vec2(r.x + 190.0, ground);
            shadow_ellipse(at, 70.0, 12.0, 0.25 * alpha);
            assets.kitty.draw_ex(kitty_frame, at, 1.0, 2.0, 0.0, white);
            // A little exclamation of determination
            let pop = appear(time, 0.6, 0.3);
            if pop > 0.0 {
                outlined("!", at.x + 70.0, at.y - 170.0 - wave(t, 3.0, 4.0, 0.0), 60.0,
                         fade(GOLD_TEXT, pop * alpha), fade(INK, pop * alpha), Align::Center);
            }
        }
        PageArt::Friend(friend) => {
            let at = vec2(centre.x, centre.y + 60.0);
            draw_branch(r.x - 20.0, r.right() + 20.0, |x| at.y + 8.0 + (x - at.x) * 0.04, 0.0, 22.0);
            let name = friend.cheer_frame((t * 5.0) as usize);
            friend_in_nest(assets, &name, at, 1.6, (t * 5.0).sin().abs() * 14.0, 1.0, alpha);
        }
        PageArt::Home => {
            // Kitty in the middle of all her friends under a rainbow, everyone cheering
            let bands = [rgb(1.0, 0.45, 0.45), rgb(1.0, 0.70, 0.35), rgb(1.0, 0.92, 0.45), rgb(0.55, 0.85, 0.45),
                         rgb(0.45, 0.70, 1.0), rgb(0.70, 0.55, 0.95)];
            for (i, band) in bands.into_iter().enumerate() {
                let radius = 250.0 - i as f32 * 16.0;
                draw_arc(centre.x, ground + 40.0, 48, radius, 180.0, 16.0, 180.0, fade(band, 0.55 * alpha));
            }
            draw_circle(r.x + 90.0, r.y + 80.0, 46.0, fade(rgb(1.0, 0.95, 0.6), 0.35 * alpha));
            draw_circle(r.x + 90.0, r.y + 80.0, 32.0, fade(rgb(1.0, 0.90, 0.45), alpha));
            confetti(r, t, 40, alpha);
            let kitty_at = vec2(centre.x, ground);
            let row = [(-205.0, Friend::Pip), (-125.0, Friend::Hazel), (125.0, Friend::Mochi),
                       (205.0, Friend::Sunny)];
            for (i, &(dx, friend)) in row.iter().enumerate() {
                let hop = (t * 4.5 + i as f32 * 1.1).sin().abs() * 24.0;
                let at = vec2(centre.x + dx, ground - hop);
                shadow_ellipse(vec2(at.x, ground), 36.0, 7.0, 0.25 * alpha);
                let facing = if dx < 0.0 { 1.0 } else { -1.0 };
                assets.friends.draw_ex(&friend.cheer_frame((t * 5.0) as usize + i), at, facing, 1.15, 0.0, white);
            }
            // Luna rides on Kitty's back
            shadow_ellipse(kitty_at, 80.0, 13.0, 0.25 * alpha);
            assets.kitty.draw_ex("wave_0", kitty_at, 1.0, 1.9, 0.0, white);
            let luna_hop = (t * 4.0).sin().abs() * 10.0;
            assets.friends.draw_ex(&Friend::Luna.cheer_frame((t * 4.0) as usize), kitty_at - vec2(34.0, 132.0 + luna_hop),
                                   1.0, 0.85, 0.0, white);
        }
        PageArt::Party => {
            // Lanterns strung under the moon and stars, and everyone snug together on a log
            for i in 0..30 {
                let at = r.point() + vec2(hash(i + 3000) * r.w, hash(i + 3100) * r.h * 0.55);
                let twinkle = ((t * (1.0 + hash(i + 3200) * 2.0) + hash(i + 3300) * TAU).sin() * 0.5 + 0.5).powi(2);
                draw_circle(at.x, at.y, 1.5 + hash(i + 3400) * 1.5, fade(rgb(1.0, 0.97, 0.85), (0.3 + twinkle * 0.7) * alpha));
            }
            let moon = vec2(r.right() - 90.0, r.y + 150.0);
            draw_circle(moon.x, moon.y, 50.0, fade(rgb(0.55, 0.45, 0.75), 0.4 * alpha));
            draw_circle(moon.x, moon.y, 34.0, fade(rgb(1.0, 0.96, 0.80), alpha));
            draw_circle(moon.x + 14.0, moon.y - 8.0, 28.0, fade(rgb(0.24, 0.20, 0.40), alpha));
            for i in 0..7 {
                let x = r.x + 50.0 + i as f32 * 70.0;
                let y = r.y + 70.0 + ((i as f32 - 3.0) / 3.0).powi(2) * -30.0 + 30.0;
                let glow = 0.7 + (t * 2.0 + i as f32).sin() * 0.3;
                draw_circle(x, y, 18.0, fade(rgb(1.0, 0.85, 0.4), 0.25 * glow * alpha));
                draw_circle(x, y, 9.0, fade([rgb(1.0, 0.6, 0.4), rgb(1.0, 0.85, 0.4), rgb(0.7, 0.9, 1.0)][i % 3], alpha));
            }
            let log_y = ground - 30.0;
            draw_branch(r.x + 40.0, r.right() - 40.0, |_| log_y + 22.0, 0.0, 40.0);
            let seats = [(-205.0, Friend::Pip), (-130.0, Friend::Hazel), (130.0, Friend::Mochi), (205.0, Friend::Sunny)];
            for (i, &(dx, friend)) in seats.iter().enumerate() {
                let sway = wave(t, 0.5, 0.06, i as f32);
                let facing = if dx < 0.0 { 1.0 } else { -1.0 };
                let frame = friend.cheer_frame((t * 1.5 + i as f32) as usize);
                assets.friends.draw_ex(&frame, vec2(centre.x + dx, log_y + 8.0), facing, 1.1, sway, white);
            }
            assets.kitty.draw_ex("sit_0", vec2(centre.x - 24.0, log_y + 8.0), 1.0, 1.6, 0.0, white);
            assets.friends.draw_ex(&Friend::Luna.help_frame(), vec2(centre.x + 52.0, log_y + 8.0), -1.0, 1.0, 0.0, white);
            fireflies_in(r, t, 10, alpha);
        }
    }
}

/// When the level-one how-to-play card starts to fade in on the level intro, and how long it
/// takes to come in fully.
pub const BASICS_CARD_IN: (f32, f32) = (0.7, 0.5);

/// How long the level intro waits before a tap moves on (and the prompt shows). On level one it
/// waits until the how-to-play card is fully in: that card is the only place the controls are
/// taught, and a quick tapper would otherwise skip it before it appeared.
pub fn level_intro_wait(basics: bool) -> f32 {
    if basics { BASICS_CARD_IN.0 + BASICS_CARD_IN.1 + 0.1 } else { 0.5 }
}

/// Who speaks on a level's intro: the friend calling from the nest, or Kitty, looking for a clue.
#[derive(Clone, Copy)]
pub enum Speaker {
    Friend(Friend),
    Kitty,
}

/// A region's clues so far: two clue slots, then the friend.
#[derive(Clone, Copy)]
pub struct ClueTrack {
    pub friend: Friend,
    /// The region's two clues, and how many are found already.
    pub objects: [ClueObject; 2],
    pub found: usize,
    /// This level's place in the region, 1..=3.
    pub stage: usize,
}

/// What a level's intro card shows.
pub struct IntroCard<'a> {
    /// "MOSSY HOLLOW · 2 OF 3", or "ENDLESS · CLIMB 7".
    pub badge: String,
    pub title: &'a str,
    pub speaker: Speaker,
    pub line: &'a str,
    pub watch_out: Threat,
    pub watch_tip: Option<&'a str>,
    /// The critter it warns about is new here.
    pub is_new: bool,
    /// The very first level: how to play, too.
    pub basics: bool,
    pub track: Option<ClueTrack>,
}

pub fn draw_level_intro(assets: &Assets, card: &IntroCard, time: f32, touch: bool) {
    let t = ambient_time();
    backdrop(assets, Mood::Day);
    let first = card.basics;

    // Badge and title
    let badge_in = ease_out_back(time / 0.5);
    let w = measure(&card.badge, 24.0) + 48.0;
    let badge = Rect::new(300.0 - w / 2.0, 40.0 - (1.0 - badge_in) * 80.0, w, 46.0);
    rounded_rect(badge, 23.0, rgb(1.0, 0.78, 0.30), rgb(0.95, 0.58, 0.18));
    rounded_outline(badge, 23.0, 3.0, fade(INK, 0.6));
    text(&card.badge, 300.0, badge.y + 32.0, 24.0, INK, Align::Center);
    let title_in = appear(time, 0.1, 0.5);
    let size = fit_size(card.title, 60.0, 560.0);
    outlined(card.title, 300.0, 150.0 + (1.0 - title_in) * 20.0, size, fade(CREAM, title_in),
             fade(INK, title_in), Align::Center);

    // The friend calling from a nest at the end of a branch (or Kitty, perched where a clue might
    // be), which grows from a trunk on the right
    let scene_y = if first { 318.0 } else { 420.0 };
    let nest_at = vec2(300.0, scene_y);
    let sway = wave(t, 0.8, 4.0, 0.0);
    let trunk_x = 632.0;
    draw_trunk(trunk_x, -20.0, VIEW_H, 120.0, 1.0);
    // Sways most at the tip, not at all where it meets the trunk
    let branch_y = |x: f32| nest_at.y + 12.0 + (x - nest_at.x) * 0.1 + sway * (trunk_x - x) / (trunk_x - nest_at.x);
    draw_branch(nest_at.x - 50.0, trunk_x, branch_y, 0.0, 36.0);
    for (i, x) in [420.0, 500.0].into_iter().enumerate() {
        let wiggle = (t * 1.5 + i as f32).sin() * 0.07;
        assets.foliage.draw_ex(if i == 1 { "green_5" } else { "green_3" }, vec2(x, branch_y(x) - 14.0), 1.0, 1.5,
                               wiggle, WHITE);
    }
    let scale = if first { 1.3 } else { 1.7 };
    let call = (t * 1.4).fract();
    let jump = if call < 0.25 { (call / 0.25 * std::f32::consts::PI).sin() * 12.0 } else { 0.0 };
    let speaker = match card.speaker {
        Speaker::Friend(friend) => {
            let bounce = (t * 3.0).sin().abs() * 6.0;
            friend_in_nest(assets, &friend.help_frame(), nest_at + vec2(0.0, sway), scale, bounce, -1.0, 1.0);
            // "Help!" calls out beside the nest, a little jump with each call
            outlined("Help!", nest_at.x - 140.0, nest_at.y - 50.0 * scale - jump, 38.0, rgb(1.0, 0.92, 0.55), INK,
                     Align::Center);
            friend.full_name()
        }
        Speaker::Kitty => {
            assets.kitty.draw_ex("sit_alert_0", vec2(nest_at.x, branch_y(nest_at.x) - 2.0), 1.0, scale * 0.9, 0.0, WHITE);
            outlined("?", nest_at.x - 110.0, nest_at.y - 60.0 * scale - jump, 56.0, rgb(1.0, 0.92, 0.55), INK,
                     Align::Center);
            "Kitty"
        }
    };

    // What they say
    let say_in = appear(time, 0.3, 0.5);
    let bubble = speech_bubble(card.line, speaker, nest_at.y + 30.0 + (1.0 - say_in) * 30.0,
                               vec2(nest_at.x, nest_at.y + 14.0), if first { 23.0 } else { 26.0 }, say_in);

    // The danger to watch for, then on the first level the basics, or the clues so far
    let tip_in = appear(time, 0.5, 0.5);
    let y = bubble.bottom() + 22.0 + (1.0 - tip_in) * 30.0;
    let (sprites, frame) = card.watch_out.icon(assets);
    let heading = if card.is_new {
        format!("New danger: {}!", card.watch_out.name())
    } else {
        format!("Watch out: {}", card.watch_out.name())
    };
    let y = tip_card(y, &heading, card.watch_tip.unwrap_or(card.watch_out.tip()), tip_in,
                     |at| fit_sprite_centred(sprites, frame, at, 88.0, -1.0, fade(WHITE, tip_in)));
    let more_in = appear(time, BASICS_CARD_IN.0, BASICS_CARD_IN.1);
    if first {
        basics_card(assets, y + 16.0 + (1.0 - more_in) * 30.0, more_in, touch);
    } else if let Some(track) = card.track {
        clue_track(assets, &track, (y + 1094.0) / 2.0 + (1.0 - more_in) * 30.0, t, more_in);
    }

    if time > level_intro_wait(first).max(0.6) {
        tap_prompt("Tap to climb!", 1128.0, t);
    }
}

/// The clues found so far in a region and its friend at the end, in a little panel centred on
/// `centre_y`: found clues golden, this level's slot glowing.
pub(crate) fn clue_track(assets: &Assets, track: &ClueTrack, centre_y: f32, t: f32, alpha: f32) {
    let slot = 110.0;
    let r = Rect::new(300.0 - slot * 1.5 - 20.0, centre_y - 48.0, slot * 3.0 + 40.0, 100.0);
    panel(r, 26.0, alpha);
    let name = track.friend.name();
    let label = if track.stage >= 3 { format!("{name} is up there!") } else { format!("Clues to {name}: {} of 2", track.found) };
    shadowed(&label, 300.0, r.y - 14.0, 24.0, fade(CREAM, alpha), Align::Center);
    for i in 0..3 {
        let centre = vec2(r.x + 20.0 + slot * (i as f32 + 0.5), r.center().y);
        if i + 1 == track.stage {
            let glow = (t * 3.0).sin() * 0.5 + 0.5;
            draw_circle(centre.x, centre.y, 40.0, fade(GOLD_TEXT, (0.2 + glow * 0.2) * alpha));
            draw_circle_lines(centre.x, centre.y, 40.0, 3.0, fade(GOLD_TEXT, 0.85 * alpha));
        }
        if i < 2 && i < track.found {
            clue_icon(assets, track.objects[i], centre, 64.0, alpha);
        } else if i < 2 {
            draw_circle(centre.x, centre.y, 26.0, fade(INK, 0.5 * alpha));
            draw_circle(centre.x, centre.y, 23.0, fade(CREAM, 0.4 * alpha));
            text("?", centre.x, centre.y + 12.0, 34.0, fade(INK, alpha), Align::Center);
        } else {
            let tint = if track.stage >= 3 { fade(WHITE, alpha) } else { fade(Color::new(0.62, 0.66, 0.66, 0.6), alpha) };
            fit_sprite_centred(&assets.friends, &track.friend.help_frame(), centre, 64.0, 1.0, tint);
        }
    }
}

/// The pictures beside the level-one tips.
#[derive(Clone, Copy)]
enum TipIcon {
    Run,
    Pounce,
    Hang,
    Snap,
    Treats,
}

/// The level-one card: the controls (as gestures on a touch screen, keys otherwise) and a couple
/// of tips, each with a little picture. Bopping critters is on the squirrel card just above it.
fn basics_card(assets: &Assets, top: f32, alpha: f32, touch: bool) {
    let controls = if touch {
        [
            "Slide the bar at the bottom left to walk, and further to run.",
            "Hold the right side to crouch, let go to pounce. Hold longer to leap higher!",
            "Fall past a branch to grab it. Tap the right side to climb up; swipe down there to let go or drop through.",
        ]
    } else {
        [
            "Run with the arrow keys or A and D, or hold Shift as well to walk.",
            "Hold Space to crouch, let go to pounce. Hold longer to leap higher!",
            "Fall past a branch to grab it. Space climbs up; Down lets go or drops through.",
        ]
    };
    let rows = [
        (TipIcon::Run, controls[0]),
        (TipIcon::Pounce, controls[1]),
        (TipIcon::Hang, controls[2]),
        (TipIcon::Snap, "Thin pale branches hold you or a dog, but snap under both, or under a big bounce!"),
        (TipIcon::Treats, "Fish treats give points (golden ones give more), and hearts heal."),
    ];
    let layout = IconRows { icon_x: 76.0, text_x: 124.0, text_w: 440.0, size: 22.0, gap: 8.0 };
    let r = Rect::new(24.0, top, 552.0, 68.0 + layout.height(&rows));
    panel(r, 24.0, alpha);
    shadowed("How to play", r.x + 24.0, r.y + 38.0, 25.0, fade(LEAF_TEXT, alpha), Align::Left);
    let white = fade(WHITE, alpha);
    layout.draw(&rows, r.y + 56.0, alpha, |icon, at| match icon {
        TipIcon::Run => fit_sprite_centred(&assets.kitty, "run_1", at, 60.0, 1.0, white),
        TipIcon::Pounce => crouched_on_ring(assets, at, RING_FILL, alpha),
        TipIcon::Hang => fit_sprite_centred(&assets.kitty, "dangle_0", at, 58.0, 1.0, white),
        TipIcon::Snap => fit_sprite_centred(&assets.foliage, "twig_1", at, 54.0, 1.0, white),
        TipIcon::Treats => {
            assets.items.draw_ex("fish_1", at + vec2(-12.0, -10.0), 1.0, 0.65, -0.3, white);
            assets.items.draw_ex("gold_0", at + vec2(12.0, 2.0), 1.0, 0.65, 0.2, white);
            assets.items.draw_ex("heart_0", at + vec2(-10.0, 16.0), 1.0, 0.6, 0.0, white);
        }
    });
}

/// Rows of a little picture beside wrapped text, as on the how-to-play and pause cards: the
/// picture centred at `icon_x`, the text from `text_x` (both in playfield x).
#[derive(Clone, Copy)]
struct IconRows {
    icon_x: f32,
    text_x: f32,
    text_w: f32,
    size: f32,
    /// Space between rows.
    gap: f32,
}

impl IconRows {
    const LINE_SPACING: f32 = 1.3;
    /// Rows are at least this tall, so the pictures don't crowd each other.
    const MIN_HEIGHT: f32 = 54.0;

    fn row_height(self, line: &str) -> f32 {
        (wrap(line, self.size, self.text_w).len() as f32 * self.size * Self::LINE_SPACING).max(Self::MIN_HEIGHT)
    }

    /// The height of all the rows together.
    fn height<T>(self, rows: &[(T, &str)]) -> f32 {
        rows.iter().map(|(_, line)| self.row_height(line) + self.gap).sum::<f32>() - self.gap
    }

    /// Draw the rows from `top`; `icon` draws a row's picture centred on the point it's given.
    fn draw<T: Copy>(self, rows: &[(T, &str)], top: f32, alpha: f32, icon: impl Fn(T, Vec2)) {
        let mut y = top;
        for &(kind, line) in rows {
            let h = self.row_height(line);
            icon(kind, vec2(self.icon_x, y + h / 2.0));
            let lines = wrap(line, self.size, self.text_w);
            let block = lines.len() as f32 * self.size * Self::LINE_SPACING;
            let mut ty = y + (h - block) / 2.0 + self.size * 0.95;
            for l in &lines {
                text(l, self.text_x, ty, self.size, fade(CREAM, alpha), Align::Left);
                ty += self.size * Self::LINE_SPACING;
            }
            y += h + self.gap;
        }
    }
}

/// Kitty crouched to pounce, centred on `at`, with a full charge ring at her feet.
fn crouched_on_ring(assets: &Assets, at: Vec2, ring: Color, alpha: f32) {
    // As in play, the ring sits under her haunches, a little behind her middle
    charge_ring(at + vec2(-6.0, 12.0), 1.0, ring, alpha);
    fit_sprite_centred(&assets.kitty, "swish_a_2", at - vec2(0.0, 4.0), 70.0, 1.0, fade(WHITE, alpha));
}

/// A small copy of the charge ring at Kitty's feet, `filled` (0..1) of the way round from the
/// front either way, as it fills in play.
pub(crate) fn charge_ring(centre: Vec2, filled: f32, color: Color, alpha: f32) {
    const PIECES: usize = 32;
    let (rx, ry) = (26.0, 7.0);
    let point = |angle: f32| centre + vec2(rx * angle.cos(), ry * angle.sin());
    for k in 0..PIECES {
        let (a, b) = (k as f32 / PIECES as f32 * TAU, (k + 1) as f32 / PIECES as f32 * TAU);
        let round = (((a + b) / 2.0 - FRAC_PI_2 + PI).rem_euclid(TAU) - PI).abs() / PI;
        let piece = if round <= filled { color } else { RING_TRACK };
        let (p, q) = (point(a), point(b));
        draw_line(p.x, p.y, q.x, q.y, 3.0, fade(piece, alpha));
    }
}

/// Shown over the world while the rescued friend cheers.
pub fn draw_goal_banner(assets: &Assets, nest: NestGoal, found: Option<&str>, time: f32) {
    let t = ambient_time();
    let slide = ease_out_back(time / 0.5);
    let leave = appear(time, 2.7, 0.3);
    let y = -140.0 + slide * 400.0 - leave * 400.0;
    let r = Rect::new(60.0, y, 480.0, 120.0);
    // Sparkles spinning around the banner
    for i in 0..8 {
        let a = t * 1.5 + i as f32 * TAU / 8.0;
        let at = r.center() + vec2(a.cos() * 270.0, a.sin() * 82.0);
        let s = 0.7 + (t * 4.0 + i as f32).sin() * 0.25;
        assets.items.draw_ex("star_0", at, 1.0, s, a, WHITE);
    }
    drop_shadow(r, 30.0, 1.0);
    rounded_rect(r, 30.0, rgb(1.0, 0.86, 0.40), rgb(0.98, 0.62, 0.22));
    rounded_outline(r, 30.0, 4.0, fade(INK, 0.7));
    let icon_at = vec2(r.x + 66.0, r.bottom() - 16.0);
    let hop = (t * 6.0).sin().abs() * 8.0;
    let (small, line) = match nest {
        NestGoal::Rescue(friend) => {
            fit_sprite(&assets.friends, &friend.cheer_frame((t * 6.0) as usize), icon_at - vec2(0.0, hop), 90.0, 1.0, WHITE);
            ("Hooray!", format!("You found {}!", friend.name()))
        }
        NestGoal::Clue { object, .. } => {
            clue_icon(assets, object, icon_at - vec2(0.0, 40.0 + hop), 70.0, 1.0);
            ("A clue!", found.unwrap_or("A clue!").to_string())
        }
    };
    text(small, r.x + 130.0, r.y + 44.0, 26.0, fade(INK, 0.8), Align::Left);
    let size = fit_size(&line, 40.0, r.w - 150.0);
    outlined(&line, r.x + 130.0, r.y + 92.0, size, CREAM, INK, Align::Left);
}

/// A clue's object centred on `at`, `size` across: its painting once there is one, until then
/// a golden bubble with an exclamation mark.
pub(crate) fn clue_icon(assets: &Assets, object: ClueObject, at: Vec2, size: f32, alpha: f32) {
    let frame = format!("{}_0", object.sprite());
    if assets.clues.has(&frame) {
        fit_sprite_centred(&assets.clues, &frame, at, size, 1.0, fade(WHITE, alpha));
    } else {
        draw_circle(at.x, at.y, size * 0.42, fade(INK, 0.6 * alpha));
        draw_circle(at.x, at.y, size * 0.38, fade(GOLD_TEXT, alpha));
        outlined("!", at.x, at.y + size * 0.2, size * 0.55, fade(CREAM, alpha), fade(INK, alpha), Align::Center);
    }
}
/// What a level-clear card shows besides the tally.
pub struct ClearCard<'a> {
    pub title: &'a str,
    pub nest: NestGoal,
    /// A rescue: the friend's thanks. A clue: what Kitty found.
    pub line: &'a str,
    pub speaker: &'a str,
    /// A campaign level's stars: those this attempt earned, and which of them are new.
    pub stars: Option<(Stars, Stars)>,
    pub star_time: f32,
    pub new_best: bool,
    /// An endless run shows its score so far.
    pub run_score: bool,
}

/// When the level-clear card has finished counting up (a tap before then skips to it).
pub const CLEAR_COUNTED: f32 = TALLY_START + 6.0 * TALLY_ROW + 1.0;

pub fn draw_level_clear(assets: &Assets, card: &ClearCard, tally: &Tally, time: f32) {
    let t = ambient_time();
    backdrop(assets, Mood::Party);
    confetti(Rect::new(0.0, 0.0, VIEW_W, VIEW_H), t, 60, 1.0);

    let pop = ease_out_back(time / 0.6);
    let heading = if matches!(card.nest, NestGoal::Clue { .. }) { "Clue Found!" } else { "Level Clear!" };
    outlined(heading, 300.0, 96.0 - (1.0 - pop) * 160.0, fit_size(heading, 68.0, 560.0), GOLD_TEXT, INK, Align::Center);

    // The friend cheering in the nest (or the clue, caught on the branch), sparkles about; low
    // enough that hopping ears clear the level's name
    let nest_at = vec2(300.0, 350.0);
    for i in 0..5 {
        let a = t * 0.8 + i as f32 * TAU / 5.0;
        let at = nest_at + vec2(a.cos() * 130.0, -60.0 + a.sin() * 50.0);
        assets.items.draw_ex("star_0", at, 1.0, 0.6 + (t * 3.0 + i as f32).sin() * 0.2, a, WHITE);
    }
    draw_trunk(-30.0, -20.0, VIEW_H, 120.0, 1.0);
    draw_branch(0.0, nest_at.x + 60.0, |x| nest_at.y + 10.0 + (nest_at.x - x) * 0.08, 0.0, 34.0);
    for (i, x) in [90.0, 170.0].into_iter().enumerate() {
        let wiggle = (t * 1.5 + i as f32).sin() * 0.07;
        assets.foliage.draw_ex("green_5", vec2(x, nest_at.y - 6.0 + (nest_at.x - x) * 0.08), 1.0, 1.4, wiggle, WHITE);
    }
    let hop = (t * 5.0).sin().abs() * 20.0;
    match card.nest {
        NestGoal::Rescue(friend) => friend_in_nest(assets, &friend.cheer_frame((t * 6.0) as usize), nest_at, 1.4, hop, 1.0, 1.0),
        NestGoal::Clue { object, .. } => clue_icon(assets, object, nest_at - vec2(0.0, 30.0 + hop * 0.5), 96.0, 1.0),
    }
    // After the friend, so tall ears hopping up can't cover it
    shadowed(card.title, 300.0, 142.0, 26.0, CREAM, Align::Center);

    let say_in = appear(time, 0.3, 0.4);
    let bubble = speech_bubble(card.line, card.speaker, 382.0 + (1.0 - say_in) * 20.0,
                               vec2(nest_at.x, nest_at.y + 6.0), 24.0, say_in);

    // The tally, counting up one row at a time
    let other = tally.level_total.saturating_sub(tally.rescue + tally.time_bonus + tally.no_damage + tally.collected);
    let goal = if matches!(card.nest, NestGoal::Clue { .. }) { "Clue found" } else { "Rescue" };
    let rows: [(&str, u32); 5] = [
        (goal, tally.rescue),
        ("Time bonus", tally.time_bonus),
        ("No-bump bonus", tally.no_damage),
        ("Treats collected", tally.collected),
        ("Climbing & bops", other),
    ];
    let card_in = appear(time, 0.4, 0.4);
    let h = if card.run_score { 340.0 } else { 290.0 };
    let r = Rect::new(40.0, bubble.bottom() + 18.0 + (1.0 - card_in) * 30.0, 520.0, h);
    panel(r, 26.0, card_in);
    let mut y = r.y + 44.0;
    for (i, &(label, value)) in rows.iter().enumerate() {
        let start = TALLY_START + i as f32 * TALLY_ROW;
        let shown = appear(time, start, 0.2);
        let counted = (value as f32 * ((time - start) / TALLY_COUNT).clamp(0.0, 1.0)).round() as u32;
        let dim = if value == 0 { 0.5 } else { 1.0 };
        text(label, r.x + 30.0, y, 24.0, fade(CREAM, shown * dim), Align::Left);
        let number = if value == 0 { "--".to_string() } else { format!("+{}", thousands(counted)) };
        text(&number, r.right() - 30.0, y, 24.0, fade(LEAF_TEXT, shown * dim), Align::Right);
        y += 38.0;
    }
    let after_rows = TALLY_START + rows.len() as f32 * TALLY_ROW;
    let line_in = appear(time, after_rows - 0.1, 0.3);
    draw_line(r.x + 30.0, y - 16.0, r.x + 30.0 + (r.w - 60.0) * line_in, y - 16.0, 3.0, fade(CREAM, 0.35 * card_in));
    y += 18.0;
    let total_in = appear(time, after_rows, 0.2);
    let total = (tally.level_total as f32 * ((time - after_rows) / TALLY_COUNT).clamp(0.0, 1.0)).round() as u32;
    outlined("Level total", r.x + 30.0, y, 30.0, fade(GOLD_TEXT, total_in), fade(INK, total_in), Align::Left);
    outlined(&thousands(total), r.right() - 30.0, y, 30.0, fade(GOLD_TEXT, total_in), fade(INK, total_in), Align::Right);
    let later = after_rows + TALLY_ROW + 0.1;
    if card.run_score {
        y += 50.0;
        let score_in = appear(time, later, 0.2);
        let before = tally.score.saturating_sub(tally.level_total) as f32;
        let score = (before + tally.level_total as f32 * ((time - later) / 0.8).clamp(0.0, 1.0)).round() as u32;
        shadowed("Run score", r.x + 30.0, y, 28.0, fade(CREAM, score_in), Align::Left);
        shadowed(&thousands(score), r.right() - 30.0, y, 28.0, fade(CREAM, score_in), Align::Right);
    }

    // The stars, popping in one by one, new ones with a burst
    let mut below = r.bottom();
    if let Some((stars, fresh)) = card.stars {
        let won = [stars.top, stars.no_bumps, stars.quick];
        let new = [fresh.top, fresh.no_bumps, fresh.quick];
        let labels = ["Reached the top", "No bumps", &format!("Under {}", mmss(card.star_time))];
        for k in 0..3 {
            let start = later + k as f32 * 0.25;
            let pop = ease_out_back(((time - start) / 0.35).clamp(0.0, 1.0));
            let at = vec2(130.0 + 170.0 * k as f32, r.bottom() + 70.0);
            if new[k] && pop > 0.0 {
                for i in 0..6 {
                    let a = i as f32 * TAU / 6.0 + t;
                    assets.items.draw_ex("star_0", at + vec2(a.cos(), a.sin()) * 44.0 * pop, 1.0, 0.45 * pop, a, fade(WHITE, pop));
                }
            }
            let fill = if won[k] { GOLD_TEXT } else { fade(CREAM, 0.25) };
            crate::ui::star(at, 30.0 * pop.max(0.01), fade(fill, pop.min(1.0)), fade(INK, 0.8 * pop.min(1.0)));
            text(labels[k], at.x, at.y + 56.0, 19.0, fade(CREAM, pop.min(1.0)), Align::Center);
        }
        below = r.bottom() + 150.0;
    }
    if card.new_best {
        let burst = appear(time, later + 0.9, 0.4);
        ribbon("New best!", vec2(300.0, below + 46.0), 30.0, burst, Ribbon::Gold);
    }
}

/// Seconds as m:ss.
pub(crate) fn mmss(seconds: f32) -> String {
    let s = seconds.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// After a clue level: the friend's trail, and the region's helper saying where they went.
pub fn draw_clue_card(assets: &Assets, region: &Region, title: &str, clue: &Clue, track: ClueTrack, time: f32) {
    let t = ambient_time();
    backdrop(assets, Mood::Day);
    let pop = ease_out_back(time / 0.6);
    outlined("A Clue!", 300.0, 100.0 - (1.0 - pop) * 160.0, 68.0, GOLD_TEXT, INK, Align::Center);
    shadowed(title, 300.0, 146.0, 26.0, CREAM, Align::Center);

    // A picture of the friend tumbling away on the wind, the clue caught in the corner
    let pic_in = appear(time, 0.15, 0.4);
    let r = Rect::new(60.0, 186.0 + (1.0 - pic_in) * 30.0, 480.0, 290.0);
    picture_frame(r, Mood::Day, pic_in);
    clip(Some(r));
    for i in 0..4 {
        let x = r.x + ((t * 140.0 + i as f32 * 150.0) % (r.w + 200.0)) - 100.0;
        gust(vec2(x, r.y + 70.0 + i as f32 * 50.0), 120.0, fade(WHITE, pic_in * 0.4), r);
    }
    let drift = (t * 60.0) % (r.w + 160.0) - 80.0;
    let tumble = vec2(r.x + drift, r.y + 120.0 + (t * 2.0).sin() * 24.0);
    fit_sprite_centred(&assets.friends, &track.friend.storm_frame((t * 3.0) as usize), tumble, 100.0, 1.0, fade(WHITE, pic_in));
    clip(None);
    clue_icon(assets, clue.object, vec2(r.right() - 64.0, r.bottom() - 58.0), 84.0, pic_in);

    // The helper, perched on the frame's corner once there's a picture of them, says what they saw
    let helper = &region.helper;
    let perch = vec2(r.x + 70.0, r.bottom() + 4.0);
    let talking = ((t * 4.0) as usize).is_multiple_of(2);
    let frame = format!("{}_{}_0", helper.sprite, if talking { "talk" } else { "idle" });
    let idle = format!("{}_idle_0", helper.sprite);
    if assets.helpers.has(&frame) {
        assets.helpers.draw_ex(&frame, perch, 1.0, 1.0, 0.0, fade(WHITE, pic_in));
    } else if assets.helpers.has(&idle) {
        assets.helpers.draw_ex(&idle, perch, 1.0, 1.0, 0.0, fade(WHITE, pic_in));
    }
    let say_in = appear(time, 0.4, 0.4);
    let bubble = speech_bubble(clue.helper_line, helper.full_name, r.bottom() + 36.0 + (1.0 - say_in) * 20.0,
                               perch, 25.0, say_in);

    let track_in = appear(time, 0.7, 0.4);
    clue_track(assets, &track, (bubble.bottom() + 1094.0) / 2.0 + (1.0 - track_in) * 30.0, t, track_in);
    if time > 0.9 {
        tap_prompt("Tap to continue", 1128.0, t);
    }
}

/// Out of hearts: a card over the level, with room for Retry and Map below.
pub fn draw_try_again(assets: &Assets, time: f32) {
    let t = ambient_time();
    let dim = appear(time, 0.0, 0.3);
    dim_screen(Color::new(0.03, 0.05, 0.08, 0.6 * dim));
    let drop = ease_out_back(time / 0.5);
    let r = Rect::new(60.0, 330.0 - (1.0 - drop) * 120.0, 480.0, 360.0);
    panel(r, 30.0, dim);
    outlined("Out of hearts!", 300.0, r.y + 80.0, fit_size("Out of hearts!", 54.0, 440.0), fade(rgb(1.0, 0.62, 0.55), dim),
             fade(INK, dim), Align::Center);
    let nap = vec2(300.0, r.y + 230.0);
    shadow_ellipse(nap + vec2(0.0, 4.0), 90.0, 14.0, 0.35 * dim);
    let frame = if (t / 1.2) as i32 % 2 == 0 { "sleep_0" } else { "sleep_1" };
    assets.kitty.draw_ex(frame, nap - vec2(0.0, 12.0), 1.0, 1.6, 0.0, fade(WHITE, dim));
    shadowed("Shake it off, Kitty! Have another go?", 300.0, r.bottom() - 40.0, 24.0, fade(CREAM, dim), Align::Center);
}
/// `previous_best` is the high score from before this game, so beating it shows the ribbon.
/// An endless run over: how far Kitty climbed and what she scored, with room for Retry and Map.
pub fn draw_endless_over(assets: &Assets, score: u32, best: u32, new_best: bool, climb: u32, time: f32) {
    let t = ambient_time();
    backdrop(assets, Mood::Night);

    let drop = ease_out_back(time / 0.7);
    outlined("What a Climb!", 300.0, 170.0 - (1.0 - drop) * 200.0, fit_size("What a Climb!", 80.0, 560.0),
             GOLD_TEXT, INK, Align::Center);

    // Kitty curled up for a nap on a pile of leaves
    let nap = Vec2::new(300.0, 470.0);
    shadow_ellipse(nap + vec2(0.0, 4.0), 120.0, 18.0, 0.35);
    for (i, dx) in [-90.0, -40.0, 20.0, 80.0].into_iter().enumerate() {
        let leaf = if i % 2 == 0 { "autumn_4" } else { "autumn_5" };
        assets.foliage.draw_ex(leaf, nap + vec2(dx, -6.0), 1.0, 1.5, 0.0, WHITE);
    }
    let frame = if (t / 1.2) as i32 % 2 == 0 { "sleep_0" } else { "sleep_1" };
    assets.kitty.draw_ex(frame, nap - vec2(0.0, 18.0), 1.0, 2.0, 0.0, WHITE);
    for i in 0..3 {
        let phase = (t * 0.6 + i as f32 / 3.0) % 1.0;
        let at = nap + vec2(90.0 + phase * 60.0 + (phase * 9.0).sin() * 8.0, -160.0 - phase * 120.0);
        let alpha = (phase * 3.0).min(1.0) * (1.0 - phase);
        shadowed("z", at.x, at.y, 30.0 + i as f32 * 8.0, fade(CREAM, alpha), Align::Center);
    }
    shadowed(&format!("Kitty reached climb {climb}!"), 300.0, 560.0, 26.0, CREAM, Align::Center);

    // Score card
    let card_in = appear(time, 0.4, 0.5);
    let r = Rect::new(70.0, 610.0 + (1.0 - card_in) * 30.0, 460.0, 250.0);
    panel(r, 28.0, card_in);
    shadowed("Score", 300.0, r.y + 56.0, 28.0, fade(CREAM, card_in), Align::Center);
    outlined(&thousands(score), 300.0, r.y + 130.0, 64.0, fade(GOLD_TEXT, card_in), fade(INK, card_in),
             Align::Center);
    let best = format!("Best  {}", thousands(best.max(score)));
    let bw = measure(&best, 26.0);
    assets.items.draw_ex("gold_0", vec2(300.0 - bw / 2.0 - 30.0, r.y + 200.0), 1.0, 0.7, 0.0, fade(WHITE, card_in));
    shadowed(&best, 300.0 + 10.0, r.y + 210.0, 26.0, fade(CREAM, card_in), Align::Center);

    if new_best {
        // A golden ribbon with stars bursting from it
        let burst = appear(time, 1.0, 0.5);
        let ribbon_at = vec2(300.0, r.bottom() + 66.0);
        for i in 0..10 {
            let a = i as f32 * TAU / 10.0 + t * 0.6;
            let reach = 150.0 + (t * 3.0 + i as f32).sin() * 20.0;
            let at = ribbon_at + vec2(a.cos() * reach, a.sin() * reach * 0.28) * burst;
            assets.items.draw_ex("star_0", at, 1.0, 0.7 * burst, a * 2.0, fade(WHITE, burst));
        }
        ribbon("New best!", ribbon_at, 34.0, burst, Ribbon::Gold);
    }
}

/// The pictures beside the pause card's reminders: gestures on a touch screen, keys otherwise.
#[derive(Clone, Copy)]
enum Legend {
    Slide,
    Hold,
    Swipe,
    ArrowKeys,
    SpaceKey,
    DownKey,
    Bounce,
}

/// Paused, with a reminder of the controls (gestures if `touch`, keys otherwise) and the bounce.
pub fn draw_paused(assets: &Assets, time: f32, touch: bool) {
    let t = ambient_time();
    let dim = appear(time, 0.0, 0.2);
    dim_screen(Color::new(0.03, 0.05, 0.08, 0.6 * dim));
    let bounce = "Hold as you land, let go as the ring glows blue: bounce higher! Too high and branches snap.";
    let rows = if touch {
        [
            (Legend::Slide, "Slide the bar at the bottom left to walk; further to run."),
            (Legend::Hold, "Hold the right side to crouch, let go to leap. Hanging? Tap to climb."),
            (Legend::Swipe, "Swipe down on the right side to let go of a branch or drop through it."),
            (Legend::Bounce, bounce),
        ]
    } else {
        [
            (Legend::ArrowKeys, "Run, or A and D. Shift walks."),
            (Legend::SpaceKey, "Hold to crouch, let go to leap or climb up. Up and W work too."),
            (Legend::DownKey, "Let go of a branch or drop through it. S works too."),
            (Legend::Bounce, bounce),
        ]
    };
    let layout = IconRows { icon_x: 112.0, text_x: 180.0, text_w: 352.0, size: 22.0, gap: 10.0 };
    // The title above the rows; the buttons go below the card
    let h = 124.0 + layout.height(&rows) + 30.0;
    let r = Rect::new(40.0, 830.0 - h, 520.0, h);
    panel(r, 30.0, dim);
    outlined("Paused", 300.0, r.y + 86.0, 64.0, fade(CREAM, dim), fade(INK, dim), Align::Center);
    layout.draw(&rows, r.y + 124.0, dim, |kind, at| legend_icon(assets, kind, at, t, dim));
}

/// A little moving picture of a gesture or the keys for one of the pause card's rows.
fn legend_icon(assets: &Assets, kind: Legend, at: Vec2, t: f32, alpha: f32) {
    let line = fade(CREAM, 0.75 * alpha);
    let soft = fade(WHITE, 0.25 * alpha);
    let fingertip = |p: Vec2, a: f32| {
        draw_circle(p.x, p.y, 12.0, fade(CREAM, 0.85 * a * alpha));
        draw_circle_lines(p.x, p.y, 12.0, 2.0, fade(INK, 0.4 * a * alpha));
    };
    match kind {
        Legend::Slide => {
            // The slider's track and its knob, sliding to and fro, with arrows either end
            draw_rectangle(at.x - 38.0, at.y - 4.0, 76.0, 8.0, soft);
            for side in [-1.0, 1.0] {
                let tip = at + vec2(side * 52.0, 0.0);
                draw_triangle(tip, tip + vec2(-side * 10.0, -8.0), tip + vec2(-side * 10.0, 8.0), line);
            }
            fingertip(at + vec2((t * 2.0).sin() * 28.0, 0.0), 1.0);
        }
        Legend::Hold => {
            // A finger held down while the charge ring fills, over and over
            let filled = (t * 0.8).fract();
            charge_ring(at + vec2(0.0, 14.0), filled, if filled > 0.9 { RING_FULL } else { RING_FILL }, alpha);
            fingertip(at + vec2(0.0, -4.0), 1.0);
        }
        Legend::Swipe => {
            // A finger flicking down past a chevron
            let k = (t * 1.2).fract();
            let p = at + vec2(0.0, -20.0 + 30.0 * k);
            draw_line(at.x, at.y - 20.0, p.x, p.y, 6.0, fade(WHITE, 0.2 * alpha));
            fingertip(p, 1.0 - k * k);
            for side in [-1.0, 1.0] {
                draw_line(at.x + side * 14.0, at.y + 12.0, at.x, at.y + 24.0, 4.0, line);
            }
        }
        Legend::ArrowKeys => {
            keycap(Rect::new(at.x - 48.0, at.y - 20.0, 44.0, 40.0), alpha, |c| arrow(c, vec2(-1.0, 0.0), line));
            keycap(Rect::new(at.x + 4.0, at.y - 20.0, 44.0, 40.0), alpha, |c| arrow(c, vec2(1.0, 0.0), line));
        }
        Legend::SpaceKey => keycap(Rect::new(at.x - 52.0, at.y - 20.0, 104.0, 40.0), alpha, |c| {
            text("Space", c.x, c.y + 7.0, 20.0, line, Align::Center);
        }),
        Legend::DownKey => keycap(Rect::new(at.x - 22.0, at.y - 20.0, 44.0, 40.0), alpha, |c| arrow(c, vec2(0.0, 1.0), line)),
        // Bobbing as if on a springy branch
        Legend::Bounce => crouched_on_ring(assets, at - vec2(0.0, (t * 4.0).sin().abs() * 6.0), RING_SWEET, alpha),
    }
}

/// A rounded key with whatever `label` draws at its centre.
fn keycap(r: Rect, alpha: f32, label: impl Fn(Vec2)) {
    rounded_rect(r, 8.0, fade(rgb(0.95, 0.92, 0.84), 0.28 * alpha), fade(rgb(0.70, 0.66, 0.60), 0.28 * alpha));
    rounded_outline(r, 8.0, 2.0, fade(CREAM, 0.6 * alpha));
    label(r.center());
}

/// A small solid arrowhead pointing along `dir` (a unit vector), centred at `at`.
fn arrow(at: Vec2, dir: Vec2, color: Color) {
    let side = vec2(-dir.y, dir.x) * 9.0;
    draw_triangle(at + dir * 9.0, at - dir * 6.0 + side, at - dir * 6.0 - side, color);
}

// ---------------------------------------------------------------------------------------------
// Scenery shared by the screens

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mood {
    Dusk,
    Night,
    Day,
    Party,
}

/// The painted forest, tinted for the mood and darkened top and bottom so cards and text stand
/// out, with leaves drifting down and fireflies or stars after dark.
pub(crate) fn backdrop(assets: &Assets, mood: Mood) {
    let t = ambient_time();
    let tex = &assets.background;
    let scale = (VIEW_W / tex.width()).max(VIEW_H / tex.height());
    let size = vec2(tex.width(), tex.height()) * scale;
    let slack = size.x - VIEW_W;
    let x = -slack / 2.0 + (t * 0.05).sin() * slack * 0.45;
    let (tint, top, bottom) = match mood {
        Mood::Dusk => (rgb(0.70, 0.62, 0.78), Color::new(0.14, 0.08, 0.28, 0.85), Color::new(0.05, 0.05, 0.10, 0.75)),
        Mood::Night => (rgb(0.30, 0.36, 0.58), Color::new(0.03, 0.04, 0.14, 0.9), Color::new(0.02, 0.03, 0.08, 0.8)),
        Mood::Day => (rgb(0.80, 0.86, 0.84), Color::new(0.10, 0.20, 0.30, 0.6), Color::new(0.04, 0.08, 0.06, 0.75)),
        Mood::Party => (rgb(0.78, 0.66, 0.72), Color::new(0.26, 0.10, 0.30, 0.75), Color::new(0.08, 0.04, 0.10, 0.75)),
    };
    draw_texture_ex(tex, x, VIEW_H - size.y, tint, DrawTextureParams { dest_size: Some(size), ..Default::default() });
    let clear = Color::new(top.r, top.g, top.b, 0.0);
    gradient_rect(Rect::new(0.0, 0.0, VIEW_W, VIEW_H * 0.45), top, clear);
    let clear = Color::new(bottom.r, bottom.g, bottom.b, 0.0);
    gradient_rect(Rect::new(0.0, VIEW_H * 0.5, VIEW_W, VIEW_H * 0.5), clear, bottom);

    if matches!(mood, Mood::Night | Mood::Dusk | Mood::Party) {
        let count = if mood == Mood::Night { 50 } else { 24 };
        for i in 0..count {
            let at = vec2(hash(i) * VIEW_W, hash(i + 500) * VIEW_H * 0.4);
            let twinkle = ((t * (1.0 + hash(i + 900) * 2.0) + hash(i + 77) * TAU).sin() * 0.5 + 0.5).powi(2);
            let r = 1.2 + hash(i + 300) * 1.8;
            draw_circle(at.x, at.y, r, Color::new(1.0, 0.97, 0.85, 0.3 + twinkle * 0.6));
        }
    }
    if matches!(mood, Mood::Dusk | Mood::Night) {
        fireflies(t, 14);
    }
    let (count, wind) = match mood {
        Mood::Night => (18, 2.2),
        _ => (12, 0.4),
    };
    leaves_in(assets, Rect::new(0.0, 0.0, VIEW_W, VIEW_H), t, count, wind, 1.0);
}

/// Leaves drifting down through `area`, blown sideways by `wind`.
pub(crate) fn leaves_in(assets: &Assets, area: Rect, t: f32, count: usize, wind: f32, alpha: f32) {
    const FALLING: [&str; 6] = ["falling_0", "falling_1", "falling_2", "falling_3", "falling_4", "falling_5"];
    let span = vec2(area.w + 80.0, area.h + 80.0);
    for i in 0..count {
        let speed = 35.0 + hash(i + 40) * 45.0;
        let y = (t * speed + hash(i) * span.y) % span.y - 40.0;
        let x = (hash(i + 20) * span.x + t * wind * 40.0 + (t * 1.3 + hash(i + 60) * TAU).sin() * 30.0)
            .rem_euclid(span.x) - 40.0;
        let at = area.point() + vec2(x, y);
        let fade_edge = ((y + 40.0) / 60.0).min((span.y - 40.0 - y) / 60.0).clamp(0.0, 1.0);
        let spin = t * (hash(i + 80) - 0.5) * 4.0 + hash(i + 90) * TAU;
        assets.foliage.draw_ex(FALLING[i % FALLING.len()], at, 1.0, 1.4 + hash(i + 10) * 0.8, spin,
                               Color::new(1.0, 1.0, 1.0, alpha * fade_edge * 0.95));
    }
}

fn fireflies(t: f32, count: usize) {
    fireflies_in(Rect::new(0.0, VIEW_H * 0.35, VIEW_W, VIEW_H * 0.5), t, count, 1.0);
}

pub(crate) fn fireflies_in(area: Rect, t: f32, count: usize, alpha: f32) {
    for i in 0..count {
        let base = area.point() + vec2(hash(i + 1000) * area.w, hash(i + 1100) * area.h);
        let at = base + vec2((t * 0.4 + hash(i + 1200) * TAU).sin() * 40.0, (t * 0.3 + hash(i + 1300) * TAU).cos() * 30.0);
        let glow = ((t * 1.5 + hash(i + 1400) * TAU).sin() * 0.5 + 0.5).powi(3);
        draw_circle(at.x, at.y, 9.0, Color::new(1.0, 0.95, 0.5, 0.12 * glow * alpha));
        draw_circle(at.x, at.y, 3.0, Color::new(1.0, 0.98, 0.7, 0.85 * glow * alpha));
    }
}

pub(crate) fn confetti(area: Rect, t: f32, count: usize, alpha: f32) {
    const COLORS: [Color; 5] = [
        Color::new(1.0, 0.82, 0.32, 1.0),
        Color::new(1.0, 0.55, 0.45, 1.0),
        Color::new(0.55, 0.85, 0.45, 1.0),
        Color::new(0.50, 0.75, 1.0, 1.0),
        Color::new(0.95, 0.60, 0.95, 1.0),
    ];
    for i in 0..count {
        let speed = 60.0 + hash(i + 2000) * 70.0;
        let y = (t * speed + hash(i + 2100) * area.h) % (area.h + 20.0) - 10.0;
        let x = hash(i + 2200) * area.w + (t * 2.0 + hash(i + 2300) * TAU).sin() * 16.0;
        let flip = (t * (3.0 + hash(i + 2400) * 4.0)).sin();
        let c = COLORS[i % COLORS.len()];
        draw_rectangle_ex(area.x + x, area.y + y, 10.0, 6.0 * flip.abs() + 1.0, DrawRectangleParams {
            offset: vec2(0.5, 0.5),
            rotation: t * (hash(i + 2500) - 0.5) * 6.0,
            color: fade(c, alpha * 0.9),
        });
    }
}

/// A swoosh of wind: a thin curved streak.
fn gust(at: Vec2, len: f32, color: Color, clip: Rect) {
    let steps = 10;
    for i in 0..steps {
        let a = i as f32 / steps as f32;
        let b = (i + 1) as f32 / steps as f32;
        let p = |f: f32| at + vec2(f * len, (f * 3.5).sin() * 10.0);
        let (pa, pb) = (p(a), p(b));
        if clip.contains(pa) && clip.contains(pb) {
            let taper = (a * std::f32::consts::PI).sin();
            draw_line(pa.x, pa.y, pb.x, pb.y, 1.0 + taper * 3.0, Color::new(color.r, color.g, color.b, color.a * taper));
        }
    }
}

/// A chunky branch along `y_at(x)` from `x0` to `x1`, lit on top and shaded underneath.
fn draw_branch(x0: f32, x1: f32, y_at: impl Fn(f32) -> f32, lift: f32, thickness: f32) {
    let steps = ((x1 - x0) / 24.0).ceil().max(1.0) as usize;
    let point = |i: usize| {
        let x = x0 + (x1 - x0) * i as f32 / steps as f32;
        vec2(x, y_at(x) + lift)
    };
    for (color, dy, w) in [(BARK_DARK, thickness * 0.18, thickness), (BARK, 0.0, thickness * 0.8),
                           (BARK_LIGHT, -thickness * 0.22, thickness * 0.22)] {
        for i in 0..steps {
            let (a, b) = (point(i), point(i + 1));
            draw_line(a.x, a.y + dy, b.x, b.y + dy, w, color);
            draw_circle(b.x, b.y + dy, w / 2.0, color);
        }
        let a = point(0);
        draw_circle(a.x, a.y + dy, w / 2.0, color);
    }
}

/// A tree trunk centred on `x`, shaded round, with a few furrows in the bark.
pub(crate) fn draw_trunk(x: f32, top: f32, bottom: f32, width: f32, alpha: f32) {
    let left = x - width / 2.0;
    let h = bottom - top;
    draw_rectangle(left, top, width, h, fade(TRUNK_COLOR, alpha));
    draw_rectangle(left + width * 0.18, top, width * 0.14, h, fade(BARK_LIGHT, alpha * 0.45));
    draw_rectangle(left + width * 0.78, top, width * 0.22, h, fade(BARK_DARK, alpha * 0.6));
    for i in 0..((h / 40.0) as usize) {
        let fx = left + width * (0.25 + hash(i + 700) * 0.5);
        let fy = top + hash(i + 800) * h;
        let len = 30.0 + hash(i + 900) * 60.0;
        draw_line(fx, fy, fx, (fy + len).min(bottom), 3.0, fade(BARK_DARK, alpha * 0.7));
    }
}

/// A friend sitting in a nest whose bottom centre is `at`: the nest, the friend, then the
/// nest's front rim again over the friend's feet.
pub(crate) fn friend_in_nest(assets: &Assets, frame: &str, at: Vec2, scale: f32, hop: f32, facing: f32, alpha: f32) {
    if scale <= 0.01 {
        return;
    }
    let white = fade(WHITE, alpha);
    assets.items.draw_ex("nest_1", at, 1.0, scale, 0.0, white);
    assets.friends.draw_ex(frame, at - vec2(0.0, 26.0 * scale + hop), facing, scale, 0.0, white);
    let nest = assets.items.get("nest_0");
    let size = nest.size();
    let cut = size.y * 0.42;
    let top_left = at - nest.anchor * scale + vec2(0.0, cut * scale);
    draw_texture_ex(&nest.texture, top_left.x, top_left.y, white, DrawTextureParams {
        dest_size: Some(vec2(size.x, size.y - cut) * scale),
        source: Some(Rect::new(0.0, cut, size.x, size.y - cut)),
        ..Default::default()
    });
}

/// A cream speech bubble with the speaker's name tag, its tail pointing up at `tail_to`.
/// Returns the bubble's rectangle.
pub(crate) fn speech_bubble(line: &str, speaker: &str, top: f32, tail_to: Vec2, size: f32, alpha: f32) -> Rect {
    let width = 520.0;
    let lines = wrap(&format!("\"{line}\""), size, width - 50.0);
    let line_h = size * 1.35;
    let r = Rect::new(300.0 - width / 2.0, top + 16.0, width, 40.0 + lines.len() as f32 * line_h);
    let paper = fade(rgb(1.0, 0.97, 0.90), alpha);
    drop_shadow(r, 22.0, alpha);
    let tail_x = tail_to.x.clamp(r.x + 40.0, r.right() - 40.0);
    draw_triangle(vec2(tail_x - 18.0, r.y + 4.0), vec2(tail_x + 18.0, r.y + 4.0), vec2(tail_to.x, tail_to.y.max(r.y - 50.0)),
                  paper);
    rounded_rect(r, 22.0, paper, paper);
    let mut y = r.y + 26.0 + size * 0.9;
    for l in &lines {
        text(l, 300.0, y, size, fade(rgb(0.25, 0.16, 0.10), alpha), Align::Center);
        y += line_h;
    }
    // Name tag on the top-left corner
    let tag_w = measure(speaker, 20.0) + 28.0;
    let tag = Rect::new(r.x + 18.0, r.y - 16.0, tag_w, 32.0);
    rounded_rect(tag, 16.0, fade(rgb(0.55, 0.80, 0.40), alpha), fade(rgb(0.38, 0.62, 0.28), alpha));
    text(speaker, tag.x + 14.0, tag.y + 23.0, 20.0, fade(INK, alpha), Align::Left);
    r
}

/// A card with a picture on the left and a heading and wrapped text beside it. Returns its
/// bottom edge.
fn tip_card(top: f32, heading: &str, body: &str, alpha: f32, icon: impl Fn(Vec2)) -> f32 {
    let size = 22.0;
    let text_w = 420.0;
    let lines = wrap(body, size, text_w);
    let h = (60.0 + lines.len() as f32 * size * 1.3 + 18.0).max(130.0);
    let r = Rect::new(24.0, top, 552.0, h);
    panel(r, 24.0, alpha);
    icon(vec2(r.x + 62.0, r.y + h / 2.0));
    shadowed(heading, r.x + 118.0, r.y + 40.0, 26.0, fade(ORANGE_TEXT, alpha), Align::Left);
    let mut y = r.y + 72.0;
    for l in &lines {
        text(l, r.x + 118.0, y, size, fade(CREAM, alpha), Align::Left);
        y += size * 1.3;
    }
    r.bottom()
}

fn page_dots(current: usize, count: usize, y: f32, t: f32) {
    let gap = 30.0;
    let x0 = 300.0 - (count as f32 - 1.0) * gap / 2.0;
    for i in 0..count {
        let x = x0 + i as f32 * gap;
        if i == current {
            let r = Rect::new(x - 16.0, y - 7.0, 32.0, 14.0);
            rounded_rect(r, 7.0, GOLD_TEXT, ORANGE_TEXT);
            let glow = (t * 3.0).sin() * 0.5 + 0.5;
            rounded_outline(grow(r, 3.0), 10.0, 2.0, fade(GOLD_TEXT, 0.3 * glow));
        } else {
            draw_circle(x, y, 6.0, fade(CREAM, if i < current { 0.7 } else { 0.3 }));
        }
    }
}

/// A gently pulsing "Tap to..." pill.
pub(crate) fn tap_prompt(label: &str, y: f32, t: f32) {
    let pulse = (t * 3.0).sin() * 0.5 + 0.5;
    let w = measure(label, 30.0) + 70.0;
    let r = Rect::new(300.0 - w / 2.0, y - 34.0, w, 60.0);
    rounded_rect(grow(r, 4.0 + pulse * 4.0), 34.0, fade(GOLD_TEXT, 0.18 * (1.0 - pulse)), fade(GOLD_TEXT, 0.18 * (1.0 - pulse)));
    drop_shadow(r, 30.0, 0.8);
    rounded_rect(r, 30.0, rgb(1.0, 0.72, 0.30), rgb(0.93, 0.50, 0.16));
    rounded_outline(r, 30.0, 3.0, fade(INK, 0.55));
    text(label, 300.0, r.y + 41.0, 30.0, fade(INK, 0.75 + pulse * 0.25), Align::Center);
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Ribbon {
    Leaf,
    Gold,
}

/// A ribbon banner with a line of text on it.
pub(crate) fn ribbon(label: &str, centre: Vec2, size: f32, alpha: f32, style: Ribbon) {
    if alpha <= 0.0 {
        return;
    }
    let (tail, top, bottom, ink) = match style {
        Ribbon::Leaf => (rgb(0.22, 0.45, 0.20), rgb(0.50, 0.78, 0.36), rgb(0.32, 0.58, 0.24), CREAM),
        Ribbon::Gold => (rgb(0.78, 0.42, 0.12), rgb(1.0, 0.86, 0.40), rgb(0.96, 0.62, 0.20), INK),
    };
    let w = measure(label, size) + 70.0;
    let h = size * 1.7;
    let r = Rect::new(centre.x - w / 2.0, centre.y - h / 2.0, w, h);
    // Notched tails tucked behind each end
    let tail = fade(tail, alpha);
    let (upper, lower) = (r.y + 12.0, r.bottom() + 12.0);
    for side in [-1.0f32, 1.0] {
        let inner = if side < 0.0 { r.x + 10.0 } else { r.right() - 10.0 };
        let outer = inner + side * 44.0;
        let notch = vec2(outer - side * 14.0, (upper + lower) / 2.0);
        draw_triangle(vec2(inner, upper), vec2(outer, upper), notch, tail);
        draw_triangle(vec2(inner, upper), notch, vec2(inner, lower), tail);
        draw_triangle(vec2(inner, lower), notch, vec2(outer, lower), tail);
    }
    drop_shadow(r, 12.0, alpha);
    rounded_rect(r, 12.0, fade(top, alpha), fade(bottom, alpha));
    rounded_outline(r, 12.0, 3.0, fade(INK, 0.5 * alpha));
    if style == Ribbon::Gold {
        text(label, centre.x, centre.y + size * 0.36, size, fade(ink, alpha), Align::Center);
    } else {
        shadowed(label, centre.x, centre.y + size * 0.36, size, fade(ink, alpha), Align::Center);
    }
}

/// A paw print: a big pad and four toes.
pub(crate) fn paw(at: Vec2, scale: f32, color: Color) {
    draw_ellipse(at.x, at.y + 6.0 * scale, 16.0 * scale, 13.0 * scale, 0.0, color);
    for (dx, dy) in [(-17.0, -10.0), (-6.0, -20.0), (6.0, -20.0), (17.0, -10.0)] {
        draw_circle(at.x + dx * scale, at.y + dy * scale, 6.5 * scale, color);
    }
}

fn shadow_ellipse(at: Vec2, w: f32, h: f32, alpha: f32) {
    draw_ellipse(at.x, at.y, w, h, 0.0, Color::new(0.0, 0.0, 0.0, alpha));
}

/// Draw a frame scaled to fit a `size` box, anchored at `at` (usually its feet).
pub(crate) fn fit_sprite(sprites: &Sprites, name: &str, at: Vec2, size: f32, facing: f32, tint: Color) {
    let frame = sprites.get(name).size();
    let scale = size / frame.x.max(frame.y);
    sprites.draw_ex(name, at, facing, scale, 0.0, tint);
}

/// Draw a frame scaled to fit a `size` box centred on `centre`, wherever its anchor is.
pub(crate) fn fit_sprite_centred(sprites: &Sprites, name: &str, centre: Vec2, size: f32, facing: f32, tint: Color) {
    let frame = sprites.get(name);
    let scale = size / frame.size().x.max(frame.size().y);
    let at = centre - frame.size() * scale / 2.0 + frame.anchor_for(facing) * scale;
    sprites.draw_ex(name, at, facing, scale, 0.0, tint);
}

/// Seconds since the game started: for ambient motion that shouldn't restart with each screen.
fn ambient_time() -> f32 {
    get_time() as f32
}

/// `1234567` -> "1,234,567".
pub fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Text, panels and easing shared by the menu screens and the HUD.
pub mod paint {
    use macroquad::prelude::*;

    use crate::camera::View;
    use crate::consts::{VIEW_H, VIEW_W};

    pub const INK: Color = Color::new(0.12, 0.07, 0.05, 1.0);
    pub const CREAM: Color = Color::new(1.0, 0.96, 0.88, 1.0);
    pub const GOLD_TEXT: Color = Color::new(1.0, 0.82, 0.32, 1.0);
    pub const ORANGE_TEXT: Color = Color::new(1.0, 0.62, 0.25, 1.0);
    pub const LEAF_TEXT: Color = Color::new(0.66, 0.90, 0.46, 1.0);
    const PANEL_TOP: Color = Color::new(0.10, 0.17, 0.13, 0.88);
    const PANEL_BOTTOM: Color = Color::new(0.05, 0.10, 0.08, 0.92);
    const PANEL_EDGE: Color = Color::new(1.0, 0.92, 0.75, 0.22);
    /// Points per quarter circle on rounded corners.
    const CORNER_STEPS: usize = 8;

    #[derive(Clone, Copy)]
    pub enum Align {
        Left,
        Center,
        Right,
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color::new(r, g, b, 1.0)
    }

    /// `color` with its alpha scaled by `alpha`.
    pub fn fade(color: Color, alpha: f32) -> Color {
        Color::new(color.r, color.g, color.b, color.a * alpha.clamp(0.0, 1.0))
    }

    /// Sizes text is rasterised at. The font atlas keeps every size it has drawn and never
    /// lets go, so snapping to these stops each new window size from adding a fresh set.
    const RASTER_SIZES: [u16; 22] =
        [10, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 40, 48, 56, 64, 72, 84, 96, 112, 128, 160, 192];

    /// The font size to rasterise at and the scale to draw it with, so text is rendered at the
    /// screen's real resolution rather than blown up from playfield pixels.
    fn font(size: f32) -> (u16, f32) {
        let scale = View::fit().scale.max(0.05);
        let wanted = (size * scale).round().max(1.0);
        let px = RASTER_SIZES.into_iter().find(|&px| px as f32 >= wanted).unwrap_or(RASTER_SIZES[RASTER_SIZES.len() - 1]);
        (px, size / px as f32)
    }

    pub fn measure(text: &str, size: f32) -> f32 {
        let (px, scale) = font(size);
        measure_text(text, None, px, scale).width
    }

    /// The largest size up to `max` at which `text` fits in `width`.
    pub fn fit_size(text: &str, max: f32, width: f32) -> f32 {
        let w = measure(text, max);
        if w <= width { max } else { (max * width / w).floor() }
    }

    /// Plain text with its baseline at `y`.
    pub fn text(text: &str, x: f32, y: f32, size: f32, color: Color, align: Align) {
        if color.a <= 0.0 {
            return;
        }
        let (px, scale) = font(size);
        let x = match align {
            Align::Left => x,
            Align::Center => x - measure(text, size) / 2.0,
            Align::Right => x - measure(text, size),
        };
        draw_text_ex(text, x, y, TextParams { font_size: px, font_scale: scale, color, ..Default::default() });
    }

    /// Text with a soft drop shadow.
    pub fn shadowed(s: &str, x: f32, y: f32, size: f32, color: Color, align: Align) {
        let d = (size * 0.07).max(2.0);
        text(s, x + d, y + d, size, fade(Color::new(0.0, 0.0, 0.0, 0.55), color.a), align);
        text(s, x, y, size, color, align);
    }

    /// Big text with an outline all round (thicker on bigger text) and a drop shadow, for titles.
    pub fn outlined(s: &str, x: f32, y: f32, size: f32, fill: Color, outline: Color, align: Align) {
        let width = (size * 0.06).clamp(2.5, 6.0);
        let d = width * 1.6;
        text(s, x + d * 0.5, y + d, size, fade(Color::new(0.0, 0.0, 0.0, 0.45), fill.a), align);
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            text(s, x + a.cos() * width, y + a.sin() * width, size, outline, align);
        }
        text(s, x, y, size, fill, align);
    }

    /// Split `text` into lines no wider than `width`, breaking between words.
    pub fn wrap(text: &str, size: f32, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if !line.is_empty() && measure(&candidate, size) > width {
                    lines.push(std::mem::replace(&mut line, word.to_string()));
                } else {
                    line = candidate;
                }
            }
            lines.push(line);
        }
        lines
    }

    /// Centred, word-wrapped text starting at `top`. Returns where it ends.
    pub fn paragraph(s: &str, centre_x: f32, top: f32, size: f32, width: f32, spacing: f32, color: Color) -> f32 {
        let mut y = top + size * 0.8;
        for line in wrap(s, size, width) {
            shadowed(&line, centre_x, y, size, color, Align::Center);
            y += size * spacing;
        }
        y
    }

    /// The outline of a rounded rectangle, clockwise from the top of the top-right corner.
    fn rounded_points(r: Rect, radius: f32) -> Vec<Vec2> {
        let radius = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
        let corners = [
            (vec2(r.right() - radius, r.y + radius), -90.0f32),
            (vec2(r.right() - radius, r.bottom() - radius), 0.0),
            (vec2(r.x + radius, r.bottom() - radius), 90.0),
            (vec2(r.x + radius, r.y + radius), 180.0),
        ];
        let mut points = Vec::with_capacity(4 * (CORNER_STEPS + 1));
        for (centre, start) in corners {
            for i in 0..=CORNER_STEPS {
                let a = (start + 90.0 * i as f32 / CORNER_STEPS as f32).to_radians();
                points.push(centre + vec2(a.cos(), a.sin()) * radius);
            }
        }
        points
    }

    /// A rounded rectangle filled with a vertical gradient, drawn as one mesh so translucent
    /// colours don't double up where pieces would overlap.
    pub fn rounded_rect(r: Rect, radius: f32, top: Color, bottom: Color) {
        if top.a <= 0.0 && bottom.a <= 0.0 {
            return;
        }
        let shade = |p: Vec2| Color::from_vec(top.to_vec().lerp(bottom.to_vec(), ((p.y - r.y) / r.h).clamp(0.0, 1.0)));
        let centre = r.center();
        let mut vertices = vec![Vertex::new(centre.x, centre.y, 0.0, 0.0, 0.0, shade(centre))];
        let points = rounded_points(r, radius);
        vertices.extend(points.iter().map(|p| Vertex::new(p.x, p.y, 0.0, 0.0, 0.0, shade(*p))));
        let n = points.len() as u16;
        let indices = (0..n).flat_map(|i| [0, 1 + i, 1 + (i + 1) % n]).collect();
        draw_mesh(&Mesh { vertices, indices, texture: None });
    }

    /// A rounded rectangle's border, `width` thick inside its edge.
    pub fn rounded_outline(r: Rect, radius: f32, width: f32, color: Color) {
        if color.a <= 0.0 {
            return;
        }
        let outer = rounded_points(r, radius);
        let inner = rounded_points(Rect::new(r.x + width, r.y + width, r.w - 2.0 * width, r.h - 2.0 * width),
                                   (radius - width).max(0.0));
        let vertices: Vec<Vertex> = outer.iter().chain(&inner).map(|p| Vertex::new(p.x, p.y, 0.0, 0.0, 0.0, color)).collect();
        let n = outer.len() as u16;
        let indices = (0..n).flat_map(|i| {
            let j = (i + 1) % n;
            [i, j, n + i, j, n + j, n + i]
        }).collect();
        draw_mesh(&Mesh { vertices, indices, texture: None });
    }

    /// Only draw inside `area` (in playfield coordinates) until called again with None.
    pub fn clip(area: Option<Rect>) {
        let clip = area.map(|r| {
            let (w, h) = miniquad::window::screen_size();
            let scale = View::fit().scale * miniquad::window::dpi_scale();
            let x = w / 2.0 + (r.x - VIEW_W / 2.0) * scale;
            let y = h / 2.0 + (r.y - VIEW_H / 2.0) * scale;
            (x.round() as i32, y.round() as i32, (r.w * scale).round() as i32, (r.h * scale).round() as i32)
        });
        // Safety: the internal GL context is only borrowed for this call, on the main thread
        unsafe { get_internal_gl() }.quad_gl.scissor(clip);
    }

    /// `r` with `by` added on every side.
    pub fn grow(r: Rect, by: f32) -> Rect {
        Rect::new(r.x - by, r.y - by, r.w + 2.0 * by, r.h + 2.0 * by)
    }

    /// A rectangle shading from `top` to `bottom`.
    pub fn gradient_rect(r: Rect, top: Color, bottom: Color) {
        let v = |x: f32, y: f32, c: Color| Vertex::new(x, y, 0.0, 0.0, 0.0, c);
        let vertices = vec![v(r.x, r.y, top), v(r.right(), r.y, top), v(r.right(), r.bottom(), bottom), v(r.x, r.bottom(), bottom)];
        draw_mesh(&Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: None });
    }

    /// Darken the whole window, letterboxes and all, whatever the camera.
    pub fn dim_screen(color: Color) {
        push_camera_state();
        set_default_camera();
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), color);
        pop_camera_state();
    }

    /// A soft shadow under a card.
    pub fn drop_shadow(r: Rect, radius: f32, alpha: f32) {
        let shadow = Color::new(0.0, 0.0, 0.0, 0.18 * alpha.clamp(0.0, 1.0));
        rounded_rect(Rect::new(r.x - 2.0, r.y + 6.0, r.w + 4.0, r.h + 6.0), radius + 2.0, shadow, shadow);
        rounded_rect(Rect::new(r.x + 2.0, r.y + 4.0, r.w - 4.0, r.h), radius, shadow, shadow);
    }

    /// The dark, softly lit card the menus put their words on.
    pub fn panel(r: Rect, radius: f32, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        drop_shadow(r, radius, alpha);
        rounded_rect(r, radius, fade(PANEL_TOP, alpha), fade(PANEL_BOTTOM, alpha));
        rounded_outline(r, radius, 2.0, fade(PANEL_EDGE, alpha));
    }

    /// A lighter, more see-through panel for the HUD, so the game shows through.
    pub fn glass(r: Rect, radius: f32) {
        rounded_rect(r, radius, Color::new(0.06, 0.10, 0.08, 0.7), Color::new(0.04, 0.07, 0.06, 0.78));
        rounded_outline(r, radius, 2.0, Color::new(1.0, 0.92, 0.75, 0.25));
    }

    /// Eased 0..1 progress of something that starts `delay` seconds in and takes `duration`.
    pub fn appear(time: f32, delay: f32, duration: f32) -> f32 {
        let t = ((time - delay) / duration).clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }

    /// 0..1 with a little overshoot, for things that pop into place.
    pub fn ease_out_back(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0) - 1.0;
        let c = 1.70158;
        1.0 + (c + 1.0) * t * t * t + c * t * t
    }

    /// A gentle bob: `amount` px at `speed` cycles per second.
    pub fn wave(t: f32, speed: f32, amount: f32, phase: f32) -> f32 {
        (t * speed * std::f32::consts::TAU + phase).sin() * amount
    }

    /// A steady pseudo-random 0..1 for index `i`, for scattering decorations without state.
    pub fn hash(i: usize) -> f32 {
        let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        x = x.wrapping_mul(0x297A_2D39);
        x ^= x >> 15;
        x as f32 / u32::MAX as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quick tapper can't leave the first level's intro before the how-to-play card is fully in;
    /// later levels move on as soon as before.
    #[test]
    fn the_first_intro_waits_for_the_how_to_play_card() {
        assert!(level_intro_wait(true) >= BASICS_CARD_IN.0 + BASICS_CARD_IN.1);
        assert_eq!(level_intro_wait(false), 0.5);
    }
}
