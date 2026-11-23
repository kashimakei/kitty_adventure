//! Tree trunks: scenery Kitty passes straight through, with limbs growing out of them and a leafy
//! canopy on top.

use macroquad::prelude::*;

use crate::consts::*;
use crate::look::WoodColors;
use crate::rng::Rng;
use crate::sprites::Assets;

const TILE_H: u16 = 256;

/// One bark texture in `wood`'s colours, tiled up every trunk.
pub fn make_bark_texture(rng: &mut Rng, wood: &WoodColors) -> Texture2D {
    let (w, h) = (TRUNK_WIDTH as u16, TILE_H);
    let mut image = Image::gen_image_color(w, h, wood.trunk);
    let mut stroke = |x: u32, y0: u32, len: u32, width: u32, color: Color| {
        for y in y0..y0 + len {
            for dx in 0..width {
                let px = (x + dx).min(w as u32 - 1);
                image.set_pixel(px, y % h as u32, color);
            }
        }
    };
    // Vertical furrows; wrapping at the tile edges keeps the tiling seamless
    for _ in 0..(h / 10) {
        let x = rng.int(4, w as i32 - 6) as u32;
        let y = rng.int(0, h as i32 - 1) as u32;
        let len = rng.int(18, 60) as u32;
        let color = if rng.chance(0.6) { wood.outline } else { wood.fill };
        stroke(x, y, len, rng.int(1, 3) as u32, color);
    }
    for x in [0, 1, 2, 3, w as u32 - 4, w as u32 - 3, w as u32 - 2, w as u32 - 1] {
        stroke(x, 0, h as u32, 1, wood.outline);
    }
    // Shade it round: light catching the left of centre, falling off into shadow at the edges
    for x in 0..w as u32 {
        let t = x as f32 / (w - 1) as f32;
        let shade = 1.12 - ((t - 0.38) * 2.0).powi(2) * 0.5;
        for y in 0..h as u32 {
            let c = image.get_pixel(x, y);
            image.set_pixel(x, y, Color::new(c.r * shade, c.g * shade, c.b * shade, 1.0));
        }
    }
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Linear);
    texture
}

struct CanopyPuff {
    sprite: &'static str,
    offset: Vec2,
    scale: f32,
}

pub struct Tree {
    pub x: f32,
    pub top_y: f32,
    canopy: Vec<CanopyPuff>,
}

impl Tree {
    /// `autumn` is the share of the canopy that has turned.
    pub fn new(x: f32, top_y: f32, autumn: f32, rng: &mut Rng) -> Self {
        let mut canopy = Vec::new();
        for _ in 0..9 {
            let sprite = if rng.chance(autumn) { *rng.pick(&["autumn_4", "autumn_5", "autumn_3"]) } else { *rng.pick(&["green_4", "green_5", "green_3"]) };
            canopy.push(CanopyPuff {
                sprite,
                offset: vec2(rng.range(-80.0, 80.0), rng.range(-90.0, 10.0)),
                scale: rng.range(1.2, 1.8),
            });
        }
        canopy.sort_by(|a, b| a.offset.y.total_cmp(&b.offset.y));
        Self { x, top_y, canopy }
    }

    pub fn draw(&self, assets: &Assets, bark: &Texture2D, view: Rect, foliage_tint: Color) {
        let left = self.x - TRUNK_WIDTH / 2.0;
        if left > view.right() + 150.0 || left + TRUNK_WIDTH < view.left() - 150.0 {
            return;
        }
        let top = self.top_y.max(view.top() - TILE_H as f32);
        let bottom = (GROUND_Y + 20.0).min(view.bottom() + TILE_H as f32);
        // Tiles line up with world y so the bark doesn't slide as the camera moves
        let mut y = (top / TILE_H as f32).floor() * TILE_H as f32;
        while y < bottom {
            let y0 = y.max(self.top_y);
            let skip = y0 - y;
            let h = (TILE_H as f32 - skip).min(bottom - y0);
            if h > 0.0 {
                draw_texture_ex(bark, left, y0, WHITE, DrawTextureParams {
                    source: Some(Rect::new(0.0, skip, TRUNK_WIDTH, h)),
                    dest_size: Some(vec2(TRUNK_WIDTH, h)),
                    ..Default::default()
                });
            }
            y += TILE_H as f32;
        }
        if self.top_y > view.top() - 300.0 {
            for puff in &self.canopy {
                assets.foliage.draw_ex(puff.sprite, vec2(self.x, self.top_y) + puff.offset, 1.0, puff.scale, 0.0, foliage_tint);
            }
        }
    }
}
