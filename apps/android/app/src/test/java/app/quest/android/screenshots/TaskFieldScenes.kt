package app.quest.android.screenshots

import androidx.compose.runtime.Composable
import app.quest.android.ListSamples
import app.quest.android.Samples
import app.quest.android.ui.components.QuestBottomSheet
import app.quest.android.ui.lists.ListActions
import app.quest.android.ui.lists.ListScreen
import app.quest.android.ui.lists.ListUiState
import app.quest.android.ui.lists.QuickAddFields
import app.quest.android.ui.lists.RowActions
import app.quest.android.ui.navigation.Tab
import app.quest.android.ui.task.SheetPage
import app.quest.android.ui.task.TaskCalendarChoices
import app.quest.android.ui.task.TaskQuestChoices
import app.quest.android.ui.task.TaskSheet
import app.quest.android.ui.task.TaskSheetActions
import app.quest.android.ui.task.TaskSheetState
import java.time.LocalDate
import uniffi.quest_mobile.CalendarCell
import uniffi.quest_mobile.CalendarMonth
import uniffi.quest_mobile.DateChoice
import uniffi.quest_mobile.DateTarget
import uniffi.quest_mobile.DayValue
import uniffi.quest_mobile.PostponedCount
import uniffi.quest_mobile.QuickFields

private val listActions = ListActions(RowActions({}, {}, {}, {}), {}, {}, {}, {})
private val detail = Samples.detail.copy(
    plannedOn = DayValue("2026-11-25", "Nov 25, 2026"),
    dueOn = DayValue("2026-11-28", "Nov 28, 2026"),
    postponed = PostponedCount(2u, 1u, 3u),
)

private fun month(target: DateTarget): CalendarMonth {
    val start = LocalDate.parse("2026-10-26")
    val today = LocalDate.parse(detail.plannedOn!!.iso)
    val due = LocalDate.parse(detail.dueOn!!.iso)
    fun day(date: LocalDate) = DayValue(date.toString(), date.toString())
    return CalendarMonth(
        day(today), "November 2026",
        listOf(
            DateChoice("Today", day(today)),
            DateChoice("Tomorrow", day(today.plusDays(1))),
            DateChoice("Next Monday", day(today.plusDays(5))),
            DateChoice("None", null),
        ),
        (0L..41L).map { offset ->
            val date = start.plusDays(offset)
            CalendarCell(
                day(date), date.month == today.month, date >= today,
                date == if (target == DateTarget.PLAN) today else due, date == today,
                when {
                    target == DateTarget.PLAN && date == due -> DateTarget.DEADLINE
                    target == DateTarget.DEADLINE && date == today -> DateTarget.PLAN
                    else -> null
                },
            )
        },
    )
}

@Composable
private fun Background() = Shell(Tab.Today, ListSamples.playerResume) {
    ListScreen(ListUiState.Ready(ListSamples.todayIdle, ""), listActions)
}

@Composable
private fun Calendar(title: String, target: DateTarget) {
    Background()
    QuestBottomSheet(title, onDismiss = {}) {
        TaskCalendarChoices(target, month = { month(target) }, shift = { _, _ -> null }, onPick = {})
    }
}

val taskFieldScenes = listOf(
    Scene("r3-task-detail", SceneKind.Window) {
        Background()
        TaskSheet(TaskSheetState.Open(detail, detail.title, detail.description, SheetPage.Details, null), TaskSheetActions.None)
    },
    Scene("r3-task-quest", SceneKind.Window) {
        Background()
        QuestBottomSheet("Quest", onDismiss = {}) {
            TaskQuestChoices({ detail.questOptions }, onPick = {})
        }
    },
    Scene("r3-task-plan", SceneKind.Window) { Calendar("Plan", DateTarget.PLAN) },
    Scene("r3-task-deadline", SceneKind.Window) { Calendar("Deadline", DateTarget.DEADLINE) },
    Scene("r3-quick-add", SceneKind.Screen) {
        val fields = QuickAddFields(
            QuickFields(Samples.writing.id, "2026-11-25", "2026-11-27"),
            Samples.writing, "Nov 25", "Nov 27",
        )
        Shell(Tab.Today) {
            ListScreen(ListUiState.Ready(ListSamples.todayIdle, "Review draft #writing !fri", fields), listActions)
        }
    },
    Scene("r3-task-done", SceneKind.Screen) {
        val list = ListSamples.todayIdle
        val open = list.sections.first()
        val lingering = open.copy(rows = open.rows.mapIndexed { index, row ->
            if (index == 1) row.copy(done = true, canStartTimer = false) else row
        })
        Shell(Tab.Today) {
            ListScreen(ListUiState.Ready(list.copy(sections = listOf(lingering) + list.sections.drop(1), toast = Samples.toast.copy(text = "Done")), ""), listActions)
        }
    },
)
