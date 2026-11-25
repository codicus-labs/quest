package app.quest.android.focus

import android.app.Application
import app.quest.android.core.CoreHolder

/** The application for service tests: the test sets the core and notifier (no Rust). */
class TestFocusApp :
    Application(),
    FocusHost {
    override lateinit var core: CoreHolder
    override lateinit var focusNotifier: FocusNotifier
}
