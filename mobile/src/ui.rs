//! Buttons for the menus and cards, drawn and pressed in one call each frame: by a tap, or by the
//! keyboard's focus. Their places are fixed rectangles on the 600x1200 playfield, so the tests can
//! check they never overlap and keep clear of the pause button.

use macroquad::prelude::*;

use crate::input::menu::{MenuInput, Nav};
use crate::story::paint::{self, Align};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    /// Gold: the thing to do next.
    Primary,
    /// Cream: the other choice.
    Secondary,
}

/// Tall enough for a thumb (about 12 mm on a phone).
pub const BUTTON_H: f32 = 100.0;
const RADIUS: f32 = 26.0;
const LABEL_SIZE: f32 = 40.0;

/// Draw a button and say whether it was pressed (see `pressed`).
pub fn button(input: &MenuInput, r: Rect, label: &str, style: Style, focused: bool, alpha: f32) -> bool {
    draw_button(input, r, label, style, focused, alpha);
    pressed(input, r, focused)
}

/// Whether a button at `r` was pressed: tapped (the finger went down and came up on it), or
/// Enter pressed while it has the keyboard's focus.
pub fn pressed(input: &MenuInput, r: Rect, focused: bool) -> bool {
    input.tapped(r) || (focused && input.nav == Some(Nav::Activate))
}

/// Draw a button: it sinks while a finger is on it, brightens under the mouse, and is outlined
/// when it has the keyboard's focus.
pub fn draw_button(input: &MenuInput, r: Rect, label: &str, style: Style, focused: bool, alpha: f32) {
    let held = input.held.is_some_and(|p| r.contains(p));
    let hover = input.hover.is_some_and(|p| r.contains(p));
    let at = if held { Rect::new(r.x, r.y + 4.0, r.w, r.h) } else { r };
    let (top, bottom) = match style {
        Style::Primary => (paint::rgb(1.0, 0.84, 0.36), paint::rgb(0.95, 0.58, 0.18)),
        Style::Secondary => (paint::rgb(1.0, 0.96, 0.88), paint::rgb(0.86, 0.78, 0.64)),
    };
    let light = if held { 0.88 } else if hover { 1.08 } else { 1.0 };
    let shade = |c: Color| paint::fade(Color::new((c.r * light).min(1.0), (c.g * light).min(1.0), (c.b * light).min(1.0), 1.0), alpha);
    paint::drop_shadow(at, RADIUS, alpha * if held { 0.4 } else { 1.0 });
    paint::rounded_rect(at, RADIUS, shade(top), shade(bottom));
    paint::rounded_outline(at, RADIUS, 3.0, paint::fade(paint::INK, 0.45 * alpha));
    if focused {
        paint::rounded_outline(paint::grow(at, 6.0), RADIUS + 6.0, 4.0, paint::fade(WHITE, 0.85 * alpha));
    }
    let size = paint::fit_size(label, LABEL_SIZE, r.w - 30.0);
    paint::text(label, at.center().x, at.center().y + size * 0.35, size, paint::fade(paint::INK, alpha), Align::Center);
}

/// A five-pointed star at `centre`, `radius` to its points, filled and outlined.
pub fn star(centre: Vec2, radius: f32, fill: Color, outline: Color) {
    let point = |i: usize, r: f32| {
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        centre + Vec2::from_angle(angle) * r
    };
    let rim: Vec<Vec2> = (0..10).map(|i| point(i, if i % 2 == 0 { radius } else { radius * 0.45 })).collect();
    if outline.a > 0.0 {
        let grown: Vec<Vec2> = rim.iter().map(|&p| centre + (p - centre) * (1.0 + 3.0 / radius)).collect();
        for i in 0..10 {
            draw_triangle(centre, grown[i], grown[(i + 1) % 10], outline);
        }
    }
    for i in 0..10 {
        draw_triangle(centre, rim[i], rim[(i + 1) % 10], fill);
    }
}

/// The keyboard's focus over `n` buttons after `nav`: back with Left or Up, on with Right or
/// Down (or Tab), wrapping round.
pub fn focus_step(nav: Option<Nav>, focus: usize, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    match nav {
        Some(Nav::Left | Nav::Up) => (focus + n - 1) % n,
        Some(Nav::Right | Nav::Down) => (focus + 1) % n,
        _ => focus.min(n - 1),
    }
}

/// Two buttons side by side at `y`, across the card.
fn pair(y: f32) -> [Rect; 2] {
    [Rect::new(60.0, y, 230.0, BUTTON_H), Rect::new(310.0, y, 230.0, BUTTON_H)]
}

/// Paused: Resume across the width, then Retry and Map side by side under it.
pub fn pause_buttons() -> [Rect; 3] {
    let [retry, map] = pair(980.0);
    [Rect::new(60.0, 860.0, 480.0, BUTTON_H), retry, map]
}

/// Out of hearts: Retry, Map.
pub fn try_again_buttons() -> [Rect; 2] {
    pair(720.0)
}

/// A level cleared: Retry, then Continue.
pub fn clear_buttons() -> [Rect; 2] {
    pair(1080.0)
}

/// An endless run over: Retry, Map.
pub fn endless_over_buttons() -> [Rect; 2] {
    pair(800.0)
}
