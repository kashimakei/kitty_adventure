//! Feasibility spike: Kitty Adventure's bendy-branch physics and animated Kitty on macroquad, to
//! check that one Rust codebase can target desktop, web (wasm), Android and iOS.
//!
//! Covers what a full port needs to get right: the shared branch engine, sprite frames and
//! anchors from tools/slice_sprites.py, a fixed physics timestep, scaling the 600x1200 playfield
//! to any screen, and touch controls alongside the keyboard.
use std::collections::HashMap;

use branch_engine::branch::Branch;
use branch_engine::math::Vec2 as EngineVec2;
use macroquad::prelude::*;
use serde::Deserialize;

/// Logical playfield, the same as the pygame game; scaled to fit any screen with letterboxing.
const VIEW_W: f32 = 600.0;
const VIEW_H: f32 = 1200.0;
/// Physics runs at a fixed 60 Hz whatever the display's refresh rate.
const STEP: f32 = 1.0 / 60.0;

const GROUND_Y: f32 = 1060.0;
const GRAVITY: f32 = 2000.0;   // px/s^2
const JUMP_SPEED: f32 = 1150.0; // px/s
const WALK_SPEED: f32 = 300.0;  // px/s
const FOOT_REACH: f32 = 27.0;
const LANDING_TOLERANCE: f32 = 12.0;

// Branch tuning, as in constants.py
const KITTY_WEIGHT: f32 = 38000.0;
const BRANCH_SUB_STEPS: usize = 8;
const BRANCH_GRAVITY: f32 = 800.0;
const BRANCH_DAMPING: f32 = 0.995;
const LANDING_KICK: f32 = 0.06;
const JUMP_KICK: f32 = 1.2;

const TRUNK_WIDTH: f32 = 50.0;
const SKY: Color = Color::new(0.57, 0.86, 1.0, 1.0);
const BARK: Color = Color::new(0.47, 0.29, 0.15, 1.0);
const BARK_DARK: Color = Color::new(0.27, 0.16, 0.08, 1.0);
const BARK_LIGHT: Color = Color::new(0.63, 0.42, 0.24, 1.0);
const TRUNK: Color = Color::new(0.36, 0.23, 0.13, 1.0);
const LEAVES: [Color; 3] = [
    Color::new(0.80, 0.22, 0.0, 1.0),
    Color::new(0.93, 0.46, 0.0, 1.0),
    Color::new(0.85, 0.65, 0.13, 1.0),
];

// ------------------ Sprites -------------------

#[derive(Deserialize)]
struct Manifest {
    animations: HashMap<String, Vec<String>>,
    anchors: HashMap<String, [f32; 2]>,
}

struct Frame {
    texture: Texture2D,
    anchor: Vec2,
}

/// Frames cut by tools/slice_sprites.py, keyed by name ("walk_0"), with their anchors.
struct Sprites {
    frames: HashMap<String, Frame>,
}

impl Sprites {
    async fn load(dir: &str) -> Self {
        let text = load_string(&format!("{dir}/anchors.json")).await.expect("anchors.json");
        let manifest: Manifest = serde_json::from_str(&text).expect("valid anchors.json");
        let mut frames = HashMap::new();
        for files in manifest.animations.values() {
            for file in files {
                let texture = load_texture(&format!("{dir}/{file}")).await.expect("frame image");
                texture.set_filter(FilterMode::Linear);
                let [ax, ay] = manifest.anchors[file];
                frames.insert(file.trim_end_matches(".png").to_string(), Frame { texture, anchor: vec2(ax, ay) });
            }
        }
        Self { frames }
    }

    /// Draw a frame with its anchor at `at`, mirrored when facing left.
    fn draw(&self, name: &str, at: Vec2, facing: f32) {
        let frame = &self.frames[name];
        let w = frame.texture.width();
        let ax = if facing < 0.0 { w - 1.0 - frame.anchor.x } else { frame.anchor.x };
        let params = DrawTextureParams { flip_x: facing < 0.0, ..Default::default() };
        draw_texture_ex(&frame.texture, at.x - ax, at.y - frame.anchor.y, WHITE, params);
    }
}

struct Clip {
    frames: &'static [&'static str],
    fps: f32,
    looped: bool,
}

impl Clip {
    fn frame(&self, time: f32) -> &'static str {
        let i = (time * self.fps) as usize;
        let i = if self.looped { i % self.frames.len() } else { i.min(self.frames.len() - 1) };
        self.frames[i]
    }

    fn finished(&self, time: f32) -> bool {
        !self.looped && time * self.fps >= self.frames.len() as f32
    }
}

const SIT: Clip = Clip { frames: &["sit_0"], fps: 1.0, looped: true };
const WALK: Clip = Clip { frames: &["walk_0", "walk_1", "walk_2", "walk_3"], fps: 10.0, looped: true };
const RISE: Clip = Clip { frames: &["jump_prep_1", "mid_jump_0"], fps: 12.0, looped: false };
const FALL: Clip = Clip { frames: &["mid_jump_2"], fps: 1.0, looped: true };
const LAND: Clip = Clip { frames: &["land_0", "land_1", "land_2"], fps: 14.0, looped: false };

// ------------------ World -------------------

struct Tree {
    x: f32,
    branch: Branch,
}

impl Tree {
    fn new(x: f32, y: f32, direction: f32, length: f32, thickness: f32, stiffness: f32, seed: u64) -> Self {
        let segments = ((length / 18.0).round() as usize).max(8);
        let mut branch = Branch::new_tree_branch(
            EngineVec2::new(x, y), length, thickness, segments, stiffness, 0.6, direction, seed, 0.0,
        );
        // Settle under its own weight, then start perfectly still
        for _ in 0..120 {
            branch.update_physics(BRANCH_SUB_STEPS, STEP, BRANCH_GRAVITY, BRANCH_DAMPING);
        }
        branch.rest();
        Self { x, branch }
    }

    /// Highest point of the branch's top under any of `xs`.
    fn surface(&self, xs: &[f32]) -> Option<f32> {
        xs.iter().filter_map(|&x| self.branch.surface_y(x)).reduce(f32::min)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Footing {
    Ground,
    Branch(usize),
    Air,
}

struct Kitty {
    x: f32,
    y: f32, // feet
    velocity_y: f32,
    footing: Footing,
    facing: f32,
    clip: &'static Clip,
    clip_time: f32,
    landing: bool,
}

impl Kitty {
    fn play(&mut self, clip: &'static Clip) {
        if !std::ptr::eq(self.clip, clip) {
            self.clip = clip;
            self.clip_time = 0.0;
        }
    }

    fn feet(&self) -> [f32; 3] {
        [self.x - FOOT_REACH, self.x, self.x + FOOT_REACH]
    }
}

// ------------------ Input -------------------

#[derive(Default, Clone, Copy)]
struct Controls {
    left: bool,
    right: bool,
    jump: bool,      // tapped this frame
    jump_held: bool, // for drawing the button pressed
}

/// On-screen buttons, in playfield coordinates.
const LEFT_BUTTON: (Vec2, f32) = (Vec2::from_array([85.0, 1130.0]), 52.0);
const RIGHT_BUTTON: (Vec2, f32) = (Vec2::from_array([215.0, 1130.0]), 52.0);
const JUMP_BUTTON: (Vec2, f32) = (Vec2::from_array([515.0, 1130.0]), 58.0);

/// Keyboard plus every finger (or the mouse) on the on-screen buttons. Holding a button walks;
/// jumping uses tap events, which are recorded between frames, so a quick tap is never missed.
fn read_controls(view: &View) -> Controls {
    let near = |p: Vec2, (centre, radius): (Vec2, f32)| p.distance(centre) < radius * 1.25;
    let mouse = view.to_playfield(mouse_position().into());
    let held = |button| {
        touches().iter().any(|t| {
            !matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled) && near(view.to_playfield(t.position), button)
        }) || (is_mouse_button_down(MouseButton::Left) && near(mouse, button))
    };
    let tapped = |button| {
        touches().iter().any(|t| t.phase == TouchPhase::Started && near(view.to_playfield(t.position), button))
            || (is_mouse_button_pressed(MouseButton::Left) && near(mouse, button))
    };
    Controls {
        left: held(LEFT_BUTTON) || is_key_down(KeyCode::Left) || is_key_down(KeyCode::A),
        right: held(RIGHT_BUTTON) || is_key_down(KeyCode::Right) || is_key_down(KeyCode::D),
        jump: tapped(JUMP_BUTTON) || is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Up),
        jump_held: held(JUMP_BUTTON),
    }
}

// ------------------ Screen scaling -------------------

/// Fits the 600x1200 playfield inside the window, centred, whatever its size or pixel density.
struct View {
    scale: f32,
}

impl View {
    fn fit() -> Self {
        Self { scale: (screen_width() / VIEW_W).min(screen_height() / VIEW_H) }
    }

    /// A camera that maps the playfield to the middle of the screen, y pointing down.
    fn camera(&self) -> Camera2D {
        Camera2D {
            target: vec2(VIEW_W / 2.0, VIEW_H / 2.0),
            zoom: vec2(2.0 * self.scale / screen_width(), 2.0 * self.scale / screen_height()),
            ..Default::default()
        }
    }

    fn to_playfield(&self, screen: Vec2) -> Vec2 {
        let centre = vec2(screen_width(), screen_height()) / 2.0;
        (screen - centre) / self.scale + vec2(VIEW_W, VIEW_H) / 2.0
    }
}

// ------------------ Game -------------------

fn step(kitty: &mut Kitty, trees: &mut [Tree], controls: &Controls) {
    let dir = controls.right as i32 - controls.left as i32;
    if dir != 0 {
        kitty.facing = dir as f32;
    }
    kitty.x = (kitty.x + dir as f32 * WALK_SPEED * STEP).clamp(40.0, VIEW_W - 40.0);

    match kitty.footing {
        Footing::Branch(i) => match trees[i].surface(&kitty.feet()) {
            Some(y) => {
                kitty.y = y;
                // Press down through the paw nearest the middle that's on the branch
                let mut feet = kitty.feet();
                feet.sort_by(|a, b| (a - kitty.x).abs().total_cmp(&(b - kitty.x).abs()));
                if let Some(&x) = feet.iter().find(|&&x| trees[i].branch.surface_y(x).is_some()) {
                    trees[i].branch.apply_load(x, KITTY_WEIGHT);
                }
            }
            None => {
                kitty.footing = Footing::Air;
                kitty.velocity_y = 0.0;
            }
        },
        Footing::Ground => kitty.y = GROUND_Y,
        Footing::Air => {
            let previous_y = kitty.y;
            kitty.velocity_y += GRAVITY * STEP;
            kitty.y += kitty.velocity_y * STEP;
            if kitty.velocity_y > 0.0 {
                // Branches are one-way: only catch Kitty on the way down
                let catch = trees.iter().enumerate().filter_map(|(i, t)| {
                    t.surface(&kitty.feet())
                        .filter(|&s| previous_y <= s + LANDING_TOLERANCE && kitty.y >= s)
                        .map(|s| (i, s))
                }).min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((i, s)) = catch {
                    trees[i].branch.apply_impulse(kitty.x, kitty.velocity_y * STEP * LANDING_KICK * 60.0 / 8.0);
                    kitty.y = s;
                    kitty.footing = Footing::Branch(i);
                    kitty.landing = true;
                } else if kitty.y >= GROUND_Y {
                    kitty.y = GROUND_Y;
                    kitty.footing = Footing::Ground;
                    kitty.landing = true;
                }
            }
        }
    }

    if controls.jump && kitty.footing != Footing::Air {
        if let Footing::Branch(i) = kitty.footing {
            trees[i].branch.apply_impulse(kitty.x, JUMP_KICK);
        }
        kitty.footing = Footing::Air;
        kitty.velocity_y = -JUMP_SPEED;
    }

    for tree in trees.iter_mut() {
        tree.branch.update_physics(BRANCH_SUB_STEPS, STEP, BRANCH_GRAVITY, BRANCH_DAMPING);
    }
}

fn animate(kitty: &mut Kitty, dir: i32, dt: f32) {
    if kitty.footing == Footing::Air {
        kitty.landing = false;
        kitty.play(if kitty.velocity_y < 0.0 { &RISE } else { &FALL });
    } else if dir != 0 {
        kitty.landing = false;
        kitty.play(&WALK);
    } else if kitty.landing {
        kitty.play(&LAND);
        if kitty.clip.finished(kitty.clip_time) {
            kitty.landing = false;
        }
    } else {
        kitty.play(&SIT);
    }
    kitty.clip_time += dt;
}

fn draw_branch(branch: &Branch, tuft_seed: usize) {
    let nodes = &branch.nodes;
    let segments = branch.segments;
    let bottom = segments + 1;
    let p = |i: usize| vec2(nodes[i].pos.x, nodes[i].pos.y);
    for i in 0..segments {
        let (t0, t1, b0, b1) = (p(i), p(i + 1), p(i + bottom), p(i + 1 + bottom));
        draw_triangle(t0, t1, b1, BARK);
        draw_triangle(t0, b1, b0, BARK);
    }
    for i in 0..segments {
        draw_line(p(i).x, p(i).y, p(i + 1).x, p(i + 1).y, 3.0, BARK_DARK);
        draw_line(p(i + bottom).x, p(i + bottom).y, p(i + 1 + bottom).x, p(i + 1 + bottom).y, 2.0, BARK_DARK);
        draw_line(p(i).x, p(i).y + 2.0, p(i + 1).x, p(i + 1).y + 2.0, 2.0, BARK_LIGHT);
    }
    let tip = p(segments);
    for k in 0..6 {
        let angle = (k * 61 + tuft_seed * 17) as f32;
        let offset = vec2(angle.cos() * 11.0, angle.sin() * 9.0 - 5.0);
        draw_circle(tip.x + offset.x, tip.y + offset.y, 8.0, LEAVES[(k + tuft_seed) % LEAVES.len()]);
    }
}

fn draw_button((centre, radius): (Vec2, f32), label: &str, held: bool) {
    let alpha = if held { 0.55 } else { 0.3 };
    draw_circle(centre.x, centre.y, radius, Color::new(1.0, 1.0, 1.0, alpha));
    draw_circle_lines(centre.x, centre.y, radius, 3.0, Color::new(0.2, 0.2, 0.2, 0.6));
    let size = measure_text(label, None, 44, 1.0);
    draw_text(label, centre.x - size.width / 2.0, centre.y + size.height / 2.0, 44.0, Color::new(0.15, 0.15, 0.15, 0.8));
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Kitty Adventure (macroquad spike)".to_owned(),
        window_width: 450,
        window_height: 900,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
    macroquad::file::set_pc_assets_folder("assets");

    let sprites = Sprites::load("kitty").await;
    let background = load_texture("background.png").await.expect("background.png");
    background.set_filter(FilterMode::Linear);

    let mut trees = vec![
        // Same size and stiffness range as the game's branches
        Tree::new(70.0, 900.0, 1.0, 220.0, 34.0, 0.9, 7),
        Tree::new(530.0, 680.0, -1.0, 220.0, 32.0, 0.88, 11),
    ];
    let mut kitty = Kitty {
        x: 200.0,
        y: GROUND_Y,
        velocity_y: 0.0,
        footing: Footing::Ground,
        facing: 1.0,
        clip: &SIT,
        clip_time: 0.0,
        landing: false,
    };
    let mut accumulator = 0.0;
    let mut jump_queued = false;

    loop {
        let view = View::fit();
        let controls = read_controls(&view);

        // Fixed-rate physics; a long pause can't fling the branches. Some frames run no step
        // at all, so a tap waits for the next one instead of being dropped.
        accumulator += get_frame_time().min(0.1);
        jump_queued |= controls.jump;
        while accumulator >= STEP {
            step(&mut kitty, &mut trees, &Controls { jump: jump_queued, ..controls });
            jump_queued = false;
            accumulator -= STEP;
        }
        animate(&mut kitty, controls.right as i32 - controls.left as i32, get_frame_time());

        clear_background(Color::new(0.11, 0.11, 0.14, 1.0));
        set_camera(&view.camera());
        draw_rectangle(0.0, 0.0, VIEW_W, VIEW_H, SKY);
        let bg_h = VIEW_W * background.height() / background.width();
        draw_texture_ex(&background, 0.0, GROUND_Y + 40.0 - bg_h, WHITE,
                        DrawTextureParams { dest_size: Some(vec2(VIEW_W, bg_h)), ..Default::default() });
        for (i, tree) in trees.iter().enumerate() {
            draw_branch(&tree.branch, i);
        }
        for tree in &trees {
            draw_rectangle(tree.x - TRUNK_WIDTH / 2.0, 0.0, TRUNK_WIDTH, GROUND_Y + 40.0, TRUNK);
        }
        draw_rectangle(0.0, GROUND_Y, VIEW_W, VIEW_H - GROUND_Y, Color::new(0.36, 0.55, 0.2, 1.0));
        sprites.draw(kitty.clip.frame(kitty.clip_time), vec2(kitty.x, kitty.y), kitty.facing);

        draw_button(LEFT_BUTTON, "<", controls.left);
        draw_button(RIGHT_BUTTON, ">", controls.right);
        draw_button(JUMP_BUTTON, "^", controls.jump_held);
        draw_text(&format!("macroquad spike   {} fps", get_fps()), 16.0, 36.0, 30.0, BLACK);

        set_default_camera();
        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kitty_at(x: f32) -> Kitty {
        Kitty { x, y: GROUND_Y, velocity_y: 0.0, footing: Footing::Ground, facing: 1.0, clip: &SIT, clip_time: 0.0, landing: false }
    }

    fn run(kitty: &mut Kitty, trees: &mut [Tree], steps: usize, controls: Controls) {
        for _ in 0..steps {
            step(kitty, trees, &controls);
        }
    }

    fn tip_y(tree: &Tree) -> f32 {
        tree.branch.top_contour().last().unwrap().1
    }

    #[test]
    fn kitty_jumps_onto_the_branch_and_bends_it() {
        let mut trees = vec![Tree::new(70.0, 900.0, 1.0, 220.0, 34.0, 0.9, 7)];
        let resting_tip = tip_y(&trees[0]);
        let mut kitty = kitty_at(180.0);

        run(&mut kitty, &mut trees, 1, Controls { jump: true, ..Default::default() });
        run(&mut kitty, &mut trees, 120, Controls::default());
        assert!(kitty.footing == Footing::Branch(0), "Kitty should land on the branch");

        // Walk out toward the tip and let it settle under the weight
        run(&mut kitty, &mut trees, 10, Controls { right: true, ..Default::default() });
        run(&mut kitty, &mut trees, 90, Controls::default());
        assert!(kitty.footing == Footing::Branch(0), "Kitty should still be on the branch near its tip");
        let bend = tip_y(&trees[0]) - resting_tip;
        assert!(bend > 10.0, "branch tip should dip under Kitty, dipped {bend:.1}px");

        // Jump off and it springs back
        run(&mut kitty, &mut trees, 1, Controls { jump: true, ..Default::default() });
        run(&mut kitty, &mut trees, 30, Controls { left: true, ..Default::default() });
        run(&mut kitty, &mut trees, 240, Controls::default());
        assert!((tip_y(&trees[0]) - resting_tip).abs() < 3.0, "branch should spring back");
    }
}
