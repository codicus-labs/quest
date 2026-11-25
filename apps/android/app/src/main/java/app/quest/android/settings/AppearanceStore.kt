package app.quest.android.settings

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import java.io.IOException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.map

/** Light or dark look. A device preference, not synced: the domain has no such setting. */
enum class Appearance { System, Light, Dark }

/** Where the appearance choice lives. */
interface AppearanceRepository {
    val appearance: Flow<Appearance>

    suspend fun set(appearance: Appearance)
}

/** [AppearanceRepository] in DataStore Preferences; DataStore does its IO off the main thread. */
class DataStoreAppearanceRepository(private val store: DataStore<Preferences>) : AppearanceRepository {
    private val key = stringPreferencesKey("appearance")

    /** An unreadable preferences file falls back to [Appearance.System] instead of failing the UI. */
    override val appearance: Flow<Appearance> = store.data
        .catch { if (it is IOException) emit(emptyPreferences()) else throw it }
        .map { preferences -> Appearance.entries.firstOrNull { it.name == preferences[key] } ?: Appearance.System }

    override suspend fun set(appearance: Appearance) {
        store.edit { it[key] = appearance.name }
    }
}
