//! The high score, kept between sessions. (Placeholder: this session only.)

use std::sync::atomic::{AtomicU32, Ordering};

static HIGH: AtomicU32 = AtomicU32::new(0);

pub fn load_high_score() -> u32 {
    HIGH.load(Ordering::Relaxed)
}

pub fn save_high_score(score: u32) {
    HIGH.fetch_max(score, Ordering::Relaxed);
}
