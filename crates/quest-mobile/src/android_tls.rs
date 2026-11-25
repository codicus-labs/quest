//! HTTPS on Android: `reqwest` verifies certificates with `rustls-platform-verifier`, which asks
//! Android's trust manager through JNI. It needs the app's JVM and `Context` once per process,
//! before the first HTTPS request; without them the first verification panics.
//!
//! Kotlin: `app.quest.android.core.RustTls.init(context)`, called before `QuestCore.open`.
//! The Kotlin half of the verifier ships as the Maven artifact `rustls:rustls-platform-verifier`.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::jni_mangle;
use jni::objects::{JClass, JObject};

/// `jni_mangle` exports the JNI symbol and `with_env` catches panics and JNI errors, turning
/// them into a Java `RuntimeException`, so nothing unwinds into the JVM.
#[jni_mangle("app.quest.android.core.RustTls")]
pub extern "system" fn init<'caller>(mut env: EnvUnowned<'caller>, _class: JClass<'caller>, context: JObject<'caller>) {
    env.with_env(|env| rustls_platform_verifier::android::init_with_env(env, context))
        .resolve::<ThrowRuntimeExAndDefault>();
}
