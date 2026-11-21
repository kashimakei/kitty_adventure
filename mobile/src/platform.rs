//! Bits only some platforms have.

/// Back on the title screen: tuck the app away like the home button does. Finishing the
/// activity instead would end the game's native thread, which miniquad starts only once per
/// process. Elsewhere there's nothing to do.
pub fn send_to_background() {
    #[cfg(target_os = "android")]
    unsafe {
        android::move_task_to_back()
    };
}

/// The display's pixels per millimetre (in macroquad's logical pixels), so touch gestures can be
/// sized for thumbs. Android asks the activity; elsewhere it isn't known.
#[cfg(target_os = "android")]
pub fn pixels_per_mm() -> Option<f32> {
    unsafe { android::pixels_per_mm() }
}

#[cfg(not(target_os = "android"))]
pub fn pixels_per_mm() -> Option<f32> {
    None
}

#[cfg(target_os = "android")]
mod android {
    use macroquad::miniquad::native::android::{attach_jni_env, ndk_sys, ACTIVITY};
    use ndk_sys::{jvalue, JNIEnv};

    /// `activity.moveTaskToBack(true)`.
    pub unsafe fn move_task_to_back() {
        let activity = ACTIVITY;
        if activity.is_null() {
            return;
        }
        let env = attach_jni_env();
        let (Some(get_class), Some(get_method), Some(call), Some(delete)) =
            ((**env).GetObjectClass, (**env).GetMethodID, (**env).CallBooleanMethodA, (**env).DeleteLocalRef)
        else {
            return;
        };
        let class = get_class(env, activity);
        if class.is_null() {
            return;
        }
        let method = get_method(env, class, c"moveTaskToBack".as_ptr(), c"(Z)Z".as_ptr());
        delete(env, class);
        if !method.is_null() {
            let args = [jvalue { z: 1 }];
            call(env, activity, method, args.as_ptr());
        }
        clear_exception(env);
    }

    /// `activity.pixelsPerMm()` (Kitty Adventure's own method in MainActivity.java).
    pub unsafe fn pixels_per_mm() -> Option<f32> {
        let activity = ACTIVITY;
        if activity.is_null() {
            return None;
        }
        let env = attach_jni_env();
        let (Some(get_class), Some(get_method), Some(call), Some(delete)) =
            ((**env).GetObjectClass, (**env).GetMethodID, (**env).CallFloatMethodA, (**env).DeleteLocalRef)
        else {
            return None;
        };
        let class = get_class(env, activity);
        if class.is_null() {
            return None;
        }
        let method = get_method(env, class, c"pixelsPerMm".as_ptr(), c"()F".as_ptr());
        delete(env, class);
        let value = if method.is_null() {
            None
        } else {
            let args: [jvalue; 0] = [];
            Some(call(env, activity, method, args.as_ptr()))
        };
        // A call that threw returns nothing worth reading
        let threw = clear_exception(env);
        value.filter(|v| !threw && v.is_finite() && *v > 0.0)
    }

    /// Clear any Java exception a call left behind, so it can't abort a later JNI call.
    /// Returns whether there was one.
    unsafe fn clear_exception(env: *mut JNIEnv) -> bool {
        let (Some(check), Some(clear)) = ((**env).ExceptionCheck, (**env).ExceptionClear) else {
            return false;
        };
        let threw = check(env) != 0;
        if threw {
            clear(env);
        }
        threw
    }
}
