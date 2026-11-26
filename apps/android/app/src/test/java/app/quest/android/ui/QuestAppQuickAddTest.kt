package app.quest.android.ui

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeFocusPreferences
import app.quest.android.ListSamples
import app.quest.android.PlanSamples
import app.quest.android.core.CoreHolder
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
import org.robolectric.shadows.ShadowDialog
import uniffi.quest_mobile.ListKind

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w411dp-h914dp-port-mdpi")
class QuestAppQuickAddTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    private val core = FakeCore()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = scope.cancel()

    private fun showApp() {
        val holder = CoreHolder(scope, Dispatchers.Unconfined, FakeClock()) { core }
        val models = testAppViewModels(holder)
        compose.setContent { QuestTheme(false) { QuestApp(models, emptyFlow(), FakeFocusPreferences()) } }
    }

    private fun click(tag: String) = compose.onNodeWithTag(tag).performClick()

    private fun add(title: String) {
        compose.onNodeWithTag("list-add").assertIsFocused().performTextInput(title)
        click("list-add-send")
    }

    private fun back() {
        compose.runOnIdle { checkNotNull(ShadowDialog.getLatestDialog()).onBackPressed() }
        compose.onNodeWithTag("list-add").assertDoesNotExist()
    }

    @Test
    fun createsFromTodayInboxAndQuestWithTheirDefaults() {
        val quest = ListKind.Quest("q1")
        core.lists[quest] = ListSamples.questList.copy(kind = quest)
        showApp()
        click("list-add-open")
        add("Today task")
        back()

        click("tab-inbox")
        click("list-add-open")
        add("Inbox task")
        back()

        click("tab-quests")
        click("quest-row:Write the book")
        click("list-add-open")
        add("Quest task")
        compose.runOnIdle {
            assertEquals(listOf(
                "quick-add Today task||null|${core.quickAddForm.today.iso}",
                "quick-add Inbox task||null|null",
                "quick-add Quest task||q1|null",
            ), core.calls)
        }
    }

    @Test
    fun taskDetailHidesTheListsPlus() {
        showApp()
        compose.onNodeWithText("Sketch a pricing page").performClick()
        compose.onNodeWithTag("task-sheet").assertExists()
        compose.onNodeWithTag("list-add-open").assertDoesNotExist()
    }

    @Test
    fun dayPlusUsesItsDateAndBackKeepsTitleAndMultilineNote() {
        showApp()
        click("segment-plan")
        click("plan-week-${PlanSamples.friday}")
        click("plan-add-${PlanSamples.friday}")
        compose.onNodeWithTag("list-add").assertIsFocused().performTextInput("Plan task")
        click("list-add-note-toggle")
        compose.onNodeWithTag("list-add-note").performTextInput("First line\nSecond line")
        back()
        click("plan-add-${PlanSamples.friday}")
        compose.onNodeWithText("Plan task").assertExists()
        compose.onNodeWithText("First line\nSecond line").assertExists()
        click("list-add-send")
        compose.runOnIdle {
            assertEquals(listOf("quick-add Plan task|First line\nSecond line|null|${PlanSamples.friday}"), core.calls)
        }
    }
}
