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

#[cfg(target_os = "android")]
mod android {
    use macroquad::miniquad::native::android::{attach_jni_env, ndk_sys, ACTIVITY};
    use ndk_sys::jvalue;

    /// `activity.moveTaskToBack(true)`, clearing any Java exception so it can't abort a later
    /// JNI call.
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
        if let (Some(check), Some(clear)) = ((**env).ExceptionCheck, (**env).ExceptionClear) {
            if check(env) != 0 {
                clear(env);
            }
        }
    }
}
