package app.quest.android.ui.search

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.v2.createComposeRule
import app.quest.android.QuestSamples
import app.quest.android.ui.theme.QuestTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class SearchScreenTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun aRestoredQueryIsNotClearedWhileTheCoreOpens() {
        val edits = mutableListOf<String>()
        var state: SearchUiState by mutableStateOf(SearchUiState.Loading)
        compose.setContent { QuestTheme(dark = false) { SearchScreen(state, SearchActions({}, { edits += it }, {}, {})) } }

        state = SearchUiState.Ready("mara", SavedSearchMode.Commands, "Search or jump…", QuestSamples.commands)
        compose.waitForIdle()

        assertEquals(listOf("mara"), edits.distinct())
    }
}
