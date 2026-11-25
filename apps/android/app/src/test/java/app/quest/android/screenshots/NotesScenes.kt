package app.quest.android.screenshots

import androidx.compose.runtime.Composable
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import app.quest.android.ListSamples
import app.quest.android.Samples
import app.quest.android.ui.focus.FocusActions
import app.quest.android.ui.focus.FocusScreen
import app.quest.android.ui.focus.FocusUiState
import app.quest.android.ui.lists.ListActions
import app.quest.android.ui.lists.ListScreen
import app.quest.android.ui.lists.ListUiState
import app.quest.android.ui.lists.RowActions
import app.quest.android.ui.navigation.Tab
import app.quest.android.ui.task.NoteActions
import app.quest.android.ui.task.SheetPage
import app.quest.android.ui.task.TaskSheet
import app.quest.android.ui.task.TaskSheetActions
import app.quest.android.ui.task.TaskSheetState
import uniffi.quest_mobile.ChecklistProgress
import uniffi.quest_mobile.ChecklistRow
import uniffi.quest_mobile.MarkdownBlock
import uniffi.quest_mobile.MarkdownInline
import uniffi.quest_mobile.MarkdownInlineKind
import uniffi.quest_mobile.MarkdownListItem
import uniffi.quest_mobile.MarkdownTaskMarker

private const val SOURCE = "## Chapter brief\n\nKeep the **storm** and end on a *question*. See the [outline](https://example.com).\n\n- [ ] Read opening aloud"
internal val sampleNoteSource = SOURCE
internal val sampleMarkerOffset = SOURCE.substringBefore("[ ]").encodeToByteArray().size.toULong()
private fun text(value: String, bold: Boolean = false, italic: Boolean = false, kind: MarkdownInlineKind = MarkdownInlineKind.Text) =
    MarkdownInline(value, bold, italic, kind)
private fun paragraph(vararg runs: MarkdownInline) = MarkdownBlock.Paragraph(runs.toList())

internal val sampleNoteBlocks = listOf(
    MarkdownBlock.Heading(2u, listOf(text("Chapter brief"))),
    paragraph(text("Keep the "), text("storm", bold = true), text(" and end on a "), text("question", italic = true),
        text(". See the "), text("outline", kind = MarkdownInlineKind.Link("https://example.com")), text(".")),
    MarkdownBlock.ItemList(null, listOf(
        MarkdownListItem(MarkdownTaskMarker(false, sampleMarkerOffset), listOf(paragraph(text("Read opening aloud")))),
    )),
)
private val points = listOf(
    "Gather notes", "Pick the setting", "Draft the opening", "Check pacing", "Send to editor",
).mapIndexed { index, title -> ChecklistRow("point-$index", title, index < 2) }
private val detail = Samples.detail.copy(description = SOURCE, checklist = points)
private val state = TaskSheetState.Open(detail, detail.title, SOURCE, SheetPage.Details, null)
private val rows = ListActions(RowActions({}, {}, {}, {}), {}, {}, {}, {})
private val plainNote = NoteActions({}, {}, { listOf(paragraph(text(it))) }, { _, _ -> })
private val sampleNote = NoteActions({}, {}, { sampleNoteBlocks }, { _, _ -> })

internal fun sampleSheetActions(notes: NoteActions = plainNote) =
    TaskSheetActions({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, notes = notes)

internal fun sampleFocusActions(notes: NoteActions = plainNote) =
    FocusActions({}, {}, {}, {}, {}, {}, {}, {}, {}, notes = notes)

@Composable
internal fun NotesSheet() {
    Shell(Tab.Today) { ListScreen(ListUiState.Ready(ListSamples.todayIdle, ""), rows) }
    TaskSheet(state, sampleSheetActions(sampleNote))
}

@Composable
internal fun NoteFocus() {
    FocusScreen(
        FocusUiState.Session(Samples.focusSession, detail.title, SOURCE, Samples.home.quests, checklist = points),
        sampleFocusActions(sampleNote), keepScreenOn = false,
    )
}

internal val notesScenes = listOf(
    Scene("r4-note-checklist", SceneKind.Window) { NotesSheet() },
    Scene("r4-note-edit", SceneKind.Window, interact = { onNodeWithTag("sheet-description").performClick() }) { NotesSheet() },
    Scene("r4-focus-checklist", SceneKind.Screen) { NoteFocus() },
    Scene("r4-list-progress", SceneKind.Screen) {
        val list = ListSamples.todayIdle
        val sections = list.sections.map { section ->
            section.copy(rows = section.rows.map { row ->
                if (row.id == detail.id) row.copy(title = detail.title, checklistProgress = ChecklistProgress(2u, 5u)) else row
            })
        }
        Shell(Tab.Today) { ListScreen(ListUiState.Ready(list.copy(sections = sections), ""), rows) }
    },
)
