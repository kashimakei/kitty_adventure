//! Progress, kept between sessions as a small JSON document: which levels are cleared, their best
//! scores, times and stars, the endless climb's best, and whether the story's opening has been
//! seen. It lives in save.json in the user's data folder on desktop, the app's private files folder
//! on Android, and the browser's localStorage on the web (through a tiny JS plugin in
//! web/index.html). Only facts are stored; what's unlocked follows from them. Missing or broken
//! storage only means starting afresh: nothing here ever stops the game.

use std::collections::BTreeMap;

use macroquad::logging::warn;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const VERSION: u32 = 2;

/// A level's stars, each won once and kept.
#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Debug)]
#[serde(default)]
pub struct Stars {
    /// Reached the top.
    pub top: bool,
    /// Without a bump.
    pub no_bumps: bool,
    /// Within the level's star time.
    pub quick: bool,
}

impl Stars {
    pub fn count(self) -> usize {
        self.top as usize + self.no_bumps as usize + self.quick as usize
    }

    pub fn union(self, other: Stars) -> Stars {
        Stars { top: self.top || other.top, no_bumps: self.no_bumps || other.no_bumps, quick: self.quick || other.quick }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Debug)]
#[serde(default)]
pub struct LevelRecord {
    pub cleared: bool,
    pub best_score: u32,
    /// Seconds to the top, at best.
    pub best_time: Option<f32>,
    pub stars: Stars,
    /// The level's layout these bests were set on (see `LevelDef::layout_rev`).
    pub layout_rev: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Debug)]
#[serde(default)]
pub struct EndlessRecord {
    pub best_score: u32,
    /// The furthest climb reached in a run.
    pub best_climb: u32,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq, Debug)]
#[serde(default)]
pub struct SaveData {
    pub version: u32,
    pub seen_intro: bool,
    /// Playtesting: every level open, though none counts as cleared.
    pub unlock_all: bool,
    /// By level key ("mossy-2").
    pub levels: BTreeMap<String, LevelRecord>,
    pub endless: EndlessRecord,
    /// The single high score of the game before levels were picked from a map.
    pub legacy_high_score: u32,
    /// Anything a newer game saved that this one doesn't know about, kept as it was.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl SaveData {
    pub fn level(&self, key: &str) -> LevelRecord {
        self.levels.get(key).copied().unwrap_or_default()
    }

    pub fn cleared(&self, key: &str) -> bool {
        self.levels.get(key).is_some_and(|r| r.cleared)
    }

    /// Take in `other` without losing anything either has: cleared levels and stars from both,
    /// the higher scores and the quicker times. Bests set on an older layout of a level give way
    /// to the newer one's.
    pub fn merge(&mut self, other: &SaveData) {
        self.seen_intro |= other.seen_intro;
        self.unlock_all |= other.unlock_all;
        for (key, theirs) in &other.levels {
            let mine = self.levels.entry(key.clone()).or_default();
            if theirs.layout_rev > mine.layout_rev {
                mine.best_score = theirs.best_score;
                mine.best_time = theirs.best_time;
                mine.layout_rev = theirs.layout_rev;
            } else if theirs.layout_rev == mine.layout_rev {
                mine.best_score = mine.best_score.max(theirs.best_score);
                mine.best_time = match (mine.best_time, theirs.best_time) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
            mine.cleared |= theirs.cleared;
            mine.stars = mine.stars.union(theirs.stars);
        }
        self.endless.best_score = self.endless.best_score.max(other.endless.best_score);
        self.endless.best_climb = self.endless.best_climb.max(other.endless.best_climb);
        self.legacy_high_score = self.legacy_high_score.max(other.legacy_high_score);
        for (key, value) in &other.extra {
            self.extra.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }

    /// What the old single high score becomes: the endless climb's best, with the opening seen.
    fn from_legacy(score: u32) -> SaveData {
        SaveData {
            version: VERSION,
            seen_intro: score > 0,
            legacy_high_score: score,
            endless: EndlessRecord { best_score: score, best_climb: 0 },
            ..SaveData::default()
        }
    }
}

/// Where saves are kept.
trait Storage {
    fn load_text(&self) -> Option<String>;
    fn save_text(&self, text: &str);
    /// The old high score, from before save.json.
    fn load_legacy(&self) -> Option<u32>;
    /// Keep a save that couldn't be read out of the way, rather than overwrite it.
    fn set_aside(&self, text: &str);
}

/// The saved progress (moving an old high score over, the first time).
pub fn load() -> SaveData {
    load_from(&store::platform())
}

/// Save `data`, first taking in anything saved meanwhile (by another browser tab, say), so
/// nothing's ever lost; `data` ends up as what was written.
pub fn write(data: &mut SaveData) {
    write_to(&store::platform(), data);
}

fn load_from(storage: &impl Storage) -> SaveData {
    match storage.load_text() {
        Some(text) => match serde_json::from_str::<SaveData>(&text) {
            Ok(data) => data,
            Err(error) => {
                warn!("couldn't read the saved progress ({error}); starting afresh");
                storage.set_aside(&text);
                let mut data = SaveData::from_legacy(storage.load_legacy().unwrap_or(0));
                write_to(storage, &mut data);
                data
            }
        },
        None => {
            let mut data = SaveData::from_legacy(storage.load_legacy().unwrap_or(0));
            if data.legacy_high_score > 0 {
                write_to(storage, &mut data);
            }
            data
        }
    }
}

fn write_to(storage: &impl Storage, data: &mut SaveData) {
    if let Some(saved) = storage.load_text().and_then(|text| serde_json::from_str::<SaveData>(&text).ok()) {
        data.merge(&saved);
    }
    data.version = VERSION;
    match serde_json::to_string_pretty(data) {
        Ok(text) => storage.save_text(&text),
        Err(error) => warn!("couldn't save progress: {error}"),
    }
}

/// The endless climb's best score (or the old high score).
pub fn load_high_score() -> u32 {
    load().endless.best_score
}

pub fn save_high_score(score: u32) {
    let mut data = load();
    data.endless.best_score = data.endless.best_score.max(score);
    write(&mut data);
}

/// Desktop and Android: save.json in a folder, written whole and renamed into place.
#[cfg(not(target_arch = "wasm32"))]
mod store {
    use std::fs;
    use std::path::PathBuf;

    use macroquad::logging::warn;

    const FILE_NAME: &str = "save.json";
    const LEGACY_FILE_NAME: &str = "high_score";

    /// Saves kept in a folder (`None` where the platform has none to offer: nothing's saved).
    pub struct Folder(pub Option<PathBuf>);

    pub fn platform() -> Folder {
        Folder(dir())
    }

    impl super::Storage for Folder {
        fn load_text(&self) -> Option<String> {
            fs::read_to_string(self.0.as_ref()?.join(FILE_NAME)).ok()
        }

        fn save_text(&self, text: &str) {
            let Some(dir) = &self.0 else { return };
            let path = dir.join(FILE_NAME);
            // Write a temporary file and rename it over the old one, so a crash mid-write can't
            // leave half a save behind
            let temp = path.with_extension("json.tmp");
            let written = fs::create_dir_all(dir)
                .and_then(|_| fs::write(&temp, text))
                .and_then(|_| fs::rename(&temp, &path));
            if let Err(error) = written {
                warn!("couldn't save progress to {}: {}", path.display(), error);
            }
        }

        fn load_legacy(&self) -> Option<u32> {
            fs::read_to_string(self.0.as_ref()?.join(LEGACY_FILE_NAME)).ok()?.trim().parse().ok()
        }

        fn set_aside(&self, text: &str) {
            if let Some(dir) = &self.0 {
                let _ = fs::write(dir.join("save.json.bad"), text);
            }
        }
    }

    #[cfg(target_os = "android")]
    fn dir() -> Option<PathBuf> {
        Some(super::android::files_dir())
    }

    /// The platform's per-user data folder: $XDG_DATA_HOME or ~/.local/share on Linux,
    /// ~/Library/Application Support on macOS, %APPDATA% on Windows.
    #[cfg(not(target_os = "android"))]
    fn dir() -> Option<PathBuf> {
        const APP_DIR: &str = "kitty-adventure";
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        #[cfg(windows)]
        let base = var("APPDATA");
        #[cfg(target_vendor = "apple")]
        let base = var("HOME").map(|home| home.join("Library/Application Support"));
        #[cfg(not(any(windows, target_vendor = "apple")))]
        let base = var("XDG_DATA_HOME")
            .filter(|dir| dir.is_absolute())
            .or_else(|| var("HOME").map(|home| home.join(".local/share")));
        Some(base?.join(APP_DIR))
    }
}

/// Android keeps each app's private files in a folder only the system knows for sure (it moves
/// for secondary users and work profiles), so ask the activity for it once through JNI.
#[cfg(target_os = "android")]
mod android {
    use std::ffi::CStr;
    use std::path::PathBuf;
    use std::ptr::null_mut;
    use std::sync::OnceLock;

    use macroquad::miniquad::native::android::{attach_jni_env, ndk_sys, ACTIVITY};
    use ndk_sys::{jobject, JNIEnv};

    /// Where Android puts it for the main user, if asking fails.
    const FALLBACK: &str = "/data/data/com.kashimakei.kittyadventure/files";

    pub fn files_dir() -> PathBuf {
        static DIR: OnceLock<PathBuf> = OnceLock::new();
        DIR.get_or_init(|| unsafe { ask_activity() }.unwrap_or_else(|| PathBuf::from(FALLBACK))).clone()
    }

    /// `activity.getFilesDir().getAbsolutePath()`, on the thread miniquad already attached to
    /// the JVM (the game's own), checking every step so a JNI hiccup can't crash the game.
    unsafe fn ask_activity() -> Option<PathBuf> {
        let activity = ACTIVITY;
        if activity.is_null() {
            return None;
        }
        let env = attach_jni_env();
        let files_dir = call(env, activity, c"getFilesDir", c"()Ljava/io/File;")?;
        let path = call(env, files_dir, c"getAbsolutePath", c"()Ljava/lang/String;");
        ((**env).DeleteLocalRef?)(env, files_dir);
        let path = path?;
        let chars = ((**env).GetStringUTFChars?)(env, path, null_mut());
        let dir = (!chars.is_null()).then(|| {
            let dir = PathBuf::from(CStr::from_ptr(chars).to_string_lossy().into_owned());
            if let Some(release) = (**env).ReleaseStringUTFChars {
                release(env, path, chars);
            }
            dir
        });
        ((**env).DeleteLocalRef?)(env, path);
        dir
    }

    /// Call a no-argument Java method that returns an object.
    unsafe fn call(env: *mut JNIEnv, object: jobject, name: &CStr, signature: &CStr) -> Option<jobject> {
        let class = ((**env).GetObjectClass?)(env, object);
        if class.is_null() {
            return None;
        }
        let method = ((**env).GetMethodID?)(env, class, name.as_ptr(), signature.as_ptr());
        ((**env).DeleteLocalRef?)(env, class);
        if thrown(env) || method.is_null() {
            return None;
        }
        let result = ((**env).CallObjectMethod?)(env, object, method);
        if thrown(env) || result.is_null() {
            return None;
        }
        Some(result)
    }

    /// Clear a pending Java exception, which would otherwise abort the next JNI call.
    unsafe fn thrown(env: *mut JNIEnv) -> bool {
        let (Some(check), Some(clear)) = ((**env).ExceptionCheck, (**env).ExceptionClear) else { return false };
        let pending = check(env) != 0;
        if pending {
            clear(env);
        }
        pending
    }
}

/// Web: localStorage, through the `kitty_storage` plugin registered in web/index.html. Without
/// the plugin (or with an old copy of the page that lacks these calls) the JS loader stubs the
/// imports out, so loads find nothing and saves do nothing.
#[cfg(target_arch = "wasm32")]
mod store {
    /// Bumped whenever these imports change, so the loader can spot a stale index.html.
    const PLUGIN_VERSION: u32 = 2;

    extern "C" {
        /// The old high score's key, for moving it over.
        fn kitty_storage_load() -> u32;
        /// Reads the save and returns its length in bytes (0 if there's none), for the next call
        /// to copy out.
        fn kitty_storage_load_len() -> u32;
        fn kitty_storage_load_into(ptr: *mut u8, len: u32) -> u32;
        fn kitty_storage_save_text(ptr: *const u8, len: u32);
    }

    pub struct Browser;

    pub fn platform() -> Browser {
        Browser
    }

    impl super::Storage for Browser {
        fn load_text(&self) -> Option<String> {
            let len = unsafe { kitty_storage_load_len() } as usize;
            if len == 0 {
                return None;
            }
            let mut bytes = vec![0u8; len];
            let copied = unsafe { kitty_storage_load_into(bytes.as_mut_ptr(), len as u32) } as usize;
            bytes.truncate(copied.min(len));
            String::from_utf8(bytes).ok().filter(|text| !text.is_empty())
        }

        fn save_text(&self, text: &str) {
            unsafe { kitty_storage_save_text(text.as_ptr(), text.len() as u32) }
        }

        fn load_legacy(&self) -> Option<u32> {
            Some(unsafe { kitty_storage_load() }).filter(|&score| score > 0)
        }

        fn set_aside(&self, _text: &str) {}
    }

    /// The JS loader compares this with the plugin's version (see `init_plugins`).
    #[no_mangle]
    pub extern "C" fn kitty_storage_crate_version() -> u32 {
        PLUGIN_VERSION
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::store::Folder;
    use super::*;

    /// A fresh, empty folder for one test.
    fn folder(name: &str) -> (Folder, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("kitty-save-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (Folder(Some(dir.clone())), dir)
    }

    fn cleared(score: u32, time: f32, stars: Stars, rev: u8) -> LevelRecord {
        LevelRecord { cleared: true, best_score: score, best_time: Some(time), stars, layout_rev: rev }
    }

    #[test]
    fn progress_round_trips_and_keeps_what_it_doesnt_know() {
        let (store, dir) = folder("round-trip");
        let mut data = SaveData { seen_intro: true, ..SaveData::default() };
        data.levels.insert("mossy-1".into(), cleared(1500, 41.5, Stars { top: true, no_bumps: true, quick: false }, 1));
        data.endless.best_score = 9000;
        data.extra.insert("future".into(), Value::from("kept"));
        write_to(&store, &mut data);
        let text = std::fs::read_to_string(dir.join("save.json")).unwrap();
        assert!(text.contains("\"version\": 2"), "{text}");
        let loaded = load_from(&store);
        assert_eq!(loaded, data);
        assert_eq!(loaded.extra["future"], "kept");
        assert!(!dir.join("save.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_old_high_score_moves_over() {
        let (store, dir) = folder("legacy");
        std::fs::write(dir.join("high_score"), "4321\n").unwrap();
        let data = load_from(&store);
        assert!(data.seen_intro);
        assert_eq!((data.endless.best_score, data.legacy_high_score), (4321, 4321));
        assert!(dir.join("save.json").exists(), "written straight away");
        assert!(dir.join("high_score").exists(), "the old file is left for older builds");
        std::fs::remove_dir_all(&dir).unwrap();
        // Nothing saved at all: a first launch
        let (store, dir) = folder("fresh");
        assert_eq!(load_from(&store), SaveData { version: VERSION, ..SaveData::default() });
        assert!(!dir.join("save.json").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_broken_save_is_set_aside() {
        let (store, dir) = folder("broken");
        std::fs::write(dir.join("high_score"), "77\n").unwrap();
        std::fs::write(dir.join("save.json"), "{ not json").unwrap();
        let data = load_from(&store);
        assert_eq!(data.endless.best_score, 77);
        assert_eq!(std::fs::read_to_string(dir.join("save.json.bad")).unwrap(), "{ not json");
        assert_eq!(load_from(&store), data);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Writing takes in whatever was saved meanwhile, so another tab's progress isn't undone.
    #[test]
    fn writing_never_loses_progress() {
        let (store, dir) = folder("merge");
        let mut other_tab = SaveData::default();
        other_tab.levels.insert("mossy-1".into(), cleared(2000, 50.0, Stars { top: true, no_bumps: false, quick: true }, 1));
        other_tab.levels.insert("mossy-2".into(), cleared(900, 70.0, Stars { top: true, ..Stars::default() }, 1));
        other_tab.endless.best_score = 5000;
        write_to(&store, &mut other_tab);
        let mut this_tab = SaveData::default();
        this_tab.levels.insert("mossy-1".into(), cleared(1800, 44.0, Stars { top: true, no_bumps: true, quick: false }, 1));
        this_tab.endless.best_score = 300;
        write_to(&store, &mut this_tab);
        let saved = load_from(&store);
        assert_eq!(saved, this_tab, "the writer ends up with what was written");
        let mossy = saved.level("mossy-1");
        assert_eq!((mossy.best_score, mossy.best_time), (2000, Some(44.0)));
        assert_eq!(mossy.stars, Stars { top: true, no_bumps: true, quick: true });
        assert!(saved.cleared("mossy-2"));
        assert_eq!(saved.endless.best_score, 5000);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Bests set on an older layout of a level are dropped for the newer layout's, but being
    /// cleared and the stars won are kept.
    #[test]
    fn a_new_layout_starts_its_bests_afresh() {
        let mut old = SaveData::default();
        old.levels.insert("pines-2".into(), cleared(3000, 30.0, Stars { top: true, no_bumps: true, quick: true }, 1));
        let mut new = SaveData::default();
        new.levels.insert("pines-2".into(), cleared(1200, 80.0, Stars { top: true, ..Stars::default() }, 2));
        let mut merged = old.clone();
        merged.merge(&new);
        let record = merged.level("pines-2");
        assert_eq!((record.best_score, record.best_time, record.layout_rev), (1200, Some(80.0), 2));
        assert_eq!(record.stars.count(), 3);
        // And the other way round: the newer one already here keeps its bests
        let mut merged = new.clone();
        merged.merge(&old);
        assert_eq!(merged.level("pines-2").best_score, 1200);
    }
}
