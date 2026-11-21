//! Sound effects and music from the game's existing sounds (assets/*.wav, BGM.ogg).

use std::cell::Cell;
use std::sync::{Arc, Mutex};

use macroquad::audio::{load_sound, play_sound, play_sound_once, PlaySoundParams, Sound};
use macroquad::experimental::coroutines::start_coroutine;

use crate::enemies::Sfx;

/// A sound that arrives once it has loaded in the background.
type Slot = Arc<Mutex<Option<Sound>>>;

/// Load `path` in a coroutine. On the web a sound the browser can't decode never finishes
/// loading, so the game must never wait on one: until (unless) it arrives, it just stays quiet.
fn load_in_background(path: &'static str) -> Slot {
    let slot: Slot = Arc::new(Mutex::new(None));
    let fill = slot.clone();
    start_coroutine(async move {
        if let Ok(sound) = load_sound(path).await {
            *fill.lock().unwrap() = Some(sound);
        }
    });
    slot
}

pub struct Audio {
    meows: [Slot; 2],
    hiss: Slot,
    crunch: Slot,
    music: Slot,
    meow_index: Cell<usize>,
    music_wanted: Cell<bool>,
    music_playing: Cell<bool>,
}

impl Audio {
    /// Start loading every sound; missing or unplayable ones are skipped rather than stopping
    /// the game.
    pub fn load() -> Self {
        Self {
            meows: [load_in_background("sounds/meow.wav"), load_in_background("sounds/meow2.wav")],
            hiss: load_in_background("sounds/hiss.wav"),
            crunch: load_in_background("sounds/crunch.wav"),
            music: load_in_background("sounds/BGM.ogg"),
            meow_index: Cell::new(0),
            music_wanted: Cell::new(false),
            music_playing: Cell::new(false),
        }
    }

    /// Play the music as soon as it has loaded.
    pub fn start_music(&self) {
        self.music_wanted.set(true);
        self.update();
    }

    /// Call once a frame: starts the music when it arrives.
    pub fn update(&self) {
        if self.music_wanted.get() && !self.music_playing.get() {
            if let Some(music) = self.music.lock().unwrap().as_ref() {
                play_sound(music, PlaySoundParams { looped: true, volume: 0.45 });
                self.music_playing.set(true);
            }
        }
    }

    pub fn play(&self, sfx: Sfx) {
        let slot = match sfx {
            Sfx::Jump => {
                let i = self.meow_index.get();
                self.meow_index.set((i + 1) % self.meows.len());
                &self.meows[i]
            }
            Sfx::Hurt | Sfx::Hiss => &self.hiss,
            Sfx::Snap => &self.crunch,
            _ => return,
        };
        if let Some(sound) = slot.lock().unwrap().as_ref() {
            play_sound_once(sound);
        }
    }
}
