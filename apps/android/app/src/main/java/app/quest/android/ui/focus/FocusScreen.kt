package app.quest.android.ui.focus

import androidx.compose.ui.semantics.heading
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.quest.android.R
import app.quest.android.ui.components.OnAccentFill
import app.quest.android.ui.components.Dot
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.QuestPickerSheet
import app.quest.android.ui.components.questColor
import app.quest.android.ui.components.CenteredNote
import app.quest.android.ui.components.EmptyNote
import app.quest.android.ui.task.ChecklistActions
import app.quest.android.ui.task.NoteActions
import app.quest.android.ui.task.TaskChecklist
import app.quest.android.ui.task.TaskNote
import app.quest.android.ui.components.UndoSnackbar
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import kotlin.coroutines.cancellation.CancellationException
import uniffi.quest_mobile.FocusPickTask
import uniffi.quest_mobile.QuestChip as QuestRef

/** Everything Focus can do, wired to [FocusViewModel]. */
class FocusActions(
    val close: () -> Unit,
    val stop: () -> Unit,
    val resume: () -> Unit,
    val complete: () -> Unit,
    val pick: (String) -> Unit,
    val setQuest: (String?) -> Unit,
    val editTitle: (String) -> Unit,
    val commitTitle: () -> Unit,
    /** Saves typed title and description (the app went to the background). */
    val saveDrafts: () -> Unit,
    val notes: NoteActions = NoteActions.None,
    val checklist: ChecklistActions = ChecklistActions(),
    val undo: () -> Unit = {},
    val dismissToast: () -> Unit = {},
    val organize: () -> Unit = {},
) {
    companion object {
        /** Does nothing; for screenshots. */
        val None = FocusActions({}, {}, {}, {}, {}, {}, {}, {}, {})
    }
}

/** The widest Focus's content grows (tablets). */
private val ContentMaxWidth = 560.dp

/**
 * Focus over the whole app (desktop `ui/focus.rs`). Back (predictive: the screen shrinks while
 * the gesture runs) returns to Home. Keeps the screen on while shown when [keepScreenOn].
 */
@Composable
fun FocusScreen(
    state: FocusUiState,
    actions: FocusActions,
    keepScreenOn: Boolean,
    /** The timer note when the timer runs in another workspace (see `TimerElsewhereNote`). */
    note: @Composable () -> Unit = {},
) {
    if (state == FocusUiState.Hidden) return
    var backProgress by remember { mutableFloatStateOf(0f) }
    PredictiveBackHandler { events ->
        try {
            events.collect { backProgress = it.progress }
            actions.close()
        } catch (e: CancellationException) {
            backProgress = 0f
            throw e
        }
    }
    KeepScreenOn(keepScreenOn)
    // Swiping the app from Recents may end the process without another chance to save.
    LifecycleEventEffect(Lifecycle.Event.ON_STOP) { actions.saveDrafts() }
    Box(
        Modifier
            .fillMaxSize()
            .background(QuestTheme.colors.bg)
            .graphicsLayer {
                val scale = 1f - 0.08f * backProgress
                scaleX = scale
                scaleY = scale
            }
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .testTag("focus"),
    ) {
        when (state) {
            FocusUiState.Hidden -> Unit
            FocusUiState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
            is FocusUiState.Failed -> Column {
                TopBar(null, false, actions.close, note)
                CenteredNote(state.message)
            }
            is FocusUiState.Session -> {
                SessionContent(state, actions, note)
                state.toast?.let {
                    UndoSnackbar(it.text, actions.undo, Modifier.align(Alignment.BottomCenter), it.actionLabel, actions.dismissToast)
                }
            }
            is FocusUiState.Pick -> PickContent(state.tasks, actions, note)
        }
    }
}

/** Keeps the display on while this is in the composition and [enabled]. */
@Composable
private fun KeepScreenOn(enabled: Boolean) {
    val view = LocalView.current
    DisposableEffect(view, enabled) {
        view.keepScreenOn = enabled
        onDispose { view.keepScreenOn = false }
    }
}

/** Back on the left; [note] in the middle; the lit (running) or dim dot and the caps label on the right. */
@Composable
private fun TopBar(header: String?, running: Boolean, onBack: () -> Unit, note: @Composable () -> Unit) {
    val colors = QuestTheme.colors
    Row(
        Modifier.fillMaxWidth().height(64.dp).padding(horizontal = QuestSpacing.small),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack, modifier = Modifier.testTag("focus-back")) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.focus_back), tint = colors.textMuted)
        }
        Box(Modifier.weight(1f), contentAlignment = Alignment.Center) { note() }
        if (header != null) {
            Dot(colors.accent.copy(alpha = if (running) 1f else 0.35f), 8.dp)
            Text(
                header,
                style = QuestType.caps,
                color = colors.textMuted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(start = 10.dp, end = QuestSpacing.medium).testTag("focus-header"),
            )
        } else {
            Text(stringResource(R.string.focus).uppercase(), style = QuestType.caps, color = colors.textMuted, modifier = Modifier.padding(end = QuestSpacing.medium))
        }
    }
}

@Composable
private fun SessionContent(state: FocusUiState.Session, actions: FocusActions, note: @Composable () -> Unit) {
    val session = state.session
    val colors = QuestTheme.colors
    Column(Modifier.fillMaxSize()) {
        TopBar(stringResource(R.string.focus).uppercase(), session.running, actions.close, note)
        CenteredScroll(Modifier.weight(1f)) {
            Column(Modifier.widthIn(max = ContentMaxWidth).fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                QuestPicker(session.quest, state.quests, actions.setQuest)
                FocusTitle(state.title, actions.editTitle, actions.commitTitle, Modifier.padding(top = 20.dp))
                BasicText(
                    session.runningTimer ?: session.timer,
                    style = QuestType.timer.copy(color = colors.text),
                    maxLines = 1,
                    autoSize = TextAutoSize.StepBased(minFontSize = 40.sp, maxFontSize = QuestType.timer.fontSize),
                    modifier = Modifier.padding(top = QuestSpacing.large).testTag("focus-timer"),
                )
                val today = stringResource(R.string.focus_today, session.trackedToday)
                val week = session.questHoursThisWeek
                Text(
                    if (week == null) today else stringResource(R.string.focus_summary, today, week),
                    style = QuestType.small,
                    color = colors.textMuted,
                    modifier = Modifier.padding(top = QuestSpacing.small),
                )
                TaskNote(session.taskId, state.description, actions.notes, "focus-description", Modifier.padding(top = QuestSpacing.medium))
                if (state.checklist.isNotEmpty()) {
                    TaskChecklist(session.taskId, state.checklist, actions.checklist, editable = false)
                }
                FlowRow(Modifier.padding(top = QuestSpacing.medium), horizontalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterHorizontally)) {
                    if (session.running) {
                        FocusButton(stringResource(R.string.focus_stop), "focus-stop", actions.stop) { Icon(QuestIcons.Stop, null, Modifier.size(18.dp)) }
                    } else {
                        FocusButton(stringResource(R.string.focus_resume), "focus-resume", actions.resume) { Icon(Icons.Filled.PlayArrow, null, Modifier.size(18.dp)) }
                    }
                    FocusButton(stringResource(R.string.focus_done), "focus-complete", actions.complete, primary = true) { Icon(Icons.Filled.Check, null, Modifier.size(18.dp)) }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally)) {
                    TextButton(onClick = actions.close) {
                        Text(stringResource(R.string.focus_back), color = colors.textMuted)
                    }
                    if (session.running) {
                        TextButton(onClick = actions.organize, modifier = Modifier.testTag("focus-organize")) {
                            Text(stringResource(R.string.focus_organize), color = colors.textMuted)
                        }
                    }
                }
            }
        }
    }
}

/** Scrolls when the content is taller than the screen (keyboard, tablet in landscape); centred otherwise, as on the desktop. */
@Composable
private fun CenteredScroll(modifier: Modifier, content: @Composable () -> Unit) {
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val minHeight = maxHeight
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .heightIn(min = minHeight)
                .padding(horizontal = QuestSpacing.screen, vertical = QuestSpacing.large),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) { content() }
    }
}

@Composable
private fun FocusButton(label: String, tag: String, onClick: () -> Unit, primary: Boolean = false, icon: @Composable () -> Unit) {
    val colors = QuestTheme.colors
    OutlinedButton(
        onClick = onClick,
        shape = RoundedCornerShape(QuestSpacing.small),
        colors = if (primary) ButtonDefaults.outlinedButtonColors(containerColor = colors.accentFill, contentColor = OnAccentFill)
            else ButtonDefaults.outlinedButtonColors(contentColor = colors.text),
        modifier = Modifier.heightIn(min = QuestSpacing.touchTarget).testTag(tag),
    ) {
        Box(Modifier.padding(end = QuestSpacing.small)) { icon() }
        Text(label, style = QuestType.bodyMedium)
    }
}

/** The quest as an outlined pill in its colour; tap to change it ("No quest" first). */
@Composable
private fun QuestPicker(quest: QuestRef?, quests: List<QuestRef>, onPick: (String?) -> Unit) {
    val colors = QuestTheme.colors
    var open by rememberSaveable { mutableStateOf(false) }
    val color = quest?.let { questColor(it.colorHex) } ?: colors.textFaint
    val name = quest?.name ?: stringResource(R.string.no_quest)
    val label = stringResource(R.string.focus_change_quest)
    val shape = RoundedCornerShape(50)
    Box(
        Modifier
            .heightIn(min = QuestSpacing.touchTarget)
            .clickable(onClickLabel = label) { open = true }
            .semantics { contentDescription = "$label: $name" }
            .testTag("focus-quest"),
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier.height(32.dp).border(QuestSpacing.hairline, color.copy(alpha = 0.5f), shape).padding(horizontal = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small),
        ) {
            Dot(color, 8.dp)
            Text(name, style = QuestType.body, color = colors.textMuted, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
    if (open) QuestPickerSheet(label, quests, quest?.id, stringResource(R.string.no_quest), onPick = { onPick(it?.id) }, onDismiss = { open = false })
}

/** "What are you working on?": today's open tasks; tapping one starts its timer. */
@Composable
private fun PickContent(tasks: List<FocusPickTask>, actions: FocusActions, note: @Composable () -> Unit) {
    val colors = QuestTheme.colors
    Column(Modifier.fillMaxSize()) {
        TopBar(null, false, actions.close, note)
        CenteredScroll(Modifier.weight(1f)) {
            Column(Modifier.widthIn(max = ContentMaxWidth).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(QuestSpacing.small)) {
                Text(
                    stringResource(R.string.focus_pick_title),
                    style = QuestType.heading,
                    color = colors.text,
                    modifier = Modifier.padding(bottom = QuestSpacing.medium).semantics { heading() },
                )
                if (tasks.isEmpty()) EmptyNote(stringResource(R.string.focus_pick_empty))
                tasks.forEach { task -> PickRow(task) { actions.pick(task.id) } }
            }
        }
    }
}

@Composable
private fun PickRow(task: FocusPickTask, onStart: () -> Unit) {
    val colors = QuestTheme.colors
    val shape = RoundedCornerShape(QuestSpacing.cardRadius)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .background(colors.card, shape)
            .border(QuestSpacing.hairline, colors.border, shape)
            .clickable(onClickLabel = stringResource(R.string.timer_start), onClick = onStart)
            .padding(horizontal = 14.dp)
            .testTag("focus-pick:${task.title}"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.medium),
    ) {
        Box(Modifier.width(3.dp).height(24.dp).background(task.quest?.let { questColor(it.colorHex) } ?: colors.textFaint, RoundedCornerShape(2.dp)))
        Text(task.title, style = QuestType.body, color = colors.text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        Icon(Icons.Filled.PlayArrow, contentDescription = null, tint = colors.textMuted, modifier = Modifier.size(18.dp))
    }
}
