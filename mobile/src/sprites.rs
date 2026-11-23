//! Frames cut by tools/slice_sprites.py: each comes with an anchor (the pixel pinned to its
//! character's position) and, for enemies, a damage mask of the pixels that can hurt Kitty.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use macroquad::prelude::*;
use serde::Deserialize;

use crate::parallax::Scenery;

#[derive(Deserialize)]
struct Manifest {
    animations: HashMap<String, Vec<String>>,
    anchors: HashMap<String, [f32; 2]>,
    #[serde(default)]
    hits: HashMap<String, String>,
}

/// Which pixels of a frame can hurt, as drawn facing right.
pub struct HitMask {
    w: usize,
    h: usize,
    bits: Vec<bool>,
}

impl HitMask {
    fn from_image(image: &Image) -> Self {
        let (w, h) = (image.width as usize, image.height as usize);
        let bits = image.bytes.chunks_exact(4).map(|px| px[3] > 127).collect();
        Self { w, h, bits }
    }

    /// Any damaging pixel inside `area`, given in frame pixels (already mirrored if needed).
    fn any_in(&self, area: Rect, flipped: bool) -> bool {
        let x0 = area.x.max(0.0) as usize;
        let y0 = area.y.max(0.0) as usize;
        let x1 = (area.right().min(self.w as f32)).max(0.0) as usize;
        let y1 = (area.bottom().min(self.h as f32)).max(0.0) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let sx = if flipped { self.w - 1 - x } else { x };
                if self.bits[y * self.w + sx] {
                    return true;
                }
            }
        }
        false
    }
}

pub struct Frame {
    pub texture: Texture2D,
    pub anchor: Vec2,
    pub hit: Option<HitMask>,
}

impl Frame {
    pub fn size(&self) -> Vec2 {
        vec2(self.texture.width(), self.texture.height())
    }

    /// The anchor as it sits on screen for this facing (frames face right).
    pub fn anchor_for(&self, facing: f32) -> Vec2 {
        if facing < 0.0 {
            vec2(self.texture.width() - 1.0 - self.anchor.x, self.anchor.y)
        } else {
            self.anchor
        }
    }

    /// Top-left corner when drawn with its anchor at `at`.
    pub fn top_left(&self, at: Vec2, facing: f32) -> Vec2 {
        at - self.anchor_for(facing)
    }
}

/// One character's frames, keyed by name ("walk_0").
pub struct Sprites {
    frames: HashMap<String, Frame>,
}

impl Sprites {
    pub async fn load(dir: &str) -> Self {
        let text = load_string(&format!("{dir}/anchors.json")).await.expect("anchors.json");
        let manifest: Manifest = serde_json::from_str(&text).expect("valid anchors.json");
        let files: Vec<&String> = manifest.animations.values().flatten().collect();
        let loads = files.iter().map(|file| load_frame(dir, file, manifest.anchors[*file], manifest.hits.get(*file)));
        let frames = join_all(loads.collect()).await;
        let names = files.iter().map(|file| file.trim_end_matches(".png").to_string());
        Self { frames: names.zip(frames).collect() }
    }

    /// Like `load`, but art that isn't there yet (no anchors.json) leaves the set empty.
    pub async fn load_optional(dir: &str) -> Self {
        match load_string(&format!("{dir}/anchors.json")).await {
            Ok(_) => Self::load(dir).await,
            Err(_) => Self { frames: HashMap::new() },
        }
    }

    pub fn get(&self, name: &str) -> &Frame {
        self.frames.get(name).unwrap_or_else(|| panic!("missing sprite frame {name}"))
    }

    pub fn has(&self, name: &str) -> bool {
        self.frames.contains_key(name)
    }

    /// Draw `name` with its anchor at `at`, mirrored when facing left.
    pub fn draw(&self, name: &str, at: Vec2, facing: f32, tint: Color) {
        let frame = self.get(name);
        let top_left = frame.top_left(at, facing);
        draw_texture_ex(&frame.texture, top_left.x, top_left.y, tint,
                        DrawTextureParams { flip_x: facing < 0.0, ..Default::default() });
    }

    /// Draw scaled and rotated (radians) about the anchor.
    pub fn draw_ex(&self, name: &str, at: Vec2, facing: f32, scale: f32, rotation: f32, tint: Color) {
        self.draw_stretched(name, at, facing, Vec2::splat(scale), rotation, tint);
    }

    /// Draw scaled by different amounts across and down (to squash), rotated (radians) about the
    /// anchor.
    pub fn draw_stretched(&self, name: &str, at: Vec2, facing: f32, scale: Vec2, rotation: f32, tint: Color) {
        let frame = self.get(name);
        let anchor = frame.anchor_for(facing) * scale;
        let size = frame.size() * scale;
        draw_texture_ex(&frame.texture, at.x - anchor.x, at.y - anchor.y, tint, DrawTextureParams {
            dest_size: Some(size),
            rotation,
            pivot: Some(at),
            flip_x: facing < 0.0,
            ..Default::default()
        });
    }

    /// True if frame `name`, drawn with its anchor at `at`, has damaging pixels inside `body`.
    pub fn strikes(&self, name: &str, at: Vec2, facing: f32, body: Rect) -> bool {
        let frame = self.get(name);
        let Some(mask) = &frame.hit else { return false };
        let top_left = frame.top_left(at, facing);
        let local = Rect::new(body.x - top_left.x, body.y - top_left.y, body.w, body.h);
        mask.any_in(local, facing < 0.0)
    }

    /// Bounding box of frame `name` drawn with its anchor at `at`.
    pub fn bounds(&self, name: &str, at: Vec2, facing: f32) -> Rect {
        let frame = self.get(name);
        let top_left = frame.top_left(at, facing);
        Rect::new(top_left.x, top_left.y, frame.texture.width(), frame.texture.height())
    }
}

/// A named run of frames. Looping clips repeat; others hold their last frame.
pub struct Clip {
    pub frames: &'static [&'static str],
    pub fps: f32,
    pub looped: bool,
}

impl Clip {
    pub fn duration(&self) -> f32 {
        self.frames.len() as f32 / self.fps
    }
}

/// Plays one clip at a time.
#[derive(Clone, Copy)]
pub struct Animator {
    pub clip: &'static Clip,
    pub time: f32,
}

impl Animator {
    pub fn new(clip: &'static Clip) -> Self {
        Self { clip, time: 0.0 }
    }

    pub fn play(&mut self, clip: &'static Clip) {
        if !std::ptr::eq(self.clip, clip) {
            self.clip = clip;
            self.time = 0.0;
        }
    }

    pub fn restart(&mut self, clip: &'static Clip) {
        self.clip = clip;
        self.time = 0.0;
    }

    pub fn is(&self, clip: &'static Clip) -> bool {
        std::ptr::eq(self.clip, clip)
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
    }

    pub fn finished(&self) -> bool {
        !self.clip.looped && self.time >= self.clip.duration()
    }

    pub fn frame(&self) -> &'static str {
        let i = (self.time * self.clip.fps) as usize;
        let n = self.clip.frames.len();
        self.clip.frames[if self.clip.looped { i % n } else { i.min(n - 1) }]
    }
}

async fn load_frame(dir: &str, file: &str, [ax, ay]: [f32; 2], hit_file: Option<&String>) -> Frame {
    let texture = load_texture(&format!("{dir}/{file}")).await.expect("frame image");
    texture.set_filter(FilterMode::Linear);
    let hit = match hit_file {
        Some(hit_file) => {
            let image = load_image(&format!("{dir}/{hit_file}")).await.expect("hit mask");
            Some(HitMask::from_image(&image))
        }
        None => None,
    };
    Frame { texture, anchor: vec2(ax, ay), hit }
}

/// Run the futures together. On the web each file load waits at least a frame, so loading
/// a few hundred frames one after another would take seconds; this fetches them all at once.
async fn join_all<T, F: Future<Output = T>>(futures: Vec<F>) -> Vec<T> {
    let mut futures: Vec<Pin<Box<F>>> = futures.into_iter().map(Box::pin).collect();
    let mut results: Vec<Option<T>> = futures.iter().map(|_| None).collect();
    std::future::poll_fn(move |cx| {
        let mut pending = false;
        for (future, result) in futures.iter_mut().zip(results.iter_mut()) {
            if result.is_none() {
                match future.as_mut().poll(cx) {
                    Poll::Ready(value) => *result = Some(value),
                    Poll::Pending => pending = true,
                }
            }
        }
        if pending {
            Poll::Pending
        } else {
            Poll::Ready(results.iter_mut().map(|r| r.take().unwrap()).collect())
        }
    })
    .await
}

/// Everything loaded at startup.
pub struct Assets {
    pub kitty: Sprites,
    pub dog: Sprites,
    pub eagle: Sprites,
    pub squirrel: Sprites,
    pub snake: Sprites,
    pub crow: Sprites,
    pub friends: Sprites,
    pub items: Sprites,
    pub foliage: Sprites,
    /// The regions' helper critters and the clues to the friends, once their art is in (until
    /// then, empty: see `Sprites::has`).
    pub helpers: Sprites,
    pub clues: Sprites,
    /// The painted forest used behind the menu screens.
    pub background: Texture2D,
    /// The sky and parallax layers behind the trees in play.
    pub scenery: Scenery,
}

impl Assets {
    /// Load every character behind a progress bar.
    pub async fn load() -> Self {
        let mut progress = Progress { done: 0 };
        let kitty = progress.load("kitty", "Waking up Kitty...").await;
        let foliage = progress.load("foliage", "Planting the trees...").await;
        let items = progress.load("items", "Hiding the fish treats...").await;
        let friends = progress.load("friends", "Finding Kitty's friends...").await;
        let dog = progress.load("dog", "Letting the dogs out...").await;
        let squirrel = progress.load("squirrel", "Waking the squirrels...").await;
        let snake = progress.load("snake", "Coiling the snakes...").await;
        let crow = progress.load("crow", "Gathering the crows...").await;
        let eagle = progress.load("eagle", "Calling the eagles...").await;
        let helpers = progress.load_optional("helpers", "Asking around the woods...").await;
        let clues = progress.load_optional("clues", "Following the clues...").await;
        let fraction = progress.done as f32 / LOAD_STAGES as f32;
        draw_loading(fraction, "Painting the sky...");
        next_frame().await;
        let background = while_loading(load_texture("background.png"), fraction, "Painting the sky...").await.expect("background.png");
        background.set_filter(FilterMode::Linear);
        let scenery = while_loading(Scenery::load(), fraction, "Painting the sky...").await;
        draw_loading(1.0, "Ready!");
        next_frame().await;
        Self { kitty, dog, eagle, squirrel, snake, crow, friends, items, foliage, helpers, clues, background, scenery }
    }
}

/// How many stages Assets::load works through: nine characters, the helpers and clues, then the
/// scenery.
const LOAD_STAGES: usize = 12;

struct Progress {
    done: usize,
}

impl Progress {
    async fn load(&mut self, dir: &str, label: &str) -> Sprites {
        let fraction = self.done as f32 / LOAD_STAGES as f32;
        // Show the bar for at least one frame, even where loading never has to wait
        draw_loading(fraction, label);
        next_frame().await;
        let sprites = while_loading(Sprites::load(dir), fraction, label).await;
        self.done += 1;
        sprites
    }

    async fn load_optional(&mut self, dir: &str, label: &str) -> Sprites {
        let fraction = self.done as f32 / LOAD_STAGES as f32;
        draw_loading(fraction, label);
        next_frame().await;
        let sprites = while_loading(Sprites::load_optional(dir), fraction, label).await;
        self.done += 1;
        sprites
    }
}

/// Await `future`, redrawing the loading screen on every frame it keeps the game waiting:
/// each frame starts cleared, so drawing it once would leave the screen blank while the web
/// build downloads its files.
async fn while_loading<T>(future: impl Future<Output = T>, fraction: f32, label: &str) -> T {
    let mut future = Box::pin(future);
    std::future::poll_fn(move |cx| {
        let poll = future.as_mut().poll(cx);
        if poll.is_pending() {
            draw_loading(fraction, label);
        }
        poll
    })
    .await
}

fn draw_loading(fraction: f32, label: &str) {
    clear_background(Color::new(0.11, 0.11, 0.14, 1.0));
    let (w, h) = (screen_width(), screen_height());
    let bar_w = w * 0.6;
    let x = (w - bar_w) / 2.0;
    let y = h * 0.55;
    let size = (h * 0.035).max(18.0);
    let title = "Kitty Adventure";
    let tw = measure_text(title, None, (size * 1.6) as u16, 1.0).width;
    draw_text(title, (w - tw) / 2.0, h * 0.45, size * 1.6, Color::new(1.0, 0.75, 0.35, 1.0));
    draw_rectangle(x, y, bar_w, size * 0.6, Color::new(0.25, 0.25, 0.3, 1.0));
    draw_rectangle(x, y, bar_w * fraction.clamp(0.0, 1.0), size * 0.6, Color::new(1.0, 0.6, 0.2, 1.0));
    let lw = measure_text(label, None, size as u16, 1.0).width;
    draw_text(label, (w - lw) / 2.0, y + size * 1.8, size, LIGHTGRAY);
}
