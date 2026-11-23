//! The map: one tall picture of Whisker Wood that Kitty climbs region by region, from Mossy
//! Hollow at the foot to the Tallest Tree in the clouds, with the endless climb above it all.
//! Levels are stepping stones on a winding paw-print path, and Kitty sits by the newest. Tap a
//! stone for its card, and Play. Drag (or the wheel, or Page Up and Down) to scroll; the arrow
//! keys move between stones and Enter opens and plays.

use std::f32::consts::PI;

use macroquad::prelude::*;

use crate::campaign::{in_roster, Goal, LevelId, RegionId, ALL_THREATS, REGION_IDS};
use crate::consts::*;
use crate::flow;
use crate::input::menu::{MenuInput, Nav};
use crate::save::SaveData;
use crate::sprites::Assets;
use crate::story::paint::{self, Align, CREAM, GOLD_TEXT, INK};
use crate::story::{self, thousands, Ribbon};
use crate::ui::{self, Style, BACK};

/// The map's height: the endless climb's sky, five regions, and the ground at the foot.
pub const TOP_H: f32 = 600.0;
pub const REGION_H: f32 = 1000.0;
const BOTTOM_H: f32 = 400.0;
pub const MAP_H: f32 = TOP_H + 5.0 * REGION_H + BOTTOM_H;
/// A stepping stone's radius: about 12 mm across on a phone.
pub const STONE_R: f32 = 46.0;
/// How far up its region each stage's stone sits, and where across: regions alternate which way
/// the path zig-zags.
const STAGE_UP: [f32; 3] = [0.18, 0.45, 0.75];
const ZIGZAG: [[f32; 3]; 2] = [[170.0, 420.0, 260.0], [430.0, 180.0, 340.0]];
const ENDLESS_AT: Vec2 = Vec2::from_array([300.0, 330.0]);
/// The bar across the top of the screen, and its back button.
const BAR: Rect = Rect { x: 0.0, y: 0.0, w: VIEW_W, h: 116.0 };
/// The wood's name in the bar (tapped seven times in a playtest build, every level opens).
pub const TITLE: Rect = Rect { x: 150.0, y: 20.0, w: 300.0, h: 80.0 };
/// A stone's card, sliding up from the bottom.
const CARD: Rect = Rect { x: 20.0, y: 690.0, w: 560.0, h: 490.0 };
const CARD_IN: f32 = 0.25;
/// Kitty's hop along the path to a newly opened stone.
const WALK_TIME: f32 = 1.2;
const WHEEL_STEP: f32 = 80.0;
/// How quickly a fling slows (per second) and a scroll eases to where it's headed.
const FLING_DECAY: f32 = 4.0;
const EASE: f32 = 6.0;
const TOAST_TIME: f32 = 1.8;

/// A place on the map to play.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stop {
    Level(LevelId),
    Endless,
}

impl Stop {
    /// Every stop, from the foot of the map to the top.
    pub fn all() -> impl Iterator<Item = Stop> {
        LevelId::all().map(Stop::Level).chain(std::iter::once(Stop::Endless))
    }

    pub fn up(self) -> Option<Stop> {
        match self {
            Stop::Level(id) => Some(id.next().map_or(Stop::Endless, Stop::Level)),
            Stop::Endless => None,
        }
    }

    pub fn down(self) -> Option<Stop> {
        match self {
            Stop::Level(id) => (id.0 > 0).then(|| Stop::Level(LevelId(id.0 - 1))),
            Stop::Endless => Some(Stop::Level(LevelId::last())),
        }
    }
}

/// A region's stretch of the map.
pub fn band(region: RegionId) -> Rect {
    Rect::new(0.0, TOP_H + (4 - region.index()) as f32 * REGION_H, VIEW_W, REGION_H)
}

/// Where a stop's stone is, in map coordinates (y down from the top of the map).
pub fn stone(stop: Stop) -> Vec2 {
    match stop {
        Stop::Endless => ENDLESS_AT,
        Stop::Level(id) => {
            let def = id.def();
            let stage = def.stage - 1;
            vec2(ZIGZAG[def.region.index() % 2][stage], band(def.region).bottom() - STAGE_UP[stage] * REGION_H)
        }
    }
}

/// The stop whose stone is under `p` (map coordinates), generously.
pub fn stop_at(p: Vec2) -> Option<Stop> {
    Stop::all().find(|&stop| stone(stop).distance(p) <= STONE_R + 12.0)
}

pub fn max_scroll() -> f32 {
    MAP_H - VIEW_H
}

/// The scroll that shows `stop` a little below the middle of the screen.
pub fn scroll_for(stop: Stop) -> f32 {
    (stone(stop).y - VIEW_H * 0.55).clamp(0.0, max_scroll())
}

/// The scroll that shows `stop` above its open card.
fn scroll_above_card(stop: Stop) -> f32 {
    (stone(stop).y - 470.0).clamp(0.0, max_scroll())
}

/// Whether a stop can be played.
pub fn is_open(save: &SaveData, stop: Stop) -> bool {
    match stop {
        Stop::Level(id) => flow::unlocked(save, id),
        Stop::Endless => flow::endless_unlocked(save),
    }
}

/// Where Kitty waits: by the newest open level, or the endless climb once Luna's home.
pub fn kitty_stop(save: &SaveData) -> Stop {
    if save.cleared(LevelId::last().def().key) { Stop::Endless } else { Stop::Level(flow::newest(save)) }
}

/// The path from one stone up to the next: an S-bend, `t` of the way along.
fn bend(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    let mid = (a.y + b.y) / 2.0;
    let (c1, c2) = (vec2(a.x, mid), vec2(b.x, mid));
    let u = 1.0 - t;
    a * u * u * u + c1 * 3.0 * u * u * t + c2 * 3.0 * u * t * t + b * t * t * t
}

/// What the player chose on the map.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapAction {
    Play(LevelId),
    PlayEndless,
    Back,
}

pub struct MapState {
    /// The map y at the top of the screen.
    pub scroll: f32,
    vel: f32,
    target: Option<f32>,
    /// The stone whose card is open, and how long it's been open.
    card: Option<(Stop, f32)>,
    /// The keyboard's stone.
    focus: Stop,
    /// Kitty hopping from one stone to a newly opened one: from, to, and how long she's been at it.
    walk: Option<(Stop, Stop, f32)>,
    toast: Option<(String, f32)>,
    /// Whether a tap can press the card's Play: only once it has settled with no finger down, so
    /// a finger that went down while it slid in (a double tap on a stone) can't.
    play_armed: bool,
}

impl MapState {
    pub fn new(save: &SaveData) -> Self {
        let stop = kitty_stop(save);
        Self { scroll: scroll_for(stop), vel: 0.0, target: None, card: None, focus: stop, walk: None, toast: None, play_armed: false }
    }

    /// Back on the map. After a clear that opened a stone, Kitty hops from `from` to it and its
    /// card opens; otherwise the newest stone is in view.
    pub fn enter(&mut self, save: &SaveData, opened: Option<(Stop, Stop)>) {
        self.card = None;
        self.vel = 0.0;
        self.toast = None;
        match opened {
            Some((from, to)) => {
                self.scroll = scroll_for(from);
                self.target = Some(scroll_above_card(to));
                self.walk = Some((from, to, 0.0));
                self.focus = to;
            }
            None => {
                let stop = kitty_stop(save);
                self.scroll = scroll_for(stop);
                self.target = None;
                self.walk = None;
                self.focus = stop;
            }
        }
    }

    /// Show `message` briefly over the map.
    pub fn say(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), TOAST_TIME));
    }

    fn open_card(&mut self, stop: Stop) {
        self.card = Some((stop, 0.0));
        self.play_armed = false;
        self.focus = stop;
        self.target = Some(scroll_above_card(stop));
    }

    pub fn update(&mut self, input: &MenuInput, save: &SaveData, dt: f32) -> Option<MapAction> {
        if let Some((_, open_for)) = &mut self.card {
            *open_for += dt;
        }
        if let Some((_, left)) = &mut self.toast {
            *left -= dt;
        }
        self.toast = self.toast.take().filter(|(_, left)| *left > 0.0);
        // Kitty's hop to a newly opened stone, then its card
        if let Some((_, to, t)) = &mut self.walk {
            *t += dt;
            if *t >= WALK_TIME {
                let to = *to;
                self.walk = None;
                self.open_card(to);
            }
            self.glide(dt);
            return None;
        }
        if input.tapped(BACK) {
            return Some(MapAction::Back);
        }
        let card_was_open = self.card.is_some();
        match input.nav {
            Some(Nav::Back) => {
                if self.card.take().is_none() {
                    return Some(MapAction::Back);
                }
            }
            Some(Nav::Up) => {
                if let Some(up) = self.focus.up().filter(|&s| is_open(save, s)) {
                    self.move_focus(up);
                }
            }
            Some(Nav::Down) => {
                if let Some(down) = self.focus.down() {
                    self.move_focus(down);
                }
            }
            Some(Nav::PageUp) => self.target = Some((self.target.unwrap_or(self.scroll) - VIEW_H * 0.8).max(0.0)),
            Some(Nav::PageDown) => self.target = Some((self.target.unwrap_or(self.scroll) + VIEW_H * 0.8).min(max_scroll())),
            Some(Nav::Activate) if !card_was_open && is_open(save, self.focus) => self.open_card(self.focus),
            _ => {}
        }
        // The card: Play (or Enter, if it was already open), its close button, or a tap
        // anywhere else to close it
        if let Some((stop, open_for)) = self.card {
            // Play by a tap that started once the card was in place (so a quick double tap on a
            // stone can't press it unseen), or by Enter once it was open before this frame
            let settled = open_for >= CARD_IN;
            let armed = self.play_armed;
            self.play_armed = settled && (armed || (input.held.is_none() && input.tap.is_none()));
            let play = play_button(0.0);
            if (armed && input.tapped(play)) || (settled && card_was_open && input.nav == Some(Nav::Activate)) {
                return Some(match stop {
                    Stop::Level(id) => MapAction::Play(id),
                    Stop::Endless => MapAction::PlayEndless,
                });
            }
            if input.tapped(close_button(card_slide(open_for))) {
                self.card = None;
                return None;
            }
            match input.tap {
                Some(tap) if CARD.contains(tap.up) => return None,
                Some(_) => self.card = None,
                None => {}
            }
        }
        // A stone tapped (where the finger went down and came up), below the bar, which also
        // covers a tall screen's letterbox above it
        if let Some(tap) = input.tap.filter(|tap| tap.down.y > BAR.bottom() && tap.up.y > BAR.bottom()) {
            let on = |p: Vec2| stop_at(p + vec2(0.0, self.scroll));
            if let Some(stop) = on(tap.up).filter(|&stop| on(tap.down) == Some(stop)) {
                if is_open(save, stop) {
                    self.open_card(stop);
                } else {
                    self.toast = Some((locked_message(stop), TOAST_TIME));
                }
            }
        }
        // Scrolling: dragged, flung, or wheeled
        if input.drag_dy != 0.0 {
            self.scroll -= input.drag_dy;
            self.vel = 0.0;
            self.target = None;
        }
        if let Some(fling) = input.fling {
            self.vel = -fling;
        }
        if input.wheel != 0.0 {
            self.scroll -= wheel_distance(input.wheel);
            self.target = None;
        }
        self.glide(dt);
        None
    }

    fn move_focus(&mut self, stop: Stop) {
        if self.card.is_some() {
            self.open_card(stop);
        } else {
            self.focus = stop;
            self.target = Some(scroll_for(stop));
        }
    }

    /// Coast after a fling, or ease toward where the scroll is headed, keeping to the map.
    fn glide(&mut self, dt: f32) {
        self.scroll += self.vel * dt;
        self.vel *= (-FLING_DECAY * dt).exp();
        if let Some(target) = self.target {
            self.scroll += (target - self.scroll) * (dt * EASE).min(1.0);
            if (target - self.scroll).abs() < 0.5 {
                self.scroll = target;
                self.target = None;
            }
        }
        let clamped = self.scroll.clamp(0.0, max_scroll());
        if clamped != self.scroll {
            self.vel = 0.0;
        }
        self.scroll = clamped;
    }

    /// Where Kitty is on the map: by her stone, or partway along her hop to the next.
    fn kitty_at(&self, save: &SaveData) -> (Vec2, f32) {
        match self.walk {
            Some((from, to, t)) => {
                let f = (t / WALK_TIME).min(1.0);
                let eased = f * f * (3.0 - 2.0 * f);
                let hop = (eased * PI * 3.0).sin().abs() * 26.0;
                let facing = if stone(to).x >= stone(from).x { 1.0 } else { -1.0 };
                (bend(stone(from), stone(to), eased) + KITTY_SEAT - vec2(0.0, hop), facing)
            }
            None => (stone(kitty_stop(save)) + KITTY_SEAT, 1.0),
        }
    }
}

/// Kitty sits just to the left of her stone.
const KITTY_SEAT: Vec2 = Vec2::from_array([-64.0, 14.0]);

/// How far a turn of the mouse wheel scrolls, by what each platform reports: browsers pixels,
/// Windows 120 a notch (less at a time from a precision touchpad), macOS pixels or 10 a notch,
/// and X11 a notch at a time. A frame never scrolls more than half a screen.
fn wheel_distance(wheel: f32) -> f32 {
    let distance = if cfg!(target_arch = "wasm32") {
        wheel * 0.8
    } else if cfg!(windows) {
        wheel / 120.0 * WHEEL_STEP
    } else if cfg!(target_os = "macos") {
        wheel * 4.0
    } else {
        wheel * WHEEL_STEP
    };
    distance.clamp(-VIEW_H * 0.5, VIEW_H * 0.5)
}

fn locked_message(stop: Stop) -> String {
    match stop {
        Stop::Level(id) if id.0 > 0 => format!("Clear {} first!", LevelId(id.0 - 1).def().title),
        Stop::Level(_) => String::new(),
        Stop::Endless => "Bring Luna home first!".to_string(),
    }
}

/// How far below its place the card still is, `open_for` seconds after it began sliding in.
fn card_slide(open_for: f32) -> f32 {
    (1.0 - paint::appear(open_for, 0.0, CARD_IN)) * (CARD.h + 40.0)
}

fn play_button(slide: f32) -> Rect {
    Rect::new(CARD.x + 40.0, CARD.bottom() - 130.0 + slide, CARD.w - 80.0, ui::BUTTON_H)
}

fn close_button(slide: f32) -> Rect {
    Rect::new(CARD.right() - 86.0, CARD.y + 10.0 + slide, 76.0, 76.0)
}

fn mmss(seconds: f32) -> String {
    let s = seconds.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Draw the map, with the screen camera set.
pub fn draw(assets: &Assets, state: &MapState, save: &SaveData, input: &MenuInput, time: f32) {
    let scroll = state.scroll;
    clear_background(paint::rgb(0.55, 0.72, 0.95));
    draw_sky(assets, scroll, save, time);
    for region in REGION_IDS {
        draw_region(assets, region, scroll, save, time);
    }
    draw_ground(scroll);
    draw_path(save, scroll);
    for region in REGION_IDS {
        draw_region_name(assets, region, scroll, save);
    }
    for stop in Stop::all() {
        draw_stone(assets, stop, save, scroll, time, state.focus == stop && state.walk.is_none());
    }
    let (kitty, facing) = state.kitty_at(save);
    let bob = if state.walk.is_none() { (time * 2.0).sin() * 1.5 } else { 0.0 };
    assets.kitty.draw_ex("sit_0", vec2(kitty.x, kitty.y - scroll + bob), facing, 0.85, 0.0, WHITE);
    draw_bar(save, input);
    draw_card(assets, state, save, input);
    if let Some((message, left)) = &state.toast {
        let alpha = (left / 0.3).min(1.0);
        let w = paint::measure(message, 28.0) + 60.0;
        let r = Rect::new(300.0 - w / 2.0, BAR.bottom() + 16.0, w, 64.0);
        paint::rounded_rect(r, 24.0, paint::fade(INK, alpha), paint::fade(INK, alpha));
        paint::rounded_outline(r, 24.0, 3.0, paint::fade(CREAM, 0.3 * alpha));
        paint::text(message, 300.0, r.y + 42.0, 28.0, paint::fade(CREAM, alpha), Align::Center);
    }
}

/// On screen: map rectangle `r` moved up by the scroll, if any of it shows (in the letterbox of a
/// tall screen too).
fn visible(r: Rect, scroll: f32) -> Option<Rect> {
    let on = Rect::new(r.x, r.y - scroll, r.w, r.h);
    (on.bottom() >= -200.0 && on.y <= VIEW_H + 200.0).then_some(on)
}

/// The sky above the regions, where the endless climb waits among the clouds.
fn draw_sky(assets: &Assets, scroll: f32, save: &SaveData, t: f32) {
    let Some(r) = visible(Rect::new(0.0, 0.0, VIEW_W, TOP_H), scroll) else { return };
    paint::gradient_rect(wide(r), paint::rgb(0.30, 0.48, 0.86), paint::rgb(0.72, 0.84, 1.0));
    for i in 0..7 {
        if let Some(cloud) = assets.scenery.cloud(i) {
            let scale = 0.45 + paint::hash(i + 40) * 0.35;
            let size = vec2(cloud.width(), cloud.height()) * scale;
            let x = (paint::hash(i + 50) * (VIEW_W + 200.0) + t * (6.0 + 4.0 * paint::hash(i))) % (VIEW_W + 200.0) - 100.0 - size.x / 2.0;
            let y = r.y + 40.0 + paint::hash(i + 60) * (TOP_H - 120.0);
            draw_texture_ex(cloud, x, y, paint::fade(WHITE, 0.9), DrawTextureParams { dest_size: Some(size), ..Default::default() });
        }
    }
    let open = flow::endless_unlocked(save);
    story::ribbon("Endless Climb", vec2(300.0, r.y + 150.0), 30.0, if open { 1.0 } else { 0.6 }, Ribbon::Gold);
}

/// A region's band: its colours, a few things that grow there, its name, and mist over it until
/// it's reached.
fn draw_region(assets: &Assets, region: RegionId, scroll: f32, save: &SaveData, t: f32) {
    let Some(r) = visible(band(region), scroll) else { return };
    let look = region.look();
    paint::gradient_rect(wide(r), look.band.0, look.band.1);
    let wood = look.wood;
    match region {
        RegionId::MossyHollow => {
            // Round oaks along the edges
            for (i, &(x, f)) in [(52.0, 0.12), (548.0, 0.28), (70.0, 0.55), (530.0, 0.7), (45.0, 0.88), (560.0, 0.92)].iter().enumerate() {
                let base = vec2(x, r.y + r.h * f);
                draw_rectangle(base.x - 9.0, base.y - 70.0, 18.0, 80.0, wood.trunk);
                for (k, offset) in [vec2(-26.0, -80.0), vec2(24.0, -86.0), vec2(0.0, -110.0)].into_iter().enumerate() {
                    let sprite = if (i + k) % 3 == 0 { "green_3" } else { "green_5" };
                    assets.foliage.draw_ex(sprite, base + offset, 1.0, 2.3, 0.0, look.foliage_tint);
                }
            }
        }
        RegionId::WhisperingPines => {
            for &(x, f, h) in &[(60.0, 0.15, 190.0), (540.0, 0.3, 230.0), (40.0, 0.5, 210.0), (555.0, 0.65, 180.0), (70.0, 0.85, 240.0), (520.0, 0.95, 200.0)] {
                pine(vec2(x, r.y + r.h * f), h, paint::rgb(0.10, 0.26, 0.20), wood.trunk);
            }
            story::fireflies_in(r, t, 14, 1.0);
        }
        RegionId::CraggyPeaks => {
            for &(x, w, f, h) in &[(110.0, 260.0, 0.45, 300.0), (480.0, 300.0, 0.3, 360.0), (300.0, 220.0, 0.8, 240.0)] {
                let base = r.y + r.h * f;
                let peak = vec2(x, base - h);
                draw_triangle(vec2(x - w / 2.0, base), vec2(x + w / 2.0, base), peak, paint::rgb(0.55, 0.52, 0.62));
                draw_triangle(vec2(x - w / 2.0, base), vec2(x - w * 0.05, base), peak, paint::rgb(0.47, 0.44, 0.54));
                let cap = 0.28;
                draw_triangle(peak, peak + vec2(-w / 2.0, h) * cap, peak + vec2(w / 2.0, h) * cap, paint::rgb(0.96, 0.97, 1.0));
            }
        }
        RegionId::WillowCreek => {
            // The creek winding down the middle
            let x_at = |y: f32| 300.0 + ((y - r.y) * 0.0065).sin() * 170.0;
            let mut y = r.y;
            while y < r.bottom() {
                let (a, b) = (vec2(x_at(y), y), vec2(x_at(y + 24.0), y + 24.0));
                draw_line(a.x, a.y, b.x, b.y, 74.0, paint::rgb(0.42, 0.66, 0.86));
                draw_line(a.x - 14.0, a.y, b.x - 14.0, b.y, 4.0, paint::fade(WHITE, 0.25));
                y += 24.0;
            }
            for &(x, f) in &[(50.0, 0.2), (550.0, 0.4), (45.0, 0.7), (555.0, 0.9)] {
                let base = vec2(x, r.y + r.h * f);
                draw_rectangle(base.x - 8.0, base.y - 60.0, 16.0, 70.0, wood.trunk);
                for k in 0..4 {
                    assets.foliage.draw_ex("green_4", base + vec2(-30.0 + 20.0 * k as f32, -80.0 + (k % 2) as f32 * 14.0), 1.0, 1.8, 0.4, look.foliage_tint);
                }
            }
        }
        RegionId::TallestTree => {
            // The great trunk itself
            let trunk = Rect::new(220.0, r.y, 160.0, r.h);
            draw_rectangle(trunk.x, trunk.y, trunk.w, trunk.h, wood.trunk);
            draw_rectangle(trunk.x + 26.0, trunk.y, 22.0, trunk.h, paint::fade(wood.light, 0.5));
            draw_rectangle(trunk.right() - 34.0, trunk.y, 34.0, trunk.h, paint::fade(wood.outline, 0.6));
            // A bank of cloud where it meets the sky, the trunk rising out of it
            for i in 0..16 {
                let radius = 34.0 + paint::hash(i + 300) * 16.0;
                draw_circle(-30.0 + i as f32 * 44.0, r.y + 6.0 + paint::hash(i + 310) * 10.0, radius, paint::rgb(0.86, 0.92, 1.0));
            }
            for i in 0..4 {
                if let Some(cloud) = assets.scenery.cloud(i + 3) {
                    let size = vec2(cloud.width(), cloud.height()) * 0.55;
                    let x = if i % 2 == 0 { -40.0 } else { VIEW_W - size.x + 40.0 };
                    draw_texture_ex(cloud, x, r.y + r.h * (0.1 + 0.25 * i as f32), paint::fade(WHITE, 0.8),
                                    DrawTextureParams { dest_size: Some(size), ..Default::default() });
                }
            }
        }
    }
    // A scalloped edge where the band below meets this one
    if region != RegionId::MossyHollow {
        let below = REGION_IDS[region.index() - 1].look().band.0;
        for i in 0..14 {
            draw_circle(i as f32 * 46.0, r.bottom(), 30.0 + paint::hash(i + region.index() * 20) * 12.0, below);
        }
    }
    if !reached(save, region) {
        paint::gradient_rect(wide(r), Color::new(0.90, 0.93, 1.0, 0.75), Color::new(0.90, 0.93, 1.0, 0.55));
    }
}

/// Whether Kitty has got as far as `region`.
fn reached(save: &SaveData, region: RegionId) -> bool {
    region.levels().next().is_some_and(|first| flow::unlocked(save, first))
}

/// A region's name on a ribbon at the top of its band, with the clues found so far either side.
fn draw_region_name(assets: &Assets, region: RegionId, scroll: f32, save: &SaveData) {
    let Some(r) = visible(band(region), scroll) else { return };
    let alpha = if reached(save, region) { 1.0 } else { 0.7 };
    story::ribbon(region.info().name, vec2(300.0, r.y + 50.0), 30.0, alpha, Ribbon::Leaf);
    let found = flow::clues_found(save, region);
    for (k, object) in region.clue_objects().into_iter().enumerate() {
        let at = vec2(84.0 + 432.0 * k as f32, r.y + 50.0);
        draw_circle(at.x, at.y, 24.0, paint::fade(INK, 0.5 * alpha));
        draw_circle(at.x, at.y, 21.0, paint::fade(if k < found { CREAM } else { paint::fade(CREAM, 0.7) }, alpha));
        if k < found {
            story::clue_icon(assets, object, at, 40.0, alpha);
        } else {
            paint::text("?", at.x, at.y + 10.0, 28.0, paint::fade(INK, alpha), Align::Center);
        }
    }
}

/// `r` stretched out sideways, over the letterbox of a wide screen.
fn wide(r: Rect) -> Rect {
    Rect::new(r.x - 300.0, r.y, r.w + 600.0, r.h)
}

/// A pine: tiers of dark green over a short trunk, `h` tall from `base`.
fn pine(base: Vec2, h: f32, needles: Color, trunk: Color) {
    draw_rectangle(base.x - 7.0, base.y - h * 0.18, 14.0, h * 0.2, trunk);
    for k in 0..3 {
        let w = h * (0.55 - 0.13 * k as f32);
        let bottom = base.y - h * (0.12 + 0.24 * k as f32);
        draw_triangle(vec2(base.x - w / 2.0, bottom), vec2(base.x + w / 2.0, bottom), vec2(base.x, bottom - h * 0.42), needles);
    }
}

/// The grass and soil at the foot of the map, where the climb begins (on down into the letterbox
/// of a tall screen).
fn draw_ground(scroll: f32) {
    let Some(r) = visible(Rect::new(0.0, MAP_H - BOTTOM_H, VIEW_W, BOTTOM_H), scroll) else { return };
    let look = RegionId::MossyHollow.look();
    let [grass, grass_dark, soil] = look.ground;
    draw_rectangle(r.x - 300.0, r.y, r.w + 600.0, r.h + 800.0, soil);
    draw_rectangle(r.x - 300.0, r.y, r.w + 600.0, 22.0, grass);
    draw_rectangle(r.x - 300.0, r.y + 18.0, r.w + 600.0, 8.0, grass_dark);
}

/// Paw prints from the foot of the map up through every stone: gold as far as Kitty can go,
/// faint beyond.
fn draw_path(save: &SaveData, scroll: f32) {
    let mut from = vec2(300.0, MAP_H - BOTTOM_H + 40.0);
    for stop in Stop::all() {
        let to = stone(stop);
        let color = if is_open(save, stop) { Color::new(1.0, 0.86, 0.42, 0.9) } else { Color::new(1.0, 1.0, 1.0, 0.28) };
        let steps = (from.distance(to) / 44.0) as usize;
        for k in 1..steps {
            let t = k as f32 / steps as f32;
            let p = bend(from, to, t);
            // Not under the stones themselves
            if p.distance(from) < STONE_R + 14.0 || p.distance(to) < STONE_R + 14.0 {
                continue;
            }
            let ahead = (bend(from, to, (t + 0.01).min(1.0)) - p).normalize_or_zero();
            let side = vec2(-ahead.y, ahead.x) * if k % 2 == 0 { 9.0 } else { -9.0 };
            let at = p + side - vec2(0.0, scroll);
            if (-30.0..VIEW_H + 30.0).contains(&at.y) {
                story::paw(at, 0.32, color);
            }
        }
        from = to;
    }
}

fn draw_stone(assets: &Assets, stop: Stop, save: &SaveData, scroll: f32, t: f32, focused: bool) {
    let c = stone(stop) - vec2(0.0, scroll);
    if c.y < -160.0 || c.y > VIEW_H + 120.0 {
        return;
    }
    let open = is_open(save, stop);
    // The newest open stone glows, to say "here next"
    let newest = open && kitty_stop(save) == stop;
    if newest {
        let pulse = (t * 3.0).sin() * 0.5 + 0.5;
        draw_circle(c.x, c.y, STONE_R + 10.0 + pulse * 6.0, Color::new(1.0, 0.9, 0.45, 0.35 + 0.25 * pulse));
    }
    draw_circle(c.x + 3.0, c.y + 6.0, STONE_R, Color::new(0.0, 0.0, 0.0, 0.2));
    let (rim, face) = if open { (paint::rgb(0.80, 0.55, 0.20), paint::rgb(1.0, 0.95, 0.84)) } else { (paint::rgb(0.42, 0.44, 0.48), paint::rgb(0.66, 0.68, 0.72)) };
    draw_circle(c.x, c.y, STONE_R, rim);
    draw_circle(c.x, c.y - 2.0, STONE_R - 6.0, face);
    if focused {
        draw_circle_lines(c.x, c.y, STONE_R + 5.0, 4.0, WHITE);
    }
    match stop {
        Stop::Endless => {
            if open {
                for side in [-1.0, 1.0] {
                    draw_circle_lines(c.x + side * 10.0, c.y - 2.0, 10.0, 5.0, INK);
                }
            } else {
                paint::text("?", c.x, c.y + 16.0, 48.0, paint::fade(INK, 0.5), Align::Center);
            }
        }
        Stop::Level(id) => {
            let def = id.def();
            let record = save.level(def.key);
            if open {
                paint::text(&def.stage.to_string(), c.x, c.y + 15.0, 44.0, INK, Align::Center);
            } else {
                padlock(c);
            }
            if record.cleared {
                let stars = [record.stars.top, record.stars.no_bumps, record.stars.quick];
                for (k, won) in stars.into_iter().enumerate() {
                    let at = c + vec2(-28.0 + 28.0 * k as f32, STONE_R + 10.0 + if k == 1 { 6.0 } else { 0.0 });
                    let fill = if won { GOLD_TEXT } else { paint::fade(CREAM, 0.35) };
                    ui::star(at, 13.0, fill, paint::fade(INK, 0.7));
                }
            }
            match def.goal {
                Goal::Clue(_) => magnifier(c + vec2(STONE_R * 0.75, -STONE_R * 0.75), open),
                Goal::Rescue => {
                    // The friend waiting in their nest above the stone, or cheering once home
                    let friend = def.friend();
                    let home = flow::rescued(save, friend);
                    let frame = if home { friend.cheer_frame((t * 5.0) as usize) } else { friend.help_frame() };
                    let hop = if home { (t * 5.0).sin().abs() * 8.0 } else { 0.0 };
                    let alpha = if open { 1.0 } else { 0.55 };
                    story::friend_in_nest(assets, &frame, c - vec2(0.0, STONE_R + 30.0), 0.8, hop, -1.0, alpha);
                }
            }
        }
    }
}

fn padlock(c: Vec2) {
    let body = Rect::new(c.x - 16.0, c.y - 6.0, 32.0, 26.0);
    draw_circle_lines(c.x, c.y - 8.0, 11.0, 5.0, paint::fade(INK, 0.6));
    paint::rounded_rect(body, 6.0, paint::fade(INK, 0.6), paint::fade(INK, 0.7));
}

/// The magnifying glass on a clue level's stone.
fn magnifier(at: Vec2, open: bool) {
    let alpha = if open { 1.0 } else { 0.5 };
    draw_line(at.x + 7.0, at.y + 7.0, at.x + 17.0, at.y + 17.0, 6.0, paint::fade(INK, alpha));
    draw_circle(at.x, at.y, 14.0, paint::fade(INK, alpha));
    draw_circle(at.x, at.y, 10.0, paint::fade(paint::rgb(0.78, 0.92, 1.0), alpha));
}

/// The bar over the top of the map: back, the wood's name, and the stars won.
fn draw_bar(save: &SaveData, input: &MenuInput) {
    let bar = Rect::new(-320.0, -240.0, VIEW_W + 640.0, BAR.h + 240.0);
    paint::rounded_rect(bar, 24.0, paint::rgb(0.08, 0.11, 0.09), paint::rgb(0.06, 0.09, 0.08));
    paint::glass(bar, 24.0);
    ui::draw_back(input, 1.0);
    paint::outlined("Whisker Wood", 300.0, 74.0, 38.0, CREAM, INK, Align::Center);
    let (won, total) = flow::total_stars(save);
    ui::star(vec2(482.0, 60.0), 18.0, GOLD_TEXT, INK);
    paint::text(&format!("{won}/{total}"), 508.0, 70.0, 28.0, CREAM, Align::Left);
}

/// The open stone's card, sliding up from the bottom.
fn draw_card(assets: &Assets, state: &MapState, save: &SaveData, input: &MenuInput) {
    let Some((stop, open_for)) = state.card else { return };
    let slide = card_slide(open_for);
    let r = Rect::new(CARD.x, CARD.y + slide, CARD.w, CARD.h);
    // Solid behind, so the stones and ribbons don't show through the text
    paint::rounded_rect(r, 30.0, paint::rgb(0.10, 0.13, 0.11), paint::rgb(0.07, 0.09, 0.08));
    paint::panel(r, 30.0, 1.0);
    let close = close_button(slide);
    let (cx, cy) = (close.center().x, close.center().y);
    for (dx, dy) in [(1.0, 1.0), (1.0, -1.0)] {
        draw_line(cx - 14.0 * dx, cy - 14.0 * dy, cx + 14.0 * dx, cy + 14.0 * dy, 6.0, paint::fade(CREAM, 0.8));
    }
    match stop {
        Stop::Level(id) => {
            let def = id.def();
            let record = save.level(def.key);
            let region = def.region.info();
            paint::text(&format!("{} · {} OF 3", region.name.to_uppercase(), def.stage), 300.0, r.y + 50.0, 22.0, GOLD_TEXT, Align::Center);
            paint::outlined(def.title, 300.0, r.y + 104.0, paint::fit_size(def.title, 46.0, 420.0), CREAM, INK, Align::Center);
            // The critters who live there, the new one marked
            let critters: Vec<_> = ALL_THREATS.iter().copied().filter(|&c| in_roster(c, &def.spec().difficulty)).collect();
            let gap = 84.0;
            let x0 = 300.0 - (critters.len() as f32 - 1.0) * gap / 2.0;
            for (k, &critter) in critters.iter().enumerate() {
                let at = vec2(x0 + k as f32 * gap, r.y + 178.0);
                let (sprites, frame) = critter.icon(assets);
                story::fit_sprite_centred(sprites, frame, at, 60.0, 1.0, WHITE);
                if def.introduces == Some(critter) {
                    paint::outlined("NEW", at.x, at.y - 30.0, 20.0, GOLD_TEXT, INK, Align::Center);
                }
            }
            // The stars, won or still to win
            let labels = ["Top", "No bumps", &format!("Under {}", mmss(def.star_time))];
            let won = [record.stars.top, record.stars.no_bumps, record.stars.quick];
            for k in 0..3 {
                let at = vec2(150.0 + 150.0 * k as f32, r.y + 258.0);
                ui::star(at, 24.0, if won[k] { GOLD_TEXT } else { paint::fade(CREAM, 0.25) }, paint::fade(INK, 0.8));
                paint::text(labels[k], at.x, at.y + 46.0, 20.0, paint::fade(CREAM, 0.9), Align::Center);
            }
            let note = if record.cleared {
                match record.best_time {
                    Some(time) => format!("Best {} · {}", thousands(record.best_score), mmss(time)),
                    None => format!("Best {}", thousands(record.best_score)),
                }
            } else {
                match def.goal {
                    Goal::Clue(_) => format!("Look for a clue to {}", def.friend().name()),
                    Goal::Rescue => format!("Rescue {}!", def.friend().name()),
                }
            };
            paint::text(&note, 300.0, r.y + 344.0, 24.0, CREAM, Align::Center);
        }
        Stop::Endless => {
            paint::text("BONUS", 300.0, r.y + 52.0, 22.0, GOLD_TEXT, Align::Center);
            paint::outlined("Endless Climb", 300.0, r.y + 106.0, 46.0, CREAM, INK, Align::Center);
            paint::text("How high can Kitty go? Each climb is wilder, and", 300.0, r.y + 170.0, 22.0, CREAM, Align::Center);
            paint::text("her hearts refill; the run ends if they run out.", 300.0, r.y + 200.0, 22.0, CREAM, Align::Center);
            paint::text(&format!("Best {}", thousands(save.endless.best_score)), 300.0, r.y + 262.0, 30.0, GOLD_TEXT, Align::Center);
            if save.endless.best_climb > 0 {
                paint::text(&format!("Furthest: climb {}", save.endless.best_climb), 300.0, r.y + 300.0, 24.0, CREAM, Align::Center);
            }
        }
    }
    ui::draw_button(input, play_button(slide), "Play", Style::Primary, true, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stones_climb_the_map_in_their_regions_and_never_crowd() {
        let stones: Vec<Vec2> = Stop::all().map(stone).collect();
        assert_eq!(stones.len(), 16);
        for pair in stones.windows(2) {
            assert!(pair[1].y < pair[0].y, "each stone is higher up the map than the last");
        }
        for (i, a) in stones.iter().enumerate() {
            assert!(a.x - STONE_R >= 60.0 && a.x + STONE_R <= VIEW_W - 60.0, "{a} has room around it");
            for b in &stones[i + 1..] {
                assert!(a.distance(*b) >= 2.0 * STONE_R + 30.0, "{a} and {b} too close");
            }
        }
        for id in LevelId::all() {
            assert!(band(id.def().region).contains(stone(Stop::Level(id))));
        }
        assert!(stone(Stop::Endless).y < TOP_H);
    }

    #[test]
    fn taps_find_their_stone() {
        for stop in Stop::all() {
            assert_eq!(stop_at(stone(stop)), Some(stop));
            assert_eq!(stop_at(stone(stop) + vec2(STONE_R, 0.0)), Some(stop));
        }
        assert_eq!(stop_at(vec2(5.0, 5.0)), None);
    }

    #[test]
    fn scrolling_stays_on_the_map_and_shows_the_stone() {
        for stop in Stop::all() {
            let s = scroll_for(stop);
            assert!((0.0..=max_scroll()).contains(&s));
            let y = stone(stop).y - s;
            assert!(y > BAR.h + STONE_R && y < VIEW_H - STONE_R, "{stop:?} on screen at {y}");
            let above = stone(stop).y - scroll_above_card(stop);
            assert!(above < CARD.y - STONE_R || scroll_above_card(stop) == 0.0, "{stop:?} above its card");
        }
        assert_eq!(scroll_for(Stop::Level(LevelId(0))), max_scroll().min(stone(Stop::Level(LevelId(0))).y - VIEW_H * 0.55));
    }

    #[test]
    fn a_fresh_map_shows_the_first_stone_and_only_it_opens() {
        let save = SaveData::default();
        let mut map = MapState::new(&save);
        assert_eq!(kitty_stop(&save), Stop::Level(LevelId(0)));
        let at = |stop: Stop, map: &MapState| stone(stop) - vec2(0.0, map.scroll);
        let tap = |p: Vec2| MenuInput { tap: Some(crate::input::menu::Tap { down: p, up: p }), ..MenuInput::default() };
        // The second stone is locked: a toast, no card
        let second = Stop::Level(LevelId(1));
        map.update(&tap(at(second, &map)), &save, 0.016);
        assert!(map.card.is_none() && map.toast.is_some());
        // The first opens its card, and Play plays it
        map.update(&tap(at(Stop::Level(LevelId(0)), &map)), &save, 0.016);
        assert_eq!(map.card.map(|c| c.0), Some(Stop::Level(LevelId(0))));
        for _ in 0..60 {
            map.update(&MenuInput::default(), &save, 0.016);
        }
        let play = play_button(0.0).center();
        assert_eq!(map.update(&tap(play), &save, 0.016), Some(MapAction::Play(LevelId(0))));
    }

    #[test]
    fn a_double_tap_on_a_stone_never_presses_play() {
        let save = SaveData::default();
        let mut map = MapState::new(&save);
        let first = Stop::Level(LevelId(0));
        let on_stone = stone(first) - vec2(0.0, map.scroll);
        let play = play_button(0.0).center();
        let tap = |down: Vec2, up: Vec2| MenuInput { tap: Some(crate::input::menu::Tap { down, up }), ..MenuInput::default() };
        map.update(&tap(on_stone, on_stone), &save, 0.016);
        assert!(map.card.is_some());
        // The second tap goes down while the card slides in, is held past it settling, and lifts
        // on Play
        let held = MenuInput { held: Some(play), ..MenuInput::default() };
        for _ in 0..30 {
            map.update(&held, &save, 0.016);
        }
        assert_eq!(map.update(&tap(play, play), &save, 0.016), None, "a finger down before the card settled");
        // A fresh tap once it's in place plays
        map.update(&MenuInput::default(), &save, 0.016);
        assert_eq!(map.update(&tap(play, play), &save, 0.016), Some(MapAction::Play(LevelId(0))));
    }

    #[test]
    fn wheel_turns_scroll_a_step_and_never_more_than_half_a_screen() {
        assert!(wheel_distance(1.0e6) <= VIEW_H * 0.5 && wheel_distance(-1.0e6) >= -VIEW_H * 0.5);
        let notch = if cfg!(windows) { 120.0 } else if cfg!(target_os = "macos") { 10.0 } else { 1.0 };
        assert!((20.0..=WHEEL_STEP * 1.01).contains(&wheel_distance(notch)), "{}", wheel_distance(notch));
        assert!(wheel_distance(notch * 0.5) < wheel_distance(notch), "smaller turns scroll less");
    }

    #[test]
    fn keys_open_and_play_and_back_goes_back() {
        let save = SaveData::default();
        let mut map = MapState::new(&save);
        let key = |nav: Nav| MenuInput { nav: Some(nav), ..MenuInput::default() };
        assert_eq!(map.update(&key(Nav::Activate), &save, 0.016), None, "Enter opens the card, but doesn't play yet");
        assert!(map.card.is_some());
        assert_eq!(map.update(&key(Nav::Up), &save, 0.016), None, "the next level is locked");
        assert_eq!(map.focus, Stop::Level(LevelId(0)));
        assert_eq!(map.update(&key(Nav::Activate), &save, 0.016), None, "not until the card has slid in");
        for _ in 0..20 {
            map.update(&MenuInput::default(), &save, 0.016);
        }
        assert_eq!(map.update(&key(Nav::Activate), &save, 0.016), Some(MapAction::Play(LevelId(0))));
        map.update(&key(Nav::Back), &save, 0.016);
        assert!(map.card.is_none());
        assert_eq!(map.update(&key(Nav::Back), &save, 0.016), Some(MapAction::Back));
    }

    #[test]
    fn after_a_clear_kitty_hops_to_the_new_stone_and_its_card_opens() {
        let mut save = SaveData::default();
        crate::flow::record_clear(&mut save, LevelId(0), &crate::flow::AttemptResult::default(), 100);
        let mut map = MapState::new(&save);
        map.enter(&save, Some((Stop::Level(LevelId(0)), Stop::Level(LevelId(1)))));
        let tap = MenuInput { tap: Some(crate::input::menu::Tap { down: BACK.center(), up: BACK.center() }), ..MenuInput::default() };
        assert_eq!(map.update(&tap, &save, 0.016), None, "taps wait while she hops");
        for _ in 0..90 {
            map.update(&MenuInput::default(), &save, 0.016);
        }
        assert_eq!(map.card.map(|c| c.0), Some(Stop::Level(LevelId(1))));
    }

    #[test]
    fn a_drag_scrolls_and_a_fling_glides_to_a_stop_on_the_map() {
        let save = SaveData::default();
        let mut map = MapState::new(&save);
        let start = map.scroll;
        map.update(&MenuInput { drag_dy: 300.0, ..MenuInput::default() }, &save, 0.016);
        assert_eq!(map.scroll, start - 300.0, "dragging down shows what's above");
        map.update(&MenuInput { fling: Some(3000.0), ..MenuInput::default() }, &save, 0.016);
        let mut last = map.scroll;
        for _ in 0..600 {
            map.update(&MenuInput::default(), &save, 0.016);
            assert!(map.scroll <= last && map.scroll >= 0.0);
            last = map.scroll;
        }
        assert!(map.vel.abs() < 1.0);
    }
}
