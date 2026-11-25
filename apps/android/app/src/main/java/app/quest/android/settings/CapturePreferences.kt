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
 * AI capture on this device (not synced): whether the user has seen the privacy note (shown when turning AI
 * features on, or before the first text goes out if they were turned on elsewhere). Whether the entries show is
 * the account's setting (`Account.aiFeatures`).
 */
data class CapturePrefs(val privacyAcknowledged: Boolean = false)

interface CapturePreferences {
    val prefs: Flow<CapturePrefs>

    suspend fun setPrivacyAcknowledged()
}

/** [CapturePreferences] in DataStore Preferences; DataStore does its IO off the main thread. */
class DataStoreCapturePreferences(private val store: DataStore<Preferences>) : CapturePreferences {
    private val privacyKey = booleanPreferencesKey("capture.privacy_acknowledged")

    /** An unreadable preferences file falls back to the defaults instead of failing the UI. */
    override val prefs: Flow<CapturePrefs> = store.data
        .catch { if (it is IOException) emit(emptyPreferences()) else throw it }
        .map { preferences -> CapturePrefs(privacyAcknowledged = preferences[privacyKey] ?: false) }

    override suspend fun setPrivacyAcknowledged() {
        store.edit { it[privacyKey] = true }
    }
}
