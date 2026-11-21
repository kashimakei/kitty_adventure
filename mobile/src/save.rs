//! The high score, kept between sessions: a small file in the user's data folder on desktop, the
//! app's private files folder on Android, and the browser's localStorage on the web (through a
//! tiny JS plugin in web/index.html). Missing or broken storage only means no saved score; nothing
//! here ever stops the game.

use std::sync::atomic::{AtomicU32, Ordering};

/// The best score seen this session, so a failed write never loses it while the game runs.
static BEST: AtomicU32 = AtomicU32::new(0);

pub fn load_high_score() -> u32 {
    let stored = store::load().unwrap_or(0);
    BEST.fetch_max(stored, Ordering::Relaxed).max(stored)
}

pub fn save_high_score(score: u32) {
    let best = BEST.fetch_max(score, Ordering::Relaxed).max(score);
    // Never write over a higher score saved meanwhile (by another browser tab, say)
    let saved = store::load().unwrap_or(0);
    if best > saved {
        store::save(best);
    }
}

/// Desktop and Android: a one-line text file.
#[cfg(not(target_arch = "wasm32"))]
mod store {
    use std::fs;
    use std::path::PathBuf;

    use macroquad::logging::warn;

    const FILE_NAME: &str = "high_score";

    pub fn load() -> Option<u32> {
        fs::read_to_string(path()?).ok()?.trim().parse().ok()
    }

    pub fn save(score: u32) {
        let Some(path) = path() else { return };
        // Write a temporary file and rename it over the old one, so a crash mid-write can't
        // leave a half-written score behind
        let temp = path.with_extension("tmp");
        let written = path.parent().map_or(Ok(()), fs::create_dir_all)
            .and_then(|_| fs::write(&temp, format!("{score}\n")))
            .and_then(|_| fs::rename(&temp, &path));
        if let Err(error) = written {
            warn!("couldn't save the high score to {}: {}", path.display(), error);
        }
    }

    fn path() -> Option<PathBuf> {
        Some(dir()?.join(FILE_NAME))
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
/// the plugin the JS loader stubs these imports out, so loads read 0 and saves do nothing.
#[cfg(target_arch = "wasm32")]
mod store {
    /// Bumped whenever these imports change, so the loader can spot a stale index.html.
    const PLUGIN_VERSION: u32 = 1;

    extern "C" {
        fn kitty_storage_load() -> u32;
        fn kitty_storage_save(score: u32);
    }

    pub fn load() -> Option<u32> {
        Some(unsafe { kitty_storage_load() })
    }

    pub fn save(score: u32) {
        unsafe { kitty_storage_save(score) }
    }

    /// The JS loader compares this with the plugin's version (see `init_plugins`).
    #[no_mangle]
    pub extern "C" fn kitty_storage_crate_version() -> u32 {
        PLUGIN_VERSION
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn high_score_round_trips_and_never_goes_down() {
        let dir = std::env::temp_dir().join(format!("kitty-save-test-{}", std::process::id()));
        std::env::set_var("XDG_DATA_HOME", &dir);
        save_high_score(1234);
        assert_eq!(store::load(), Some(1234));
        assert_eq!(std::fs::read_to_string(dir.join("kitty-adventure/high_score")).unwrap(), "1234\n");
        save_high_score(99);
        assert_eq!(store::load(), Some(1234));
        assert_eq!(load_high_score(), 1234);
        std::fs::remove_dir_all(&dir).unwrap();
        // Storage vanishing mid-game keeps the session's best
        assert_eq!(load_high_score(), 1234);
    }
}
