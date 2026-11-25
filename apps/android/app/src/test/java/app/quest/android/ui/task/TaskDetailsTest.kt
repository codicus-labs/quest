package app.quest.android.ui.task

import android.os.Looper
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.platform.UriHandler
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.lifecycle.SavedStateHandle
import app.quest.android.FakeClock
import app.quest.android.FakeCore
import app.quest.android.FakeRepeatRules
import app.quest.android.Samples
import app.quest.android.screenshots.sampleMarkerOffset
import app.quest.android.screenshots.sampleNoteBlocks
import app.quest.android.screenshots.sampleNoteSource
import app.quest.android.ui.openedHolder
import app.quest.android.ui.theme.QuestTheme
import java.time.Duration
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import uniffi.quest_mobile.ChecklistRow
import uniffi.quest_mobile.MarkdownBlock
import uniffi.quest_mobile.MarkdownInline
import uniffi.quest_mobile.MarkdownInlineKind

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
@OptIn(ExperimentalCoroutinesApi::class, ExperimentalTestApi::class)
class TaskDetailsTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun tappingMarkdownCheckboxCallsCoreWithModelOffsetWithoutOpeningEditor() = runTest(UnconfinedTestDispatcher()) {
        val core = FakeCore()
        core.details[Samples.detail.id] = Samples.detail.copy(description = sampleNoteSource)
        val vm = TaskSheetViewModel(
            openedHolder(core, backgroundScope, UnconfinedTestDispatcher(testScheduler)),
            SavedStateHandle(), FakeRepeatRules(), FakeClock(),
        )
        vm.open(Samples.detail.id)
        compose.setContent {
            QuestTheme(dark = false) {
                TaskNote(
                    Samples.detail.id, sampleNoteSource,
                    NoteActions({}, {}, { sampleNoteBlocks }, vm.notes.toggleMarker), "notes",
                )
            }
        }

        compose.onNodeWithTag("note-marker-$sampleMarkerOffset").performTouchInput { click() }

        assertEquals(listOf("toggle marker $sampleMarkerOffset"), core.calls)
        compose.onNodeWithTag("notes-source").assertDoesNotExist()
    }

    @Test
    fun noteLinkOpensExternallyAndDoneReturnsFromEditor() {
        val url = "https://example.com"
        val opened = mutableListOf<String>()
        val handler = object : UriHandler {
            override fun openUri(uri: String) { opened += uri }
        }
        val blocks = listOf(MarkdownBlock.Paragraph(listOf(
            MarkdownInline("Outline", false, false, MarkdownInlineKind.Link(url)),
        )))
        var saves = 0
        compose.setContent {
            CompositionLocalProvider(LocalUriHandler provides handler) {
                QuestTheme(dark = false) {
                    TaskNote("task", "[Outline]($url)", NoteActions({}, { saves++ }, { blocks }, { _, _ -> }), "notes")
                }
            }
        }

        compose.onNodeWithText("Outline", useUnmergedTree = true).performTouchInput { click() }
        assertEquals(listOf(url), opened)
        compose.onNodeWithTag("notes-source").assertDoesNotExist()

        compose.onNodeWithTag("notes").performClick()
        compose.onNodeWithTag("notes-source").assertIsDisplayed()
        compose.onNodeWithText("Done").performClick()
        compose.onNodeWithTag("notes-source").assertDoesNotExist()
        assertEquals(1, saves)
    }

    @Test
    fun longPressMovesChecklistPointIntoGap() {
        var moved: Pair<String, String?>? = null
        compose.setContent {
            QuestTheme(dark = false) {
                TaskChecklist(
                    "task",
                    listOf(ChecklistRow("first", "First", false), ChecklistRow("second", "Second", false)),
                    ChecklistActions(move = { id, before -> moved = id to before }),
                    editable = true,
                )
            }
        }
        val root = compose.onRoot()
        val origin = root.fetchSemanticsNode().boundsInRoot.topLeft
        val first = compose.onNodeWithTag("checklist-toggle-first").fetchSemanticsNode().boundsInRoot
        val second = compose.onNodeWithTag("checklist-toggle-second").fetchSemanticsNode().boundsInRoot
        root.performTouchInput { down(first.center - origin) }
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(650))
        compose.mainClock.advanceTimeBy(650)
        root.performTouchInput { moveTo(Offset(second.center.x, second.bottom - 3) - origin) }
        compose.onNodeWithTag("checklist-drop-gap").assertIsDisplayed()
        root.performTouchInput { up() }

        assertEquals("first" to null, moved)
    }

    @Test
    fun enterInsertsAndFocusesNewPointAndEmptyBackspaceDeletesIt() {
        val insertedAfter = mutableListOf<String>()
        val removed = mutableListOf<String>()
        compose.setContent {
            var points by remember { mutableStateOf(listOf(ChecklistRow("first", "First", false))) }
            QuestTheme(dark = false) {
                TaskChecklist(
                    "task", points,
                    ChecklistActions(
                        insertAfter = {
                            insertedAfter += it
                            points = points + ChecklistRow("new", "", false)
                            "new"
                        },
                        delete = {
                            removed += it
                            points = points.filterNot { point -> point.id == it }
                        },
                    ),
                    editable = true,
                )
            }
        }

        compose.onNodeWithTag("checklist-text-first").performClick().performKeyInput { pressKey(Key.Enter) }
        compose.onNodeWithTag("checklist-text-new").assertIsDisplayed().performKeyInput { pressKey(Key.Backspace) }

        assertEquals(listOf("first"), insertedAfter)
        assertEquals(listOf("new"), removed)
        compose.onNodeWithTag("checklist-text-new").assertDoesNotExist()
        compose.onNodeWithTag("checklist-text-first").performImeAction()
        assertEquals(listOf("first", "first"), insertedAfter)
    }
}
