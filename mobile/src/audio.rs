//! Sound effects and music from the game's existing sounds (assets/*.wav, BGM.ogg).

use macroquad::audio::{load_sound, play_sound, play_sound_once, PlaySoundParams, Sound};

use crate::enemies::Sfx;

pub struct Audio {
    meows: Vec<Sound>,
    hiss: Option<Sound>,
    crunch: Option<Sound>,
    music: Option<Sound>,
    meow_index: std::cell::Cell<usize>,
}

impl Audio {
    /// Missing or unplayable sounds are skipped rather than stopping the game.
    pub async fn load() -> Self {
        let mut meows = Vec::new();
        for name in ["sounds/meow.wav", "sounds/meow2.wav"] {
            if let Ok(sound) = load_sound(name).await {
                meows.push(sound);
            }
        }
        Self {
            meows,
            hiss: load_sound("sounds/hiss.wav").await.ok(),
            crunch: load_sound("sounds/crunch.wav").await.ok(),
            music: load_sound("sounds/BGM.ogg").await.ok(),
            meow_index: std::cell::Cell::new(0),
        }
    }

    pub fn start_music(&self) {
        if let Some(music) = &self.music {
            play_sound(music, PlaySoundParams { looped: true, volume: 0.45 });
        }
    }

    pub fn play(&self, sfx: Sfx) {
        let sound = match sfx {
            Sfx::Jump => {
                if self.meows.is_empty() {
                    return;
                }
                let i = self.meow_index.get();
                self.meow_index.set((i + 1) % self.meows.len());
                Some(&self.meows[i])
            }
            Sfx::Hurt | Sfx::Hiss => self.hiss.as_ref(),
            Sfx::Snap => self.crunch.as_ref(),
            _ => None,
        };
        if let Some(sound) = sound {
            play_sound_once(sound);
        }
    }
}
