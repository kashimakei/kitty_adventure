//! The scenery behind the trees, drawn in playfield coordinates before the world: a sky that
//! deepens as Kitty climbs, clouds drifting across it, then misty mountains, rolling hills and a
//! nearer forest (painted layers keyed out by tools/key_layers.py). The farther a layer, the less
//! it moves with the camera, so climbing leaves the forest below while the mountains barely sink.
//!
//! Layers only ever slide down the screen as the camera rises, and each is filled below with the
//! colour along its bottom edge, so whatever the height there's never a gap under the scenery.

use macroquad::prelude::*;

use crate::consts::*;
use crate::look::{tinted, Look};

/// Where a layer sits and how it moves.
struct Placement {
    /// Drawn width in playfield px (wide enough to cover the view across the whole world).
    width: f32,
    /// Screen y of its top edge with the camera on the ground.
    top: f32,
    /// How much it follows the camera, sideways and up (1 would move with the world).
    follow_x: f32,
    follow_y: f32,
}

const FAR: Placement = Placement { width: 1100.0, top: 330.0, follow_x: 0.06, follow_y: 0.08 };
const MID: Placement = Placement { width: 1250.0, top: 520.0, follow_x: 0.18, follow_y: 0.22 };
const NEAR: Placement = Placement { width: 1450.0, top: 640.0, follow_x: 0.38, follow_y: 0.45 };

/// Clouds sit between the sky and the mountains, at a range of depths.
const CLOUDS: usize = 14;
const CLOUD_FOLLOW_X: (f32, f32) = (0.04, 0.12);
const CLOUD_FOLLOW_Y: (f32, f32) = (0.12, 0.22);
/// Screen y with the camera on the ground: most start above the view and come down into it
/// as Kitty climbs.
const CLOUD_TOP: (f32, f32) = (-800.0, 260.0);
const CLOUD_SCALE: (f32, f32) = (0.22, 0.5);
const CLOUD_DRIFT: (f32, f32) = (4.0, 12.0); // px/s
/// Clouds wrap around a strip this much wider than the view.
const CLOUD_WRAP_MARGIN: f32 = 360.0;

pub struct Layer {
    texture: Texture2D,
    /// The average colour along its bottom edge, filled in below it.
    fill: Color,
}

impl Layer {
    async fn load(path: &str) -> Self {
        let image = load_image(path).await.unwrap_or_else(|e| panic!("{path}: {e}"));
        let bottom = image.height() as u32 - 1;
        let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
        for x in 0..image.width() as u32 {
            let c = image.get_pixel(x, bottom);
            (r, g, b) = (r + c.r, g + c.g, b + c.b);
        }
        let n = image.width() as f32;
        let texture = Texture2D::from_image(&image);
        texture.set_filter(FilterMode::Linear);
        Self { texture, fill: Color::new(r / n, g / n, b / n, 1.0) }
    }

    fn draw(&self, placement: &Placement, camera_x: f32, climbed: f32, tint: Color) {
        let w = placement.width;
        let h = w * self.texture.height() / self.texture.width();
        // Centre its spare width over the camera's travel so both world edges stay covered
        let travel = (WORLD_W - VIEW_W) * placement.follow_x;
        let x = (VIEW_W + travel - w) / 2.0 - camera_x * placement.follow_x;
        let y = placement.top + climbed * placement.follow_y;
        if y >= VIEW_H {
            return;
        }
        draw_texture_ex(&self.texture, x, y, tint, DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() });
        if y + h < VIEW_H {
            draw_rectangle(0.0, y + h - 1.0, VIEW_W, VIEW_H - (y + h) + 1.0, tinted(self.fill, tint));
        }
    }
}

struct Cloud {
    sprite: usize,
    x: f32,
    top: f32,
    scale: f32,
    follow_x: f32,
    follow_y: f32,
    drift: f32,
}

/// Everything the scenery needs, loaded with the other assets.
pub struct Scenery {
    far: Layer,
    mid: Layer,
    near: Layer,
    cloud_sprites: Vec<Texture2D>,
    clouds: Vec<Cloud>,
}

impl Scenery {
    pub async fn load() -> Self {
        let far = Layer::load("parallax/far.png").await;
        let mid = Layer::load("parallax/mid.png").await;
        let near = Layer::load("parallax/near.png").await;
        let mut cloud_sprites = Vec::new();
        // cloud_0.png, cloud_1.png, ... for as many as tools/key_layers.py cut out
        while let Ok(texture) = load_texture(&format!("parallax/cloud_{}.png", cloud_sprites.len())).await {
            texture.set_filter(FilterMode::Linear);
            cloud_sprites.push(texture);
        }
        // A fixed scatter (a simple hash per cloud), so the sky doesn't reshuffle between levels
        let pick = |i: usize, salt: u32, (lo, hi): (f32, f32)| {
            let h = (i as u32).wrapping_mul(2_654_435_761).wrapping_add(salt.wrapping_mul(40_503)) ^ 0x9E37_79B9;
            let h = h.wrapping_mul(0x85EB_CA6B) >> 8;
            lo + (hi - lo) * (h % 10_000) as f32 / 10_000.0
        };
        let mut clouds: Vec<Cloud> = (0..CLOUDS)
            .filter(|_| !cloud_sprites.is_empty())
            .map(|i| {
                // Depth: far clouds are small, slow and barely follow the camera
                let depth = pick(i, 1, (0.0, 1.0));
                Cloud {
                    sprite: i % cloud_sprites.len(),
                    x: pick(i, 2, (0.0, VIEW_W + CLOUD_WRAP_MARGIN)),
                    top: CLOUD_TOP.0 + (CLOUD_TOP.1 - CLOUD_TOP.0) * i as f32 / (CLOUDS - 1) as f32 + pick(i, 3, (-40.0, 40.0)),
                    scale: CLOUD_SCALE.0 + (CLOUD_SCALE.1 - CLOUD_SCALE.0) * depth,
                    follow_x: CLOUD_FOLLOW_X.0 + (CLOUD_FOLLOW_X.1 - CLOUD_FOLLOW_X.0) * depth,
                    follow_y: CLOUD_FOLLOW_Y.0 + (CLOUD_FOLLOW_Y.1 - CLOUD_FOLLOW_Y.0) * depth,
                    drift: CLOUD_DRIFT.0 + (CLOUD_DRIFT.1 - CLOUD_DRIFT.0) * depth,
                }
            })
            .collect();
        // Farthest first, so nearer clouds pass in front
        clouds.sort_by(|a, b| a.scale.total_cmp(&b.scale));
        Self { far, mid, near, cloud_sprites, clouds }
    }

    /// Draw it all with the screen camera set, in the colours of `look`. `camera_x` is the world
    /// camera's left edge, `climbed` how far it has risen above the ground, and `height` that as a
    /// fraction of the level's height (the sky deepens toward the top).
    pub fn draw(&self, camera_x: f32, climbed: f32, height: f32, time: f32, look: &Look) {
        let mix = |a: Color, b: Color| Color::new(a.r + (b.r - a.r) * height, a.g + (b.g - a.g) * height,
                                                  a.b + (b.b - a.b) * height, 1.0);
        let [top, horizon, top_high, horizon_high] = look.sky;
        gradient(Rect::new(0.0, 0.0, VIEW_W, VIEW_H), mix(top, top_high), mix(horizon, horizon_high));
        // Clouds take half the tint: they're in the sky, not the hazy distance
        let tint = look.scenery_tint;
        let cloud_tint = Color::new((1.0 + tint.r) / 2.0, (1.0 + tint.g) / 2.0, (1.0 + tint.b) / 2.0, 1.0);

        let span = VIEW_W + CLOUD_WRAP_MARGIN;
        for cloud in &self.clouds {
            let texture = &self.cloud_sprites[cloud.sprite];
            let size = vec2(texture.width(), texture.height()) * cloud.scale;
            let y = cloud.top + climbed * cloud.follow_y;
            if y > VIEW_H || y + size.y < 0.0 {
                continue;
            }
            let x = (cloud.x + cloud.drift * time - camera_x * cloud.follow_x).rem_euclid(span) - CLOUD_WRAP_MARGIN / 2.0 - size.x / 2.0;
            draw_texture_ex(texture, x, y, cloud_tint, DrawTextureParams { dest_size: Some(size), ..Default::default() });
        }

        self.far.draw(&FAR, camera_x, climbed, tint);
        self.mid.draw(&MID, camera_x, climbed, tint);
        self.near.draw(&NEAR, camera_x, climbed, tint);
    }
}

/// A rectangle shading from `top` to `bottom`.
fn gradient(r: Rect, top: Color, bottom: Color) {
    let vertex = |x: f32, y: f32, color: Color| Vertex::new(x, y, 0.0, 0.0, 0.0, color);
    let mesh = Mesh {
        vertices: vec![
            vertex(r.x, r.y, top),
            vertex(r.right(), r.y, top),
            vertex(r.right(), r.bottom(), bottom),
            vertex(r.x, r.bottom(), bottom),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    };
    draw_mesh(&mesh);
}
