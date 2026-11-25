package app.quest.android.sync

import android.app.Application
import app.quest.android.core.CoreHolder

/** The application for worker tests: the test sets the core (no Rust) and counts widget redraws. */
class TestSyncApp :
    Application(),
    SyncHost {
    override lateinit var core: CoreHolder
    var widgetRefreshes = 0

    override suspend fun refreshWidgets() {
        widgetRefreshes++
    }
}
