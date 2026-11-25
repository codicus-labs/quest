package app.quest.android.ui.quests

import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.pressKey
import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeHomePreferences
import app.quest.android.QuestSamples
import app.quest.android.core.CoreHolder
import app.quest.android.ui.theme.QuestTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Renaming and Complete quest on the quest page, through the real screen and view model over a [FakeCore]. */
@OptIn(ExperimentalTestApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class QuestDetailInteractionTest {
    @get:Rule val compose = createComposeRule()

    private val core = FakeCore()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private lateinit var vm: QuestDetailViewModel

    @After
    fun tearDown() = scope.cancel()

    private fun showPage() {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, FakeClock()) { core }
        vm = QuestDetailViewModel(holder, SavedStateHandle(), FakeHomePreferences())
        vm.open("q1")
        val actions = QuestDetailActions(
            close = {}, beginRename = vm::beginRename, editTitle = vm::editTitle, commitTitle = vm::commitTitle,
            saveDrafts = {}, setHue = {}, toggleStatus = vm::toggleStatus, delete = {}, undo = vm::undo, dismissToast = {},
            tasks = QuestTaskActions.None,
        )
        compose.setContent {
            QuestTheme(dark = false) {
                val state by vm.state.collectAsState()
                QuestDetailScreen(state, actions)
            }
        }
    }

    @Test
    fun tappingTheNameRenamesItInPlaceAndEnterSaves() {
        showPage()

        compose.onNodeWithTag("quest-title").performClick()
        compose.onNodeWithTag("quest-rename").assertIsFocused()
        compose.onNodeWithTag("quest-rename").performTextReplacement("The book")
        compose.onNodeWithTag("quest-rename").performKeyInput { pressKey(Key.Enter) }
        compose.waitForIdle()

        assertEquals(listOf("rename-quest q1 The book"), core.calls)
        compose.onNodeWithTag("quest-title").assertExists()
    }

    @Test
    fun completeQuestShowsTheBadgeAndUndoTakesItBack() {
        core.questDetails["q1"] = QuestSamples.detail
        showPage()

        compose.onNodeWithTag("quest-status").performScrollTo().performClick()
        compose.onNodeWithTag("quest-badge").assertTextEquals("COMPLETED")
        vm.undo()
        compose.waitForIdle()

        compose.onNodeWithTag("quest-badge").assertDoesNotExist()
        assertEquals(listOf("toggle-quest q1", "undo"), core.calls)
    }
}
