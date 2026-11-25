package app.quest.android.capture

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import android.content.pm.ShortcutInfo
import android.content.pm.ShortcutManager
import android.graphics.drawable.Icon
import app.quest.android.R

private const val DICTATE_SHORTCUT_ID = "dictate"
internal const val SHARE_ALIAS = "app.quest.android.capture.StructureWithAi"

/**
 * The entries outside the app follow the AI capture setting: the "Dictate" launcher shortcut (dynamic, so it
 * can go away; a pinned copy is disabled) and the share target "Structure with AI" (a component that is switched off). Quick add's mic and
 * the search's action read the setting themselves.
 */
object CaptureEntries {
    fun show(context: Context, shown: Boolean) {
        val state = if (shown) PackageManager.COMPONENT_ENABLED_STATE_DEFAULT else PackageManager.COMPONENT_ENABLED_STATE_DISABLED
        context.packageManager.setComponentEnabledSetting(ComponentName(context, SHARE_ALIAS), state, PackageManager.DONT_KILL_APP)
        val shortcuts = context.getSystemService(ShortcutManager::class.java) ?: return
        val ids = listOf(DICTATE_SHORTCUT_ID)
        // False when rate-limited, thrown while the user is locked; the next app start tries again.
        try {
            if (shown) {
                // A pinned copy was disabled while off; enable it before the dynamic one is added again.
                shortcuts.enableShortcuts(ids)
                shortcuts.addDynamicShortcuts(listOf(dictateShortcut(context)))
            } else {
                shortcuts.removeDynamicShortcuts(ids)
                shortcuts.disableShortcuts(ids, context.getString(R.string.shortcut_dictate_disabled))
            }
        } catch (_: IllegalStateException) {
        }
    }

    private fun dictateShortcut(context: Context) = ShortcutInfo.Builder(context, DICTATE_SHORTCUT_ID)
        .setShortLabel(context.getString(R.string.shortcut_dictate))
        .setLongLabel(context.getString(R.string.shortcut_dictate_long))
        .setIcon(Icon.createWithResource(context, R.drawable.ic_shortcut_dictate))
        .setIntent(CaptureActivity.intent(context, dictate = true))
        .setRank(2)
        .build()
}
