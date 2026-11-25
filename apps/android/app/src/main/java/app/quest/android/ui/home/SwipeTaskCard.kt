package app.quest.android.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Icon
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import app.quest.android.R
import app.quest.android.ui.components.QuestIcons
import app.quest.android.ui.components.TaskCard
import app.quest.android.ui.components.TaskCardActions
import app.quest.android.ui.theme.QuestSpacing
import app.quest.android.ui.theme.QuestTheme
import app.quest.android.ui.theme.QuestType
import app.quest.android.ui.theme.StatusColors
import uniffi.quest_mobile.HomeTask

/**
 * A [TaskCard] with swipe actions: right completes (or reopens), left deletes. Both leave an
 * undo snackbar (the core's toast). Screen readers get the same actions as custom actions, plus
 * [extraActions]. [swipeEnabled] false turns swiping off (while the card is dragged in the plan).
 */
@Composable
fun SwipeTaskCard(
    task: HomeTask,
    actions: TaskCardActions,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
    swipeEnabled: Boolean = true,
    extraActions: List<CustomAccessibilityAction> = emptyList(),
) {
    val onToggle by rememberUpdatedState(actions.onToggle)
    val onDeleteNow by rememberUpdatedState(onDelete)
    // Material may confirm a value several times per gesture, and restores a swiped state after
    // process death. So a gesture only arms the action (not saved), and the action runs once per
    // settled value, after which the card snaps back.
    var armed by remember { mutableStateOf(false) }
    val swipe = rememberSwipeToDismissBoxState(
        confirmValueChange = { target ->
            if (target != SwipeToDismissBoxValue.Settled) armed = true
            true
        },
    )
    LaunchedEffect(swipe.currentValue) {
        val swiped = swipe.currentValue
        if (swiped == SwipeToDismissBoxValue.Settled) return@LaunchedEffect
        if (armed) {
            armed = false
            if (swiped == SwipeToDismissBoxValue.StartToEnd) onToggle() else onDeleteNow()
        }
        swipe.snapTo(SwipeToDismissBoxValue.Settled)
    }
    val toggleLabel = stringResource(if (task.done) R.string.task_reopen else R.string.task_complete)
    val deleteLabel = stringResource(R.string.task_delete)
    SwipeToDismissBox(
        state = swipe,
        modifier = modifier,
        gesturesEnabled = swipeEnabled,
        backgroundContent = { SwipeBackground(swipe.dismissDirection, task.done) },
    ) {
        TaskCard(
            task,
            actions,
            modifier = Modifier.semantics {
                customActions = listOf(
                    CustomAccessibilityAction(toggleLabel) { actions.onToggle(); true },
                    CustomAccessibilityAction(deleteLabel) { onDelete(); true },
                ) + extraActions
            },
        )
    }
}

/** What shows under the card while it is swiped: the action it will take. */
@Composable
private fun SwipeBackground(direction: SwipeToDismissBoxValue, done: Boolean) {
    if (direction == SwipeToDismissBoxValue.Settled) return
    val colors = QuestTheme.colors
    val deleting = direction == SwipeToDismissBoxValue.EndToStart
    val fill = if (deleting) StatusColors.failed else colors.accent
    val label = stringResource(
        when {
            deleting -> R.string.task_delete
            done -> R.string.task_reopen
            else -> R.string.task_complete
        },
    )
    Row(
        Modifier
            .fillMaxSize()
            .background(fill.copy(alpha = 0.18f), RoundedCornerShape(QuestSpacing.cardRadius))
            .padding(horizontal = QuestSpacing.screen),
        horizontalArrangement = Arrangement.spacedBy(QuestSpacing.small, if (deleting) Alignment.End else Alignment.Start),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val icon = when {
            deleting -> QuestIcons.Trash
            done -> Icons.Filled.Refresh
            else -> Icons.Filled.Check
        }
        Icon(icon, contentDescription = null, tint = fill)
        // The label in `text`: on the tinted fill, `fill` itself drops below 4.5 : 1.
        Text(label, style = QuestType.bodyMedium, color = colors.text)
    }
}
