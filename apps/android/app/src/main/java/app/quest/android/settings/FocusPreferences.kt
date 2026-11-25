package app.quest.android.settings

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import java.io.IOException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.map

/**
 * Focus on this device (not synced): keep the screen on while Focus is visible, and whether the
 * app already asked for notification permission (it asks once, at the first timer start).
 */
data class FocusPrefs(val keepScreenOn: Boolean = true, val notificationsAsked: Boolean = false)

interface FocusPreferences {
    val prefs: Flow<FocusPrefs>

    suspend fun setKeepScreenOn(keep: Boolean)

    suspend fun setNotificationsAsked()
}

/** [FocusPreferences] in DataStore Preferences; DataStore does its IO off the main thread. */
class DataStoreFocusPreferences(private val store: DataStore<Preferences>) : FocusPreferences {
    private val keepScreenOnKey = booleanPreferencesKey("focus.keep_screen_on")
    private val notificationsAskedKey = booleanPreferencesKey("focus.notifications_asked")

    /** An unreadable preferences file falls back to the defaults instead of failing the UI. */
    override val prefs: Flow<FocusPrefs> = store.data
        .catch { if (it is IOException) emit(emptyPreferences()) else throw it }
        .map { preferences ->
            FocusPrefs(
                keepScreenOn = preferences[keepScreenOnKey] ?: true,
                notificationsAsked = preferences[notificationsAskedKey] ?: false,
            )
        }

    override suspend fun setKeepScreenOn(keep: Boolean) {
        store.edit { it[keepScreenOnKey] = keep }
    }

    override suspend fun setNotificationsAsked() {
        store.edit { it[notificationsAskedKey] = true }
    }
}
