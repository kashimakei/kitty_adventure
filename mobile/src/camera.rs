//! Fitting the 600x1200 playfield to the screen, and the camera that follows Kitty around a world
//! three screens wide and as tall as the trees.

use macroquad::prelude::*;

use crate::consts::*;

/// Scales the playfield to fit the window, centred, whatever its size or pixel density.
pub struct View {
    pub scale: f32,
}

impl View {
    pub fn fit() -> Self {
        Self { scale: (screen_width() / VIEW_W).min(screen_height() / VIEW_H) }
    }

    /// A camera that shows the playfield-sized area centred on `centre`, y pointing down.
    pub fn camera(&self, centre: Vec2) -> Camera2D {
        Camera2D {
            target: centre,
            zoom: vec2(2.0 * self.scale / screen_width(), 2.0 * self.scale / screen_height()),
            ..Default::default()
        }
    }

    /// The screen-space camera for HUD and menus: playfield coordinates 0..600 x 0..1200.
    pub fn screen_camera(&self) -> Camera2D {
        self.camera(vec2(VIEW_W / 2.0, VIEW_H / 2.0))
    }

    /// A window position (touch or mouse) in playfield coordinates.
    pub fn to_playfield(&self, screen: Vec2) -> Vec2 {
        let centre = vec2(screen_width(), screen_height()) / 2.0;
        (screen - centre) / self.scale + vec2(VIEW_W, VIEW_H) / 2.0
    }
}

/// Top-left of the visible playfield, in world coordinates.
pub struct Camera {
    pub x: f32,
    pub y: f32,
}

impl Camera {
    pub fn new(focus_x: f32) -> Self {
        let mut camera = Self { x: 0.0, y: Self::lowest() };
        camera.x = Self::clamp_x(focus_x - VIEW_W / 2.0);
        camera
    }

    /// The camera never shows more than a strip of ground below the world.
    fn lowest() -> f32 {
        GROUND_Y + CAMERA_GROUND_SHOWN - VIEW_H
    }

    fn clamp_x(x: f32) -> f32 {
        x.clamp(0.0, WORLD_W - VIEW_W)
    }

    /// Ease toward keeping `target` (Kitty's box) inside the dead zones.
    pub fn follow(&mut self, target: Rect) {
        let margin_x = VIEW_W * CAMERA_DEADZONE_X;
        let mut want_x = self.x;
        let cx = target.center().x;
        if cx < self.x + margin_x {
            want_x = cx - margin_x;
        } else if cx > self.x + VIEW_W - margin_x {
            want_x = cx - (VIEW_W - margin_x);
        }
        let mut want_y = self.y;
        if target.top() < self.y + VIEW_H * CAMERA_TOP_MARGIN {
            want_y = target.top() - VIEW_H * CAMERA_TOP_MARGIN;
        } else if target.bottom() > self.y + VIEW_H * CAMERA_BOTTOM_MARGIN {
            want_y = target.bottom() - VIEW_H * CAMERA_BOTTOM_MARGIN;
        }
        want_x = Self::clamp_x(want_x);
        want_y = want_y.min(Self::lowest());
        self.x += (want_x - self.x) * CAMERA_FOLLOW * 2.0;
        self.y += (want_y - self.y) * CAMERA_FOLLOW;
    }

    pub fn snap_to(&mut self, target: Rect) {
        self.x = Self::clamp_x(target.center().x - VIEW_W / 2.0);
        self.y = (target.bottom() - VIEW_H * CAMERA_BOTTOM_MARGIN).min(Self::lowest());
    }

    pub fn view_rect(&self) -> Rect {
        Rect::new(self.x, self.y, VIEW_W, VIEW_H)
    }

    pub fn world_camera(&self, view: &View) -> Camera2D {
        view.camera(vec2(self.x + VIEW_W / 2.0, self.y + VIEW_H / 2.0))
    }

    /// How far up the trees the camera has climbed, 0 at the ground.
    pub fn height(&self) -> f32 {
        (Self::lowest() - self.y).max(0.0)
    }
}
