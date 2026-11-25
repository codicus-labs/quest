package app.quest.android.settings

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import java.io.IOException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.map
import uniffi.quest_mobile.AddPosition

/**
 * How Home looks on this device: OTHER shown or hidden, and whether new tasks go to the top or
 * bottom. Device preferences, not synced (the desktop keeps both in memory only).
 */
data class HomePrefs(val showOther: Boolean = true, val addPosition: AddPosition = AddPosition.TOP)

interface HomePreferences {
    val prefs: Flow<HomePrefs>

    suspend fun setShowOther(show: Boolean)

    suspend fun setAddPosition(position: AddPosition)
}

/** [HomePreferences] in DataStore Preferences; DataStore does its IO off the main thread. */
class DataStoreHomePreferences(private val store: DataStore<Preferences>) : HomePreferences {
    private val showOtherKey = booleanPreferencesKey("home.show_other")
    private val addPositionKey = stringPreferencesKey("home.add_position")

    /** An unreadable preferences file falls back to the defaults instead of failing the UI. */
    override val prefs: Flow<HomePrefs> = store.data
        .catch { if (it is IOException) emit(emptyPreferences()) else throw it }
        .map { preferences ->
            HomePrefs(
                showOther = preferences[showOtherKey] ?: true,
                addPosition = AddPosition.entries.firstOrNull { it.name == preferences[addPositionKey] } ?: AddPosition.TOP,
            )
        }

    override suspend fun setShowOther(show: Boolean) {
        store.edit { it[showOtherKey] = show }
    }

    override suspend fun setAddPosition(position: AddPosition) {
        store.edit { it[addPositionKey] = position.name }
    }
}
