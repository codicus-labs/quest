package app.quest.android.platform

import android.content.Context
import android.content.Intent
import android.content.pm.ShortcutInfo
import android.content.pm.ShortcutManager
import android.graphics.drawable.Icon
import app.quest.android.INTERNAL_ENTRY
import app.quest.android.R
import app.quest.android.core.CoreHolder
import app.quest.android.core.Outcome
import uniffi.quest_mobile.FocusView

/** Intent actions of the launcher shortcuts (static ones in `res/xml/shortcuts.xml`). */
object ShortcutActions {
    const val START_TIMER = "app.quest.android.action.START_TIMER"
    const val LOG_WORK = "app.quest.android.action.LOG_WORK"
    const val QUICK_ADD = "app.quest.android.action.QUICK_ADD"
}

private const val START_TIMER_ID = "start-timer"

/**
 * The dynamic "Start timer" shortcut, named after the task it would start (the core's focus target:
 * the task worked on last). "New task" and "Log work" are static.
 */
object AppShortcuts {
    fun publishStartTimer(context: Context, core: CoreHolder) {
        val view = (core.read { c, now -> c.focus(now, null) } as? Outcome.Done)?.value ?: return
        val longLabel = when (view) {
            is FocusView.Session -> context.getString(R.string.shortcut_start_timer_on, view.session.title)
            is FocusView.Pick -> context.getString(R.string.shortcut_start_timer_long)
        }
        val intent = Intent().setClassName(context, INTERNAL_ENTRY).setAction(ShortcutActions.START_TIMER)
        val shortcut = ShortcutInfo.Builder(context, START_TIMER_ID)
            .setShortLabel(context.getString(R.string.shortcut_start_timer))
            .setLongLabel(longLabel)
            .setIcon(Icon.createWithResource(context, R.drawable.ic_shortcut_timer))
            .setIntent(intent)
            .setRank(1)
            .build()
        // Added (or updated), not set: "Dictate" (CaptureEntries) is dynamic too. False when rate-limited, thrown while the user is locked; either way the next app stop tries again.
        try {
            context.getSystemService(ShortcutManager::class.java)?.addDynamicShortcuts(listOf(shortcut))
        } catch (_: IllegalStateException) {
        }
    }
}
