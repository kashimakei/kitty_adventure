//! Pickups resting on limbs: fish treats and golden fish for score, hearts for health. They sit
//! just above the bark and ride along as the limb bends.

use macroquad::prelude::*;

pub use crate::level::ItemKind;
use crate::limb::{Limb, LimbId};
use crate::sprites::Assets;

pub struct Item {
    pub kind: ItemKind,
    pub limb: LimbId,
    /// How far along the limb's walkable span, from the trunk toward the tip.
    pub frac: f32,
    pub taken: bool,
    bob_phase: f32,
}

impl Item {
    pub fn new(kind: ItemKind, limb: LimbId, frac: f32, bob_phase: f32) -> Self {
        Self { kind, limb, frac, taken: false, bob_phase }
    }

    /// Where it hovers now, or None if its part of the limb has fallen away.
    pub fn position(&self, limbs: &[Limb], time: f32) -> Option<Vec2> {
        let limb = &limbs[self.limb];
        let (lo, hi) = limb.walk_range();
        let x = if limb.direction > 0.0 { lo + (hi - lo) * self.frac } else { hi - (hi - lo) * self.frac };
        let surface = limb.surface_y(x)?;
        let bob = (time * 3.0 + self.bob_phase).sin() * 3.0;
        Some(vec2(x, surface - 26.0 + bob))
    }

    pub fn rect(&self, limbs: &[Limb], time: f32) -> Option<Rect> {
        self.position(limbs, time).map(|p| Rect::new(p.x - 20.0, p.y - 16.0, 40.0, 32.0))
    }

    pub fn draw(&self, assets: &Assets, limbs: &[Limb], time: f32) {
        if self.taken {
            return;
        }
        let Some(pos) = self.position(limbs, time) else { return };
        // The fish row's first frame came out beige, so fish twinkle between the two silver ones
        let frames: &[&str] = match self.kind {
            ItemKind::Fish => &["fish_1", "fish_2"],
            ItemKind::Gold => &["gold_0", "gold_1", "gold_2"],
            ItemKind::Heart => &["heart_0", "heart_1", "heart_2"],
        };
        let frame = ((time * 4.0 + self.bob_phase) as usize) % frames.len();
        assets.items.draw(frames[frame], pos, 1.0, WHITE);
    }
}
