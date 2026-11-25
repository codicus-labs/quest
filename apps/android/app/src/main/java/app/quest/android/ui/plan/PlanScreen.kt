package app.quest.android.ui.plan

import androidx.compose.ui.semantics.heading
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.Icons
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import app.quest.android.R
import app.quest.android.ui.components.QuestChip
import app.quest.android.ui.components.QuestFilterButton
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.QuestSheetContent
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.home.CenteredNote
import app.quest.android.ui.task.DayPickerDialog
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import uniffi.quest_mobile.QuestChip as QuestRef

/** From this width the plan shows day columns (medium and expanded), as Home shows list and grid side by side. */
private val ColumnsWidth = 600.dp

/** The plan: header (month, Today, date, filter, OTHER), the days, the undo snackbar and "Move to…". */
@Composable
fun PlanScreen(state: PlanUiState, actions: PlanActions) {
    when (state) {
        PlanUiState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is PlanUiState.Failed -> CenteredNote(state.message)
        is PlanUiState.Ready -> ReadyPlan(state, actions)
    }
}

@Composable
private fun ReadyPlan(state: PlanUiState.Ready, actions: PlanActions) {
    var month by rememberSaveable { mutableStateOf("") }
    var pickingDate by rememberSaveable { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val columns = maxWidth >= ColumnsWidth
        Column(Modifier.fillMaxSize()) {
            PlanHeader(state, month, actions, onPickDate = { pickingDate = true })
            if (columns) {
                PlanBoard(state, actions, onMonth = { month = it }, Modifier.weight(1f))
            } else {
                PlanList(state, actions, onMonth = { month = it }, Modifier.weight(1f))
            }
        }
        state.plan.toast?.let { toast ->
            UndoSnackbar(
                toast.text,
                onUndo = actions.undo,
                actionLabel = toast.actionLabel,
                onDismiss = actions.dismissToast,
                modifier = Modifier.align(Alignment.BottomCenter).widthIn(max = 560.dp),
            )
        }
    }
    if (pickingDate) DayPickerDialog(initialIso = state.plan.todayIso, onPick = { actions.jumpTo(it) }, onDismiss = { pickingDate = false })
    state.moveSheet?.let { MoveToSheet(it, actions) }
}

/**
 * Month of what shows, then Today, a date to go to, the quest filter (its chip clears it) and
 * OTHER shown or hidden (desktop header; Earlier/Later are the swipe itself).
 */
@Composable
private fun PlanHeader(state: PlanUiState.Ready, month: String, actions: PlanActions, onPickDate: () -> Unit) {
    val colors = QuestTheme.colors
    val plan = state.plan
    Column(Modifier.fillMaxWidth().padding(start = QuestSpacing.screen, end = QuestSpacing.small)) {
        // At large font sizes the buttons wrap under the month instead of cutting it off.
        FlowRow(itemVerticalAlignment = Alignment.CenterVertically) {
            Text(month, style = QuestType.stat, color = colors.text, modifier = Modifier.semantics { heading() }.testTag("plan-month"))
            Box(Modifier.weight(1f).widthIn(min = QuestSpacing.small))
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedButton(
                    onClick = { actions.jumpTo(null) },
                    contentPadding = PaddingValues(horizontal = QuestSpacing.medium),
                    modifier = Modifier.testTag("plan-today"),
                ) {
                    Text(stringResource(R.string.plan_today), style = QuestType.bodyMedium)
                }
                IconButton(onClick = onPickDate, modifier = Modifier.testTag("plan-pick-date")) {
                    Icon(Icons.Filled.DateRange, contentDescription = stringResource(R.string.plan_pick_date), tint = colors.textMuted)
                }
                QuestFilterButton(plan.questFilter, plan.quests, actions.setQuestFilter, "plan-filter")
                val showOther = state.prefs.showOther
                IconButton(onClick = { actions.setShowOther(!showOther) }, modifier = Modifier.testTag("plan-other-toggle")) {
                    Icon(
                        if (showOther) QuestIcons.Eye else QuestIcons.EyeOff,
                        contentDescription = stringResource(if (showOther) R.string.plan_hide_other else R.string.plan_show_other),
                        tint = colors.textMuted,
                    )
                }
            }
        }
        plan.questFilter?.let { quest ->
            val clear = stringResource(R.string.plan_filter_clear)
            FilterChipButton(quest, clear) { actions.setQuestFilter(null) }
        }
    }
}

/** The filter's chip under the header; tapping it clears the filter (desktop). */
@Composable
private fun FilterChipButton(quest: QuestRef, label: String, onClear: () -> Unit) {
    TextButton(
        onClick = onClear,
        modifier = Modifier
            .padding(bottom = QuestSpacing.small)
            .clearAndSetSemantics { contentDescription = "$label: ${quest.name}"; role = Role.Button }
            .testTag("plan-filter-chip"),
    ) { QuestChip(quest) }
}

/** "Move to…": the core's targets (Today, Tomorrow, the next days, Backlog), then a date from the picker. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
@Composable
private fun MoveToSheet(sheet: MoveSheet, actions: PlanActions) {
    var picking by rememberSaveable { mutableStateOf(false) }
    ModalBottomSheet(
        onDismissRequest = actions.closeMoveTo,
        containerColor = QuestTheme.colors.card,
        // The sheet is its own window, so it needs its own opt-in for UI automation ids.
        modifier = Modifier.semantics { testTagsAsResourceId = true },
    ) { MoveToContent(sheet, actions, onPickDate = { picking = true }) }
    if (picking) DayPickerDialog(initialIso = null, onPick = { actions.moveTo(it) }, onDismiss = { picking = false })
}

/** The inside of "Move to…" (also for screenshots: the modal sheet is a window of its own). */
@Composable
fun MoveToContent(sheet: MoveSheet, actions: PlanActions, onPickDate: () -> Unit) {
    QuestSheetContent(stringResource(R.string.plan_move_title, sheet.title)) {
        Column(Modifier.testTag("move-sheet")) {
            sheet.targets.forEach { target -> SheetRow(target.label, "move-target:${target.label}") { actions.moveTo(target.dayIso) } }
            SheetRow(stringResource(R.string.plan_move_pick_date), "move-target-pick", onPickDate)
        }
    }
}
