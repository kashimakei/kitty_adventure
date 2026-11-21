//! Keyboard plus on-screen touch buttons. Holding a button walks; jumping and other one-off
//! actions use tap events, which are recorded between frames, so a quick tap is never missed.

use macroquad::prelude::*;

use crate::camera::View;

#[derive(Default, Clone, Copy)]
pub struct Controls {
    pub left: bool,
    pub right: bool,
    pub jump: bool,      // tapped this frame
    pub jump_held: bool, // still held: a longer press jumps higher
    pub down: bool,      // tapped this frame: let go of a branch
    pub pause: bool,     // tapped this frame
    pub any_tap: bool,   // any tap or key this frame, for "tap to continue" screens
}

pub struct Button {
    pub centre: Vec2,
    pub radius: f32,
    pub label: &'static str,
}

impl Button {
    fn near(&self, p: Vec2) -> bool {
        p.distance(self.centre) < self.radius * 1.25
    }
}

/// On-screen buttons, in playfield coordinates.
pub const LEFT: Button = Button { centre: Vec2::from_array([78.0, 1120.0]), radius: 54.0, label: "<" };
pub const RIGHT: Button = Button { centre: Vec2::from_array([206.0, 1120.0]), radius: 54.0, label: ">" };
pub const DOWN: Button = Button { centre: Vec2::from_array([408.0, 1146.0]), radius: 38.0, label: "v" };
pub const JUMP: Button = Button { centre: Vec2::from_array([520.0, 1104.0]), radius: 62.0, label: "^" };
pub const PAUSE: Button = Button { centre: Vec2::from_array([572.0, 92.0]), radius: 24.0, label: "II" };

/// Read keyboard and touches. `buttons` is false on menu screens, where any tap continues.
pub fn read(view: &View) -> Controls {
    let mouse = view.to_playfield(mouse_position().into());
    let touches = touches();
    let held = |b: &Button| {
        touches.iter().any(|t| !matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled)
            && b.near(view.to_playfield(t.position)))
            || (is_mouse_button_down(MouseButton::Left) && b.near(mouse))
    };
    let tapped = |b: &Button| {
        touches.iter().any(|t| t.phase == TouchPhase::Started && b.near(view.to_playfield(t.position)))
            || (is_mouse_button_pressed(MouseButton::Left) && b.near(mouse))
    };
    let any_tap = touches.iter().any(|t| t.phase == TouchPhase::Started)
        || is_mouse_button_pressed(MouseButton::Left)
        || get_last_key_pressed().is_some();
    Controls {
        left: held(&LEFT) || is_key_down(KeyCode::Left) || is_key_down(KeyCode::A),
        right: held(&RIGHT) || is_key_down(KeyCode::Right) || is_key_down(KeyCode::D),
        jump: tapped(&JUMP) || is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Up)
            || is_key_pressed(KeyCode::W),
        jump_held: held(&JUMP) || is_key_down(KeyCode::Space) || is_key_down(KeyCode::Up)
            || is_key_down(KeyCode::W),
        down: tapped(&DOWN) || is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S),
        pause: tapped(&PAUSE) || is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::P),
        any_tap,
    }
}

/// Draw the touch buttons (in the screen camera), lighting the ones held.
pub fn draw_buttons(controls: &Controls) {
    for (button, lit) in [(&LEFT, controls.left), (&RIGHT, controls.right), (&DOWN, false), (&JUMP, controls.jump_held)] {
        let alpha = if lit { 0.55 } else { 0.28 };
        draw_circle(button.centre.x, button.centre.y, button.radius, Color::new(1.0, 1.0, 1.0, alpha));
        draw_circle_lines(button.centre.x, button.centre.y, button.radius, 3.0, Color::new(0.15, 0.15, 0.15, 0.55));
        let size = button.radius * 0.8;
        let dims = measure_text(button.label, None, size as u16, 1.0);
        draw_text(button.label, button.centre.x - dims.width / 2.0, button.centre.y + dims.height / 2.0,
                  size, Color::new(0.1, 0.1, 0.1, 0.75));
    }
    draw_circle(PAUSE.centre.x, PAUSE.centre.y, PAUSE.radius, Color::new(1.0, 1.0, 1.0, 0.35));
    for dx in [-6.0, 6.0] {
        draw_rectangle(PAUSE.centre.x + dx - 3.0, PAUSE.centre.y - 9.0, 6.0, 18.0, Color::new(0.1, 0.1, 0.1, 0.7));
    }
}
