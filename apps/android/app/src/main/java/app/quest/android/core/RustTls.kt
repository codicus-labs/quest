package app.quest.android.core

import android.content.Context

/**
 * Lets the Rust core verify HTTPS certificates with Android's trust store
 * (rustls-platform-verifier). Call [init] once per process, before `QuestCore.open`,
 * otherwise the first HTTPS request fails.
 */
object RustTls {
    init {
        System.loadLibrary("quest_mobile")
    }

    /** Throws a RuntimeException if the verifier cannot reach the JVM. */
    external fun init(context: Context)
}
