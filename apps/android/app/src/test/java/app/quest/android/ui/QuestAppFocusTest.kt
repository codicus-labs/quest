package app.quest.android.ui

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import app.quest.android.FakeClock
import app.quest.android.FakeFocusPreferences
import app.quest.android.FakeCore
import app.quest.android.core.CoreHolder
import app.quest.android.ui.task.TaskSheetState
import app.quest.android.ui.theme.QuestTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.emptyFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Focus over the whole app: Home under it must not react. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class QuestAppFocusTest {
    @get:Rule val compose = createComposeRule()

    private val core = FakeCore()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun tapsOnFocusNeverReachHome() {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, FakeClock()) { core }
        val viewModels = testAppViewModels(holder)
        val sheet = viewModels.taskSheet
        val focus = viewModels.focus
        compose.setContent { QuestTheme(dark = false) { QuestApp(viewModels, emptyFlow(), FakeFocusPreferences()) } }
        compose.onNodeWithTag("home").assertExists()

        focus.open()
        compose.waitForIdle()
        compose.onNodeWithTag("home").assertDoesNotExist()
        // Where Home's cards are: an empty part of Focus.
        compose.onRoot().performTouchInput {
            click(Offset(centerX, height * 0.5f))
            click(Offset(centerX, height * 0.85f))
        }
        compose.waitForIdle()

        assertEquals(emptyList<String>(), core.calls)
        assertEquals(emptyList<String>(), core.toggled)
        assertEquals(TaskSheetState.Closed, sheet.state.value)
    }
}
